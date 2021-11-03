use minecraft_data_types::auto_string;
use crate::create_registry;
use minecraft_data_types::nums::VarInt;
use minecraft_data_types::common::{Chat, Identifier};
use rand::rngs::OsRng;
use rsa::{RsaPrivateKey, RsaPublicKey, PublicKeyParts};
use rand::RngCore;
use std::convert::TryFrom;

pub type PublicKeyBytes = (VarInt, Vec<u8>);

auto_string!(ServerId, 20);

create_registry! {
    Disconnect {
        reason: Chat,
        |LocalProtocol => (_) => (0x00);
    }

    EncryptionRequest {
        server_id: ServerId,
        public_key: PublicKeyBytes,
        verify_token: super::VerifyToken,
        |LocalProtocol => (_) => (0x01);
    }

    LoginSuccess {
        uuid: uuid::Uuid,
        username: super::LoginName,
        |LocalProtocol => (_) => (0x02);
    }

    SetCompression {
        threshold: VarInt,
        |LocalProtocol => (_) => (0x03);
    }

    LoginPluginRequest {
        message_id: VarInt,
        channel: Identifier,
        data: Vec<u8>,
        |LocalProtocol => (_) => (0x04);
    }
}

impl EncryptionRequest {
    pub fn new() -> anyhow::Result<(RsaPrivateKey, RsaPublicKey, Self)> {
        let mut rng = OsRng;
        let bits = 1024;
        let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);

        let server_id = "";

        let mut verify_token: Vec<u8> = vec![0; 4];
        rng.fill_bytes(&mut verify_token);

        let pem = rsa_der::public_key_to_der(&public_key.n().to_bytes_be(), &public_key.e().to_bytes_be());

        Ok((
            private_key,
            public_key,
            Self {
                server_id: ServerId::from(server_id),
                public_key: (VarInt::try_from(pem.len())?, pem),
                verify_token: (VarInt::from(4), verify_token),
            },
        ))
    }
}