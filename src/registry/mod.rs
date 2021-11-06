use crate::protocol_version::{MCProtocol, MapDecodable};
use anyhow::Context;

#[cfg(feature = "handshake")]
pub mod handshake;
#[cfg(feature = "login")]
pub mod login;
#[cfg(feature = "status")]
pub mod status;

pub trait LazyHandle<T: MapDecodable> {
    fn decode_type(self) -> anyhow::Result<T>;

    fn pass_bytes<W: std::io::Write>(self, writer: &mut W) -> anyhow::Result<()>;

    fn consume_bytes(self) -> anyhow::Result<()>;
}

pub struct SimpleLazyHandle {
    bytes: std::io::Cursor<Vec<u8>>,
    protocol: MCProtocol,
}

impl SimpleLazyHandle {
    pub fn new(bytes: std::io::Cursor<Vec<u8>>, protocol: MCProtocol) -> Self {
        SimpleLazyHandle { bytes, protocol }
    }
}

impl<T: MapDecodable> LazyHandle<T> for SimpleLazyHandle {
    fn decode_type(mut self) -> anyhow::Result<T> {
        T::decode_mapped(self.protocol, &mut self.bytes)
    }

    fn pass_bytes<W: std::io::Write>(self, writer: &mut W) -> anyhow::Result<()> {
        writer
            .write_all(&self.bytes.into_inner())
            .context("Failed to pass through bytes to writer.")
    }

