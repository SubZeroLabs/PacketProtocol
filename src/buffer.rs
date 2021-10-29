use bytes::{Buf, BufMut, BytesMut};
use minecraft_data_types::{Decodable, Encodable, VarInt};
use std::io::Cursor;

pub enum BufferState {
    Waiting,
    PacketReady,
}

pub struct MinecraftPacketBuffer {
    bytes: BytesMut,
    decoded: BytesMut,
    encryption: Option<crate::encryption::Codec>,
}

impl MinecraftPacketBuffer {
    pub fn new() -> Self {
        MinecraftPacketBuffer {
            bytes: BytesMut::with_capacity(2097151),
            decoded: BytesMut::with_capacity(2097151),
            encryption: None,
        }
    }

    pub fn enable_encryption(&mut self, encryption: crate::encryption::Codec) {
        self.encryption = Some(encryption)
    }

    pub fn inner_buf(&mut self) -> &mut BytesMut {
        &mut self.bytes
    }

    fn is_packet_available(&self) -> bool {
        println!("Polled with available decoded: {:?}", self.decoded);
        let mut cursor: Cursor<&BytesMut> = Cursor::new(&self.decoded);

        if let Ok(length) = VarInt::decode(&mut cursor) {
            (length + length.size().expect("VarInt should have static size.")) <= self.decoded.len()
        } else {
            false
        }
    }

    pub fn poll(&mut self) -> BufferState {
        println!("Incoming bytes from poll: {:?}", &self.bytes);

        let size_read = self
            .bytes
            .len()
            .min(self.decoded.capacity() - self.decoded.len());

        if size_read == 0 {
            return if self.is_packet_available() {
                BufferState::PacketReady
            } else {
                BufferState::Waiting
            };
        }

        let mut read_half = self.bytes.chunks_mut(size_read).next().unwrap();

        if let Some(encryption) = &mut self.encryption {
            println!("Decrypting incoming read");
            encryption.decrypt(&mut read_half);
        }

        self.decoded.put_slice(read_half);

        self.bytes.advance(size_read);

        if self.is_packet_available() {
            BufferState::PacketReady
        } else {
            BufferState::Waiting
        }
    }

    pub fn packet_reader(&mut self) -> Cursor<&[u8]> {
        let cursor = Cursor::new(self.decoded.chunk());
        cursor
    }

    pub fn consume_packet(&mut self, packet_size: usize) {
        self.decoded.advance(packet_size);
    }
}
