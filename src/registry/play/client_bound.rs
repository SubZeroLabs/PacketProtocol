use crate::create_registry;
use crate::protocol_version::MCProtocol;
use minecraft_data_types::common::Chat;

create_registry! {
    Disconnect {
        reason: Chat,
        |LocalProtocol => (MCProtocol::V1_17_1) => (0x1A);
    }
}
