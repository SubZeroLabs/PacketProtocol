use crate::create_registry;
use crate::strict_enum;
use minecraft_data_types::auto_string;

auto_string!(ServerAddress, 32767);

strict_enum! {
    NextState; minecraft_data_types::nums::VarInt {
        1 => Status;
        2 => Login;
    }
}

create_registry! {
    Handshake {
        protocol_version: minecraft_data_types::nums::VarInt,
        server_address: ServerAddress,
        server_port: u16,
        next_state: NextState,
        |LocalProtocol => (_) => (0x00);
    }
}
