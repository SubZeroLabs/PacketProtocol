use crate::create_registry;
use minecraft_data_types::auto_string;

auto_string!(JSONResponse, 32767);

create_registry! {
    StatusResponse {
        json_response: JSONResponse,
        |LocalProtocol => (_) => (0x00);
    }
    Pong {
        payload: i64,
        |LocalProtocol => (_) => (0x01);
    }
}