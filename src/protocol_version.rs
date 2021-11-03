use std::fmt::{Display, Formatter, Write};
macro_rules! protocol {
    ($($protocol_identifier:ident => $protocol_version:literal,)*) => {
        #[derive(Copy, Clone)]
        pub enum MCProtocol {
            $(
                $protocol_identifier,
            )*
        }

        impl std::convert::TryFrom<minecraft_data_types::nums::VarInt> for MCProtocol {
            type Error = anyhow::Error;

            fn try_from(protocol_number: minecraft_data_types::nums::VarInt) -> anyhow::Result<MCProtocol> {
                match *protocol_number {
                    $(
                        $protocol_version => Ok(MCProtocol::$protocol_identifier),
                    )*
                    _ => anyhow::bail!("Unsupported protocol {} detected.", protocol_number),
                }
            }
        }

        impl Display for MCProtocol {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        MCProtocol::$protocol_identifier => {
                            f.write_str("MCProtocol(")?;
                            f.write_str(stringify!($protocol_identifier))?;
                            f.write_char(')')
                        },
                    )*
                }
            }
        }
    }
}

protocol! {
    Undefined => 0,
    V1_17_1 => 756,
}

pub trait MapDecodable: Sized {
    fn decode_mapped<R: std::io::Read>(
        protocol: MCProtocol,
        reader: &mut R,
    ) -> anyhow::Result<Self>;
}

pub trait MapEncodable {
    fn encode_mapped<W: std::io::Write>(
        &self,
        protocol: MCProtocol,
        writer: &mut W,
    ) -> anyhow::Result<()>;

    fn size_mapped(
        &self,
        protocol: MCProtocol,
    ) -> anyhow::Result<minecraft_data_types::nums::VarInt>;
}

#[async_trait::async_trait]
pub trait AsyncMapEncodable: MapEncodable {
    async fn encode_mapped_async<W: tokio::io::AsyncWrite + Send + Unpin>(
        &self,
        protocol: MCProtocol,
        writer: &mut W,
    ) -> anyhow::Result<()>;
}
