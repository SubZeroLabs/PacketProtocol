use crate::create_registry;
use crate::protocol_version::MCProtocol;
use minecraft_data_types::{nums::VarInt, common::{Chat, Identifier}};
use commander::protocol::Node;

create_registry! {
    DeclareCommands {
        nodes: (VarInt, Vec<Node>),
        root_index: VarInt,
        |LocalProtocol => (MCProtocol::V1_17_1) => (0x12);
        |Protocol {
            (_) => (0x12) {
                (anyhow::bail!("Unsupported version."))
            }
        }
    }

    PluginMessage {
        channel: Identifier,
        data: Vec<u8>,
        |LocalProtocol => (MCProtocol::V1_17_1) => (0x18);
        |Protocol {
            (_) => (0x18) {
                (anyhow::bail!("Unsupported version."))
            }
        }
    }

    Disconnect {
        reason: Chat,
        |LocalProtocol => (MCProtocol::V1_17_1) => (0x1A);
        |Protocol {
            (_) => (0x1A) {
                (anyhow::bail!("Unsupported version."))
            }
        }
    }
}
