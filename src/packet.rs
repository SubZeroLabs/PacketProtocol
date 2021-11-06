use crate::buffer::BufferState;
use crate::protocol_version::{MCProtocol, MapEncodable};
use anyhow::Context;
use flate2::bufread::ZlibEncoder;
use flate2::Compression;
use minecraft_data_types::{encoder::*, nums::VarInt};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Display, Formatter};
use std::io::{Read, Seek};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex, MutexGuard};
use tokio::time::{timeout_at, Duration, Instant};

pub trait WritablePacket: MapEncodable {
    fn to_resolved_packet(&self, protocol: MCProtocol) -> anyhow::Result<ResolvedPacket>;
}

pub struct ResolvedPacket {
    compression_data: Option<(VarInt, VarInt)>,
    packet_id: VarInt,
    uncompressed_length: VarInt,
    packet: Vec<u8>,
}

impl Display for ResolvedPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.packet_id)
    }
}

impl Debug for ResolvedPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.packet_id)
    }
}

impl ResolvedPacket {
    pub fn new(packet_id: VarInt, packet: Vec<u8>) -> anyhow::Result<Self> {
        Ok(Self {
            compression_data: None,
            packet_id,
            uncompressed_length: packet_id.size()? + VarInt::try_from(packet.len())?,
            packet,
        })
    }

    pub fn from_cursor(mut cursor: std::io::Cursor<Vec<u8>>) -> anyhow::Result<Self> {
        cursor.rewind()?;
        let (packet_size, packet_id) = VarInt::decode_and_size(&mut cursor)?;
        let mut packet = Vec::new();
        std::io::Read::read_to_end(&mut cursor, &mut packet)?;
        Ok(Self {
            compression_data: None,
            packet_id,
            uncompressed_length: packet_size + VarInt::try_from(packet.len())?,
            packet,
        })
    }

    pub fn from_mapped_encodable<T: MapEncodable>(
        packet_id: VarInt,
        protocol: MCProtocol,
        encodable: &T,
    ) -> anyhow::Result<Self> {
        let sized = encodable.size_mapped(protocol)?;
        let mut packet: Vec<u8> = Vec::with_capacity(sized.try_into()?);
        encodable.encode_mapped(protocol, &mut packet)?;
        ResolvedPacket::new(packet_id, packet)
    }

    pub fn compress(&mut self, compression_threshold: i32) -> anyhow::Result<()> {
        let mut new_packet = Vec::with_capacity(usize::try_from(self.uncompressed_length)?);
        self.packet_id.encode(&mut new_packet)?;
        new_packet.append(&mut self.packet);

        if self.uncompressed_length > compression_threshold {
            log::trace!(
                "Compressing packet of length {} for threshold {}",
                self.uncompressed_length,
                compression_threshold
            );

            let mut encoder = ZlibEncoder::new(new_packet.as_slice(), Compression::default());

            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed)?;

            self.compression_data = Some((
                VarInt::try_from(compressed.len())? + self.uncompressed_length.size()?,
                self.uncompressed_length,
            ));
            self.packet = compressed;
        } else {
            log::trace!(
                "Not compressing packet of length {} for threshold {}",
                self.uncompressed_length,
                compression_threshold
            );
            self.compression_data = Some((self.uncompressed_length + 1, VarInt::from(0)));
            self.packet = new_packet;
        }
        Ok(())
    }

    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        if let Some((packet_length, data_length)) = self.compression_data {
            log::trace!(
                "Compression Encoding ({}, {}) for {}",
                packet_length,
                data_length,
                self.packet.len()
            );
            packet_length.encode(writer)?;
            data_length.encode(writer)?;
            writer.write_all(&self.packet)?; // the packet will include the ID if compressed
            Ok(())
        } else {
            self.uncompressed_length.encode(writer)?;
            self.packet_id.encode(writer)?;
            writer.write_all(&self.packet)?;
            Ok(())
        }
    }

    pub async fn write_async<W: tokio::io::AsyncWrite + Send + Unpin>(
        &self,
        writer: &mut W,
    ) -> anyhow::Result<()> {
        if let Some((packet_length, data_length)) = self.compression_data {
            log::trace!(
                "Compression Encoding ({}, {}) for {}",
                packet_length,
                data_length,
                self.packet.len()
            );
            packet_length.async_encode(writer).await?;
            data_length.async_encode(writer).await?;
            writer.write_all(&self.packet).await?; // the packet will include the ID if compressed
            Ok(())
        } else {
            self.uncompressed_length.async_encode(writer).await?;
            self.packet_id.async_encode(writer).await?;
            writer.write_all(&self.packet).await?;
            Ok(())
        }
    }

    pub fn size(&self) -> anyhow::Result<usize> {
        if let Some((packet_length, _)) = self.compression_data {
            (packet_length.size()? + packet_length)
                .try_into()
                .context("Failed to convert VarInt to usize.")
        } else {
            (self.uncompressed_length.size()? + self.uncompressed_length)
                .try_into()
                .context("Failed to convert VarInt to usize.")
        }
    }
}

