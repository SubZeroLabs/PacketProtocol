use minecraft_data_types::{Encodable, VarInt};
use std::io::{Write, Read};
use flate2::bufread::ZlibEncoder;
use flate2::Compression;

pub trait WritablePacket
    where
        Self: Encodable,
{
    fn write_uncompressed(&self, writer: &mut impl Write) -> anyhow::Result<()>;

    fn to_resolved_packet(&self) -> anyhow::Result<ResolvedPacket>;
}

pub struct ResolvedPacket {
    compression_data: Option<(VarInt, VarInt)>,
    packet_id: VarInt,
    uncompressed_length: VarInt,
    packet: Vec<u8>,
}

impl ResolvedPacket {
    pub fn new(packet_id: VarInt, packet: Vec<u8>) -> anyhow::Result<Self> {
        Ok(Self {
            compression_data: None,
            packet_id,
            uncompressed_length: packet_id.size()? + VarInt::from(packet.len()),
            packet,
        })
    }

    pub fn from_encodable<T: Encodable>(packet_id: VarInt, encodable: &T) -> anyhow::Result<Self> {
        let sized = encodable.size()?;
        let mut packet: Vec<u8> = Vec::with_capacity(sized.into());
        encodable.encode(&mut packet)?;
        ResolvedPacket::new(minecraft_data_types::VarInt::from(packet_id), packet)
    }

    pub fn encode(&mut self, codec: &mut super::encryption::Codec) {
        codec.encrypt(&mut self.packet)
    }

    pub fn compress(&mut self, compression_target: usize) -> anyhow::Result<()> {
        if self.uncompressed_length > compression_target {
            let packet_id_size = self.packet_id.size()?;

            let mut uncompressed_packet: Vec<u8> = Vec::with_capacity(usize::from(packet_id_size) + self.packet.len());
            self.packet_id.encode(&mut uncompressed_packet)?;
            uncompressed_packet.append(&mut self.packet);

            let slice: &[u8] = &uncompressed_packet;
            let mut encoder = ZlibEncoder::new(slice, Compression::default());

            let mut compressed = Vec::new();
            encoder.read_to_end(&mut compressed)?;

            self.compression_data = Some((VarInt::from(compressed.len()) + self.uncompressed_length.size()?, self.uncompressed_length));
            self.packet = compressed;
        } else {
            self.compression_data = Some((self.uncompressed_length.clone() + VarInt::from(1), VarInt::from(0))); // 0-VarInt.size() is always 1 byte
        }
        Ok(())
    }

    pub fn write(&self, writer: &mut impl Write) -> anyhow::Result<()> {
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

    pub fn size(&self) -> anyhow::Result<usize> {
        if let Some((packet_length, _)) = self.compression_data {
            Ok((packet_length.size()? + packet_length).into())
        } else {
            Ok((self.uncompressed_length.size()? + self.uncompressed_length).into())
        }
    }
}

pub struct Packet;

impl Packet {
    pub fn write_packet(
        packet_id: &VarInt,
        packet: &impl Encodable,
        writer: &mut impl Write,
    ) -> anyhow::Result<()> {
        let length = packet_id.size()? + packet.size()?;
        length.encode(writer)?;
        packet_id.encode(writer)?;
        packet.encode(writer)?;
        Ok(())
    }
}
