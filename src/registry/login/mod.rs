use minecraft_data_types::auto_string;
use minecraft_data_types::nums::VarInt;

auto_string!(LoginName, 16);
pub type VerifyToken = (VarInt, Vec<u8>);

pub mod client_bound;
pub mod server_bound;
