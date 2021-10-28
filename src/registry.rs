use minecraft_data_types::simple_auto_enum;
use minecraft_data_types::VarInt;

simple_auto_enum! {
    Test; VarInt {
        0 => Test1,
    }
}