use crate::protocol_version::{MCProtocol, MapEncodable};
use anyhow::Context;
use flate2::bufread::ZlibEncoder;
use flate2::Compression;
use minecraft_data_types::{encoder::*, nums::VarInt};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Formatter, Display};
use std::io::{Read, Write};
use tokio::io::AsyncWriteExt;

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

    pub fn compress(&mut self, compression_threshold: usize) -> anyhow::Result<()> {
        if self.uncompressed_length > compression_threshold {
            let packet_id_size = self.packet_id.size()?;

            let mut uncompressed_packet: Vec<u8> =
                Vec::with_capacity(usize::try_from(packet_id_size)? + self.packet.len());
            self.packet_id.encode(&mut uncompressed_packet)?;
            uncompressed_packet.append(&mut self.packet);

            let slice: &[u8] = &uncompressed_packet;
            let mut encoder = ZlibEncoder::new(slice, Compression::default());

            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed)?;

            self.compression_data = Some((
                VarInt::try_from(compressed.len())? + self.uncompressed_length.size()?,
                self.uncompressed_length,
            ));
            self.packet = compressed;
        } else {
            self.compression_data =
                Some((self.uncompressed_length + VarInt::from(1), VarInt::from(0)));
            // 0-VarInt.size() is always 1 byte
        }
        Ok(())
    }

    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        if let Some((packet_length, data_length)) = self.compression_data {
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
}

impl<T: tokio::io::AsyncWrite + Send + Sync + Sized + Unpin> PacketWriter<T> {
    pub fn new(internal_writer: T) -> Self {
        PacketWriter { internal_writer, codec: None }
    }

    pub async fn send_resolved_packet(&mut self, packet: &ResolvedPacket) -> anyhow::Result<()> {
        if let Some(codec) = &mut self.codec {
            let mut buf = Vec::with_capacity(packet.size()?);
            packet.write_async(&mut buf).await?;
            codec.encrypt(&mut buf);
            self.internal_writer.write_all(&buf).await.context("Failed to write encoded packet.")
        } else {
            packet.write_async(&mut self.internal_writer).await.context("Failed to write non-encoded packet.")
        }
    }
}
