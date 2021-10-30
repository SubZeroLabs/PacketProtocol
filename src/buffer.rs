use bytes::{Buf, BufMut, BytesMut};
use minecraft_data_types::VarInt;
use std::io::{Cursor, Read};
use flate2::bufread::ZlibDecoder;

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

impl MinecraftPacketBuffer {
    pub fn new() -> Self {
        MinecraftPacketBuffer {
            bytes: BytesMut::with_capacity(2097151 + 3),
            decoded: BytesMut::with_capacity(2097151 + 3),
            decryption: None,
            decompressing: false,
        }
    }

    pub fn len(&self) -> (usize, usize) {
        (self.bytes.len(), self.decoded.len())
    }

    pub fn enable_decryption(&mut self, decryption: crate::encryption::Codec) {
        self.decryption = Some(decryption)
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
                BufferState::Error(String::from("Next packet was too big to decode, something went wrong."))
            } else {
                BufferState::Waiting
            };
        }

        log::debug!("Polling {} with {} in decoded.", size_read, self.decoded.len());

        let read_half = self.bytes.chunks_mut(size_read).next().unwrap();

        if let Some(encryption) = &mut self.decryption {
            encryption.decrypt(read_half);
        }

        self.decoded.put_slice(read_half);

        self.bytes.advance(size_read);

        if self.is_packet_available() {
            BufferState::PacketReady
        } else {
            BufferState::Waiting
        }
    }

    pub fn packet_reader(&mut self) -> anyhow::Result<Cursor<Vec<u8>>> {
        let mut cursor = Cursor::new(self.decoded.chunk());
        let (length_size, length) = VarInt::decode_and_size(&mut cursor)?;
        self.decoded.advance(length_size.into());
        let mut cursor: Cursor<Vec<u8>> = Cursor::new(self.decoded.chunks(length.into()).next().unwrap().to_vec());

        let cursor = if self.decompressing {
            let (decompressed_length_size, decompressed_length) = VarInt::decode_and_size(&mut cursor)?;
            let remaining_bytes = &cursor.into_inner()[decompressed_length_size.into()..];
            if decompressed_length == 0 {
                Cursor::new(Vec::from(remaining_bytes))
            } else {
                let mut target = Vec::with_capacity(decompressed_length.into());
                ZlibDecoder::new(remaining_bytes).read_to_end(&mut target)?;
                Cursor::new(target)
            }
        } else {
            cursor
        };
        self.decoded.advance(length.into());
        Ok(cursor)
    }
}

impl Default for MinecraftPacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}