    fn consume_bytes(self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait RegistryBase<H: Send + Sync> {
    async fn handle_packet(
        handler: &mut H,
        mut packet_cursor: std::io::Cursor<Vec<u8>>,
        target_protocol: MCProtocol,
    ) -> anyhow::Result<()>;
}

#[macro_export]
macro_rules! strict_enum {
    ($($enum_name:ident; $index_type:ty { $($byte_representation:literal => $option_name:ident;)* })*) => {
        $(
            #[derive(Debug)]
            pub enum $enum_name {
                $($option_name,)*
            }

            impl minecraft_data_types::encoder::Decodable for $enum_name {
                fn decode<R: std::io::Read>(reader: &mut R) -> anyhow::Result<Self> {
                    let index = <$index_type>::decode(reader)?;
                    match *index {
                        $(
                            $byte_representation => Ok($enum_name::$option_name),
                        )*
                        _ => anyhow::bail!("Failed to decode index {} for {}.", index, stringify!($enum_name)),
                    }
                }
            }

            impl minecraft_data_types::encoder::Encodable for $enum_name {
                fn encode<W: std::io::Write>(&self, writer: &mut W) -> anyhow::Result<()> {
                    match self {
                        $(
                            $enum_name::$option_name => {
                                <$index_type>::encode(&<$index_type>::from($byte_representation), writer)?;
                                Ok(())
                            }
                        )*
                    }
                }

                fn size(&self) -> anyhow::Result<minecraft_data_types::nums::VarInt> {
                    match self {
                        $(
                            $enum_name::$option_name => {
                                let size = <$index_type>::size(&<$index_type>::from($byte_representation))?;
                                Ok(size)
                            }
                        )*
                    }
                }
            }

            #[async_trait::async_trait]
            impl minecraft_data_types::encoder::AsyncEncodable for $enum_name {
                async fn async_encode<W: tokio::io::AsyncWrite + Send + Unpin>(
                    &self,
                    writer: &mut W,
                ) -> anyhow::Result<()> {
                    match self {
                        $(
                            $enum_name::$option_name => {
                                <$index_type>::async_encode(&<$index_type>::from($byte_representation), writer).await?;
                                Ok(())
                            }
                        )*
                    }
                }
            }
        )*
    };
}

#[macro_export]
macro_rules! create_registry {
    (
        $(
            $packet_name:ident {
                $($field_name:ident: $field_type:ty,)*
                |LocalProtocol => ($($protocol:tt)+) $(+($($protocol_ext:tt)+))* => ($packet_id:literal);
                $(|Protocol {
                    $(
                        ($($extra_protocol:tt)+) $(+($($extra_protocol_ext:tt)+))* => ($proto_packet_id:literal) {
                            $($parse_ident:ident: $parse_type:ty => ($($parse_mapper:tt)+);)*
                            ($constructor:expr)
                        }
                    )*
                })?
            }
        )*
    ) => {
        use minecraft_data_types::encoder::*;
        use anyhow::Context;
        $(
            #[derive(Debug)]
            pub struct $packet_name {$(
                pub $field_name: $field_type,
            )*}

            impl $crate::protocol_version::MapDecodable for $packet_name {
                fn decode_mapped<R: std::io::Read>(protocol: $crate::protocol_version::MCProtocol, _reader: &mut R) -> anyhow::Result<Self> {
                    match protocol {
                        $($protocol)* $(| $($protocol_ext)*)* => {
                            $(
                                let $field_name = <$field_type>::decode(_reader)
                                    .context(format!(
                                        "Failed to decode field {} of packet {}.",
                                        stringify!($field_name),
                                        stringify!($packet_name)
                                    ))?;
                            )*
                            Ok(Self {
                                $($field_name,)*
                            })
                        },
                        $($(
                            $($extra_protocol)* $(| $($extra_protocol_ext)*)* => {
                                $(
                                    let $parse_ident = <$parse_type>::decode(_reader)
                                        .context(format!(
                                            "Failed to decode field {} of packet {} for protocol {}.",
                                            stringify!($parse_ident),
                                            stringify!($packet_name),
                                            protocol
                                        ))?;
                                )*
                                $constructor
                            },
                        )*)*
                    }
                }
            }

            impl $crate::protocol_version::MapEncodable for $packet_name {
                fn encode_mapped<W: std::io::Write>(&self, protocol: $crate::protocol_version::MCProtocol, _writer: &mut W) -> anyhow::Result<()> {
                    match protocol {
                        $($protocol)* $(| $($protocol_ext)*)* => {
                            $(
                                self.$field_name.encode(_writer)
                                    .context(format!(
                                        "Failed to encode field {} of packet {}.",
                                        stringify!($field_name),
                                        stringify!($packet_name)
                                    ))?;
                            )*
                            Ok(())
                        }
                        $($(
                            $($extra_protocol)* $(| $($extra_protocol_ext)*)* => {
                                $(
                                    let $parse_ident = &self.$parse_ident;
                                    $($parse_mapper)*.encode(_writer)
                                        .context(format!(
                                            "Failed to encode field {} of packet {} for protocol {}.",
                                            stringify!($parse_ident),
                                            stringify!($packet_name),
                                            protocol
                                        ))?;
                                )*
                                Ok(())
                            },
                        )*)*
                    }
                }

                #[allow(unused_mut)]
                fn size_mapped(&self, protocol: $crate::protocol_version::MCProtocol) -> anyhow::Result<minecraft_data_types::nums::VarInt> {
                    match protocol {
                        $($protocol)* $(| $($protocol_ext)*)* => {
                            let mut accum = minecraft_data_types::nums::VarInt::from(0);
                            $(
                                accum += self.$field_name.size()
                                    .context(format!(
                                        "Failed to size field {} of packet {}.",
                                        stringify!($field_name),
                                        stringify!($packet_name)
                                    ))?;
                            )*
                            Ok(accum)
                        }
                        $($(
                            $($extra_protocol)* $(| $($extra_protocol_ext)*)* => {
                                let mut accum = minecraft_data_types::nums::VarInt::from(0);
                                $(
                                    let $parse_ident = &self.$parse_ident;
                                    accum += $($parse_mapper)*.size()
                                        .context(format!(
                                            "Failed to size field {} of packet {} for protocol {}.",
                                            stringify!($parse_ident),
                                            stringify!($packet_name),
                                            protocol
                                        ))?;
                                )*
                                Ok(accum)
                            }
                        )*)*
                    }
                }
            }

            impl $crate::packet::WritablePacket for $packet_name {
                #[allow(unused_mut)]
                fn to_resolved_packet(&self, protocol: $crate::protocol_version::MCProtocol) -> anyhow::Result<$crate::packet::ResolvedPacket> {
                    match protocol {
                        $($protocol)* $(| $($protocol_ext)*)* => {
                            let mut vec: Vec<u8> = Vec::new();
                            $(
                                self.$field_name.encode(&mut vec)
                                    .context(format!(
                                        "Failed to encode field {} of packet {}.",
                                        stringify!($field_name),
                                        stringify!($packet_name)
                                    ))?;
                            )*
                            $crate::packet::ResolvedPacket::new(minecraft_data_types::nums::VarInt::from($packet_id), vec)
                        }
                        $($(
                            $($extra_protocol)* $(| $($extra_protocol_ext)*)* => {
                                let mut vec: Vec<u8> = Vec::new();
                                $(
                                    let $parse_ident = &self.$parse_ident;
                                    $($parse_mapper)*.encode(&mut vec)
                                        .context(format!(
                                            "Failed to encode field {} of packet {} for protocol {}.",
                                            stringify!($parse_ident),
                                            stringify!($packet_name),
                                            protocol
                                        ))?;
                                )*
                                $crate::packet::ResolvedPacket::new(minecraft_data_types::nums::VarInt::from($proto_packet_id), vec)
                            },
                        )*)*
                    }
                }
            }

            #[async_trait::async_trait]
            impl $crate::protocol_version::AsyncMapEncodable for $packet_name {
                async fn encode_mapped_async<W: tokio::io::AsyncWrite + Send + Unpin>(&self, protocol: $crate::protocol_version::MCProtocol, _writer: &mut W) -> anyhow::Result<()> {
                    match protocol {
                        $($protocol)* $(| $($protocol_ext)*)* => {
                            $(
                                self.$field_name.async_encode(_writer)
                                    .await
                                    .context(format!(
                                        "Failed to encode field {} of packet {}.",
                                        stringify!($field_name),
                                        stringify!($packet_name)
                                    ))?;
                            )*
                            Ok(())
                        }
                        $($(
                            $($extra_protocol)* $(| $($extra_protocol_ext)*)* => {
                                $(
                                    let $parse_ident = &self.$parse_ident;
                                    $($parse_mapper)*.async_encode(_writer)
                                        .await
                                        .context(format!(
                                            "Failed to encode field {} of packet {} for protocol {}.",
                                            stringify!($parse_ident),
                                            stringify!($packet_name),
                                            protocol
                                        ))?;
                                )*
                                Ok(())
                            },
                        )*)*
                    }
                }
            }
        )*
        pub struct Registry;

        #[async_trait::async_trait]
        impl<H: RegistryHandler> $crate::registry::RegistryBase<H> for Registry {
            async fn handle_packet(
                handler: &mut H, mut packet_cursor: std::io::Cursor<Vec<u8>>,
                target_protocol: $crate::protocol_version::MCProtocol
            ) -> anyhow::Result<()> {
                let packet_id = minecraft_data_types::nums::VarInt::decode(&mut packet_cursor)?;
                paste::paste! {
                    match (target_protocol, *packet_id) {
                        $(
                            ($($protocol)* $(| $($protocol_ext)*)*, $packet_id) => {
                                handler.[<handle_$packet_name:snake>]($crate::registry::SimpleLazyHandle::new(packet_cursor, target_protocol)).await
                            }
                            $($(
                                ($($extra_protocol)* $(| $($extra_protocol_ext)*)*, $proto_packet_id) => {
                                    handler.[<handle_$packet_name:snake>]($crate::registry::SimpleLazyHandle::new(packet_cursor, target_protocol)).await
                                }
                            )*)*
                        )*
                        (_, _) => {
                            handler.handle_unknown(packet_cursor).await
                        }
                    }
                }
            }
        }
        paste::paste! {
            #[async_trait::async_trait]
            pub trait RegistryHandler: Send + Sync {
                async fn handle_unknown(&mut self, packet_cursor: std::io::Cursor<Vec<u8>>);

                async fn handle_default<T: crate::protocol_version::MapDecodable, H: $crate::registry::LazyHandle<T> + Send>(
                    &mut self, handle: H
                ) -> anyhow::Result<()>;
                $(
                    async fn [<handle_$packet_name:snake>]<H: $crate::registry::LazyHandle<$packet_name> + Send>(
                        &mut self,
                        handle: H
                    ) -> anyhow::Result<()> {
                        Self::handle_default(self, handle).await
                    }
                )*
            }
        }
    }
}
