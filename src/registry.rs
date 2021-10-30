use crate::packet::PacketAllocator;
use anyhow::Context;
use minecraft_data_types::{auto_enum, packets::*, Decodable};
use paste::paste;
use std::io::{Seek, Write};
use async_trait::async_trait;

pub trait LazyHandle<T>
where
    T: Decodable,
{
    fn decode_type(self) -> anyhow::Result<T>;

    fn pass_bytes(self, writer: &mut impl std::io::Write) -> anyhow::Result<()>;

    fn consume_bytes(self) -> anyhow::Result<()>;
}

pub struct SimpleLazyHandle {
    bytes: std::io::Cursor<Vec<u8>>,
}

impl SimpleLazyHandle {
    fn new(bytes: std::io::Cursor<Vec<u8>>) -> Self {
        SimpleLazyHandle { bytes }
    }
}

impl<T> LazyHandle<T> for SimpleLazyHandle
where
    T: Decodable,
{
    fn decode_type(mut self) -> anyhow::Result<T> {
        T::decode(&mut self.bytes)
    }

    fn pass_bytes(mut self, writer: &mut impl Write) -> anyhow::Result<()> {
        (&mut self.bytes)
            .rewind()
            .context("Failed to rewind bytes.")?;
        writer
            .write_all(&self.bytes.into_inner())
            .context("Failed to pass through bytes to writer.")
    }

    fn consume_bytes(mut self) -> anyhow::Result<()> {
        let length = {
            self.bytes.stream_len()?
        };
        self.bytes.set_position(length);
        Ok(())
    }
}

macro_rules! registry {
    ($($registry_name:ident { $($packet_id:literal => $enum_ident:ident: $packet_type:ty,)* })*) => {
        $(
            auto_enum! {
                $registry_name; minecraft_data_types::VarInt {
                    $(
                        $packet_id => $enum_ident: $packet_type,
                    )*
                }
            }

            $(
                impl $crate::packet::WritablePacket for $packet_type {
                    fn write_uncompressed(&self, writer: &mut impl Write) -> anyhow::Result<()> {
                        $crate::packet::UncompressedPacket::write_packet(&minecraft_data_types::VarInt::from($packet_id), self, writer)
                    }
                }
            )*

            paste! {
                impl $registry_name {
                    pub async fn read_packet<T: $crate::packet::PacketAllocator, H: [<$registry_name Handler>] + std::marker::Send, R: std::io::Read>(handler: &mut H, reader: &mut R) -> anyhow::Result<()> {
                        let uncompressed_packet = T::decode(reader)?;
                        let (packet_id, data_cursor) = uncompressed_packet.into_packet_cursor(reader)?;
                        let lazy_handler = SimpleLazyHandle::new(data_cursor);
                        match packet_id.into() {
                            $(
                                $packet_id => {
                                    [<$registry_name Handler>]::[<handle_$enum_ident:snake>](handler, lazy_handler).await
                                }
                            )*
                            _ => {
                                anyhow::bail!("Unknown packet ID {} found while decoding packet.", &packet_id);
                            }
                        }
                    }
                }
                #[async_trait]
                pub trait [<$registry_name Handler>] {
                    async fn handle_default<T: minecraft_data_types::Decodable>(&mut self, handle: impl LazyHandle<T> + std::marker::Send + 'async_trait) -> anyhow::Result<()>;
                    $(
                        async fn [<handle_$enum_ident:snake>](&mut self, handle: impl LazyHandle<$packet_type> + std::marker::Send + 'async_trait) -> anyhow::Result<()> {
                            Self::handle_default(self, handle).await
                        }
                    )*
                }
            }
        )*
    }
}

registry! {
    HandshakingServerBoundRegistry {
        0 => Handshake: handshaking::server::Handshake,
    }

    StatusClientBoundRegistry {
        0 => StatusResponse: status::client::StatusResponse,
        1 => Pong: status::client::Pong,
    }

    StatusServerBoundRegistry {
        0 => StatusRequest: status::server::StatusRequest,
        1 => Ping: status::server::Ping,
    }

    LoginClientBoundRegistry {
        0 => Disconnect: login::client::Disconnect,
        1 => EncryptionRequest: login::client::EncryptionRequest,
        2 => LoginSuccess: login::client::LoginSuccess,
        3 => SetCompression: login::client::SetCompression,
        4 => LoginPluginRequest: login::client::LoginPluginRequest,
    }

    LoginServerBoundRegistry {
        0 => LoginStart: login::server::LoginStart,
        1 => EncryptionResponse: login::server::EncryptionResponse,
        2 => LoginPluginResponse: login::server::LoginPluginResponse,
    }
}
