use crate::create_registry;

create_registry! {
    StatusRequest {
        |LocalProtocol => (_) => (0x00);
    }

    Ping {
        payload: i64,
        |LocalProtocol => (_) => (0x01);
    }
}