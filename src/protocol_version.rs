use std::fmt::{Debug, Formatter, Write};
macro_rules! protocol {
    ($($string_name:literal => $protocol_version:literal as $protocol_identifier:ident,)*) => {
        #[derive(Copy, Clone)]
        pub enum MCProtocol {
            $(
                $protocol_identifier,
            )*
            Illegal(minecraft_data_types::nums::VarInt),
        }

        impl MCProtocol {
            pub fn as_i32(self) -> i32 {
                match self {
                    $(
                        MCProtocol::$protocol_identifier => $protocol_version,
                    )*
                    MCProtocol::Illegal(number) => number.into(),
                }
            }
        }

        impl ToString for MCProtocol {
            fn to_string(&self) -> std::string::String {
                match self {
                    $(MCProtocol::$protocol_identifier => $string_name.to_string(),)*
                    _ => String::from("Unknown"),
                }
            }
        }

        impl From<minecraft_data_types::nums::VarInt> for MCProtocol {
            fn from(protocol_number: minecraft_data_types::nums::VarInt) -> MCProtocol {
                match *protocol_number {
                    $(
                        $protocol_version => MCProtocol::$protocol_identifier,
                    )*
                    _ => MCProtocol::Illegal(protocol_number),
                }
            }
        }

        impl Debug for MCProtocol {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        MCProtocol::$protocol_identifier => {
                            f.write_str("MCProtocol(")?;
                            f.write_str($string_name)?;
                            f.write_char(')')
                        },
                    )*
                    MCProtocol::Illegal(number) => {
                        f.write_str("MCProtocol(")?;
                        f.write_str(&format!("{}", number))?;
                        f.write_char(')')
                    }
                }
            }
        }
    }
}

protocol! {
    "Undefined" => 0 as Undefined,
    "1.17.1" => 756 as V1_17_1,
    "1.18" => 757 as V1_18,
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
