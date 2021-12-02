use crate::create_registry;
use minecraft_data_types::common::Identifier;
use crate::protocol_version::MCProtocol;

create_registry! {
    PluginMessage {
        channel: Identifier,
        data: Vec<u8>,
        |LocalProtocol => (MCProtocol::V1_17_1) + (MCProtocol::V1_18) => (0x0A);
        |Protocol {
            (_) => (0x0A) {
                (anyhow::bail!("Unsupported version."))
            }
        }
    }
}
