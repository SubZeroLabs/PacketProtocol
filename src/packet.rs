use minecraft_data_types::{Encodable, VarInt};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};

pub trait WritablePacket
where
    Self: Encodable,
{
    fn write_uncompressed(&self, writer: &mut impl Write) -> anyhow::Result<()>;
}

pub trait PacketAllocator {
    fn into_packet_cursor(
        self,
        reader: &mut impl Read,
    ) -> anyhow::Result<(VarInt, std::io::Cursor<Vec<u8>>)>;

    fn write_packet(
        packet_id: &VarInt,
        packet: &impl Encodable,
        writer: &mut impl Write,
    ) -> anyhow::Result<()>;
}

pub struct UncompressedPacket {
    length: VarInt,
    packet_id: VarInt,
}

impl minecraft_data_types::Decodable for UncompressedPacket {
    fn decode(reader: &mut impl Read) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let length = VarInt::decode(reader)?;
        let packet_id = VarInt::decode(reader)?;
        Ok(UncompressedPacket { length, packet_id })
    }
}

impl PacketAllocator for UncompressedPacket {
    fn into_packet_cursor(
        self,
        reader: &mut impl Read,
    ) -> anyhow::Result<(VarInt, Cursor<Vec<u8>>)> {
        let mut cursor = Cursor::new(Vec::new());
        self.length.encode(&mut cursor)?;
        let length_size = cursor.position();
        self.packet_id.encode(&mut cursor)?;
        let id_size = cursor.position() - length_size;
        let mut vec =
            vec![0; (*self.length as usize) - (cursor.position() as usize - length_size as usize)];
        reader.read_exact(&mut vec)?;
        cursor.write_all(&mut vec)?;
        cursor.seek(SeekFrom::Start(length_size + id_size))?;
        Ok((self.packet_id, cursor))
    }

    fn write_packet(
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