pub struct PacketWriter<T: tokio::io::AsyncWrite + Send + Sync + Sized + Unpin> {
    internal_writer: T,
    codec: Option<crate::encryption::Codec>,
    compression_threshold: Option<i32>,
}

impl<T: tokio::io::AsyncWrite + Send + Sync + Sized + Unpin> PacketWriter<T> {
    pub fn new(internal_writer: T) -> Self {
        PacketWriter {
            internal_writer,
            codec: None,
            compression_threshold: None,
        }
    }

    pub fn enable_encryption(&mut self, codec: crate::encryption::Codec) {
        self.codec = Some(codec);
    }

    pub fn enable_compression(&mut self, compression_threshold: i32) {
        self.compression_threshold = Some(compression_threshold);
    }

    pub async fn send_resolved_packet(
        &mut self,
        packet: &mut ResolvedPacket,
    ) -> anyhow::Result<()> {
        if let Some(compression) = self.compression_threshold {
            packet.compress(compression)?;
        }
        if let Some(codec) = &mut self.codec {
            let mut buf = Vec::with_capacity(packet.size()?);
            packet.write_async(&mut buf).await?;
            codec.encrypt(&mut buf);
            self.internal_writer
                .write_all(&buf)
                .await
                .context("Failed to write encoded packet.")
        } else {
            packet
                .write_async(&mut self.internal_writer)
                .await
                .context("Failed to write non-encoded packet.")
        }
    }
}

pub struct PacketReader<T: tokio::io::AsyncRead + Send + Sync + Sized + Unpin> {
    internal_reader: T,
    buffer: crate::buffer::MinecraftPacketBuffer,
    address: Arc<SocketAddr>,
}

impl<T: tokio::io::AsyncRead + Send + Sync + Sized + Unpin> PacketReader<T> {
    pub fn new(internal_reader: T, address: Arc<SocketAddr>) -> Self {
        PacketReader {
            internal_reader,
            buffer: crate::buffer::MinecraftPacketBuffer::new(),
            address,
        }
    }

    pub fn enable_decryption(&mut self, codec: crate::encryption::Codec) {
        self.buffer.enable_decryption(codec);
    }

    pub fn enable_decompression(&mut self) {
        self.buffer.enable_decompression();
    }

    async fn read_buf(&mut self) -> anyhow::Result<()> {
        let mut buf = self.buffer.inner_buf();
        self.internal_reader.read_buf(&mut buf).await?;
        Ok(())
    }

    pub fn poll(&mut self) -> BufferState {
        self.buffer.poll()
    }

    pub async fn next_packet(&mut self) -> anyhow::Result<std::io::Cursor<Vec<u8>>> {
        let (encoded, decoded) = self.buffer.len();
        loop {
            match self.poll() {
                BufferState::PacketReady => {
                    return self.buffer.packet_reader();
                }
                BufferState::Waiting => {
                    log::trace!(target: &self.address.to_string(), "Buf read awaiting packet: Encoded {}, Decoded: {}", encoded, decoded);
                    if let Err(err) =
                    timeout_at(Instant::now() + Duration::from_secs(10), self.read_buf()).await
                    {
                        let len = { self.buffer.len() };
                        log::trace!(target: &self.address.to_string(), "Failed read with buffer: {:?}, {:?}", self.buffer.inner_buf(), len);
                        anyhow::bail!("Error occurred reading buffer: {:?}", err);
                    } else if let (0, 0) = self.buffer.len() {
                        anyhow::bail!("Found buffer EOF when not expected, ending.");
                    }
                }
                BufferState::Error(error) => {
                    anyhow::bail!("Found error {} while polling buffer.", error);
                }
            }
        }
    }
}

pub struct PacketReadWriteLocker<
    R: tokio::io::AsyncRead + Send + Sync + Sized + Unpin,
    W: tokio::io::AsyncWrite + Send + Sync + Sized + Unpin,
> {
    packet_writer: Arc<Mutex<PacketWriter<W>>>,
    packet_reader: Arc<Mutex<PacketReader<R>>>,
}

impl<
    R: tokio::io::AsyncRead + Send + Sync + Sized + Unpin,
    W: tokio::io::AsyncWrite + Send + Sync + Sized + Unpin,
> PacketReadWriteLocker<R, W>
{
    pub fn new(
        packet_writer: Arc<Mutex<PacketWriter<W>>>,
        packet_reader: Arc<Mutex<PacketReader<R>>>,
    ) -> Self {
        Self {
            packet_writer,
            packet_reader,
        }
    }

    pub async fn lock_reader(&self) -> MutexGuard<'_, PacketReader<R>> {
        self.packet_reader.lock().await
    }

    pub async fn lock_writer(&self) -> MutexGuard<'_, PacketWriter<W>> {
        self.packet_writer.lock().await
    }

    pub async fn send_packet(&self, packet: &mut ResolvedPacket) -> anyhow::Result<()> {
        let mut write_lock = self.lock_writer().await;
        write_lock.send_resolved_packet(packet).await?;
        drop(write_lock);
        Ok(())
    }
}
