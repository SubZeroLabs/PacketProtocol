use minecraft_data_types::{Encodable, VarInt};
use std::io::Write;

pub trait WritablePacket
where
    Self: Encodable,
{
    fn write_uncompressed(&self, writer: &mut impl Write) -> anyhow::Result<()>;
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
