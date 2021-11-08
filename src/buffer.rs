use bytes::{Buf, BufMut, BytesMut};
use flate2::bufread::ZlibDecoder;
use minecraft_data_types::nums::VarInt;
use std::convert::TryInto;
use std::io::{Cursor, Read};

pub enum BufferState {
    Waiting,
    PacketReady,
    Error(String),
}

pub struct MinecraftPacketBuffer {
    bytes: BytesMut,
    decoded: BytesMut,
    decryption: Option<crate::encryption::Codec>,
    decompressing: bool,
}

const BUFFER_CAPACITY: usize = 2097151 + 3;

impl MinecraftPacketBuffer {
    pub fn new() -> Self {
        MinecraftPacketBuffer {
            bytes: BytesMut::with_capacity(BUFFER_CAPACITY),
            decoded: BytesMut::with_capacity(BUFFER_CAPACITY),
            decryption: None,
            decompressing: false,
        }
    }

    pub fn len(&self) -> (usize, usize) {
        (self.bytes.len(), self.decoded.len())
    }

    pub fn enable_decryption(&mut self, codec: crate::encryption::Codec) {
        self.decryption = Some(codec);
    }

    pub fn enable_decompression(&mut self) {
        self.decompressing = true;
    }

    pub fn inner_buf(&mut self) -> &mut BytesMut {
        &mut self.bytes
    }

    fn is_packet_available(&self) -> bool {
        let mut cursor: Cursor<&[u8]> = Cursor::new(self.decoded.chunk());

        if let Ok((size, length)) = VarInt::decode_and_size(&mut cursor) {
            (length + size) <= self.decoded.len()
        } else {
            false
        }
    }

    pub fn poll(&mut self) -> BufferState {
        let size_read = self
            .bytes
            .len()
            .min(self.decoded.capacity() - self.decoded.len());

        if size_read == 0 {
            return if self.is_packet_available() {
                BufferState::PacketReady
            } else if self.decoded.capacity() == self.decoded.len() {
                log::debug!("Too Big Error, Failed at: Capacity {}, length {}", self.decoded.capacity(), self.decoded.len());
                BufferState::Error(String::from(
                    "Next packet was too big to decode, something went wrong.",
                ))
            } else {
                BufferState::Waiting
            };
        }

        log::trace!(
            "Polling {} with {} in decoded.",
            size_read,
            self.decoded.len()
        );

        let read_half = self.bytes.chunks_mut(size_read).next().unwrap();

        if let Some(codec) = &mut self.decryption {
            codec.decrypt(read_half);
        }

        self.decoded.put_slice(read_half);

        self.bytes.advance(size_read);
        self.bytes.reserve(BUFFER_CAPACITY - self.bytes.len());

        if self.is_packet_available() {
            BufferState::PacketReady
        } else {
            BufferState::Waiting
        }
    }

    pub fn packet_reader(&mut self) -> anyhow::Result<Cursor<Vec<u8>>> {
        let mut cursor = Cursor::new(self.decoded.chunk());
        let (length_size, length) = VarInt::decode_and_size(&mut cursor)?;
        self.decoded.advance(length_size.try_into()?);
        let mut cursor: Cursor<Vec<u8>> = Cursor::new(
            self.decoded
                .chunks(length.try_into()?)
                .next()
                .unwrap()
                .to_vec(),
        );

        let cursor = if self.decompressing {
            let (decompressed_length_size, decompressed_length) =
                VarInt::decode_and_size(&mut cursor)?;
            let remaining_bytes = &cursor.into_inner()[decompressed_length_size.try_into()?..];
            if decompressed_length == 0 {
                Cursor::new(Vec::from(remaining_bytes))
            } else {
                let mut target = Vec::with_capacity(decompressed_length.try_into()?);
                ZlibDecoder::new(remaining_bytes).read_to_end(&mut target)?;
                Cursor::new(target)
            }
        } else {
            cursor
        };
        self.decoded.advance(length.try_into()?);
        self.decoded.reserve(BUFFER_CAPACITY - self.decoded.len());
        Ok(cursor)
    }
}

impl Default for MinecraftPacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}
