use crate::create_registry;
use minecraft_data_types::nums::VarInt;
use rand::rngs::OsRng;
use rand::RngCore;
use rsa::pkcs1::FromRsaPublicKey;
use rsa::{PaddingScheme, PublicKey, RsaPublicKey};
use std::convert::TryFrom;

pub type SharedSecret = (VarInt, Vec<u8>);

create_registry! {
    LoginStart {
        name: super::LoginName,
        |LocalProtocol => (_) => (0x00);
    }

    EncryptionResponse {
        shared_secret: SharedSecret,
        verify_token: super::VerifyToken,
        |LocalProtocol => (_) => (0x01);
    }

    LoginPluginResponse {
        message_id: VarInt,
        successful: bool,
        data: Vec<u8>,
        |LocalProtocol => (_) => (0x02);
    }
}

impl EncryptionResponse {
    pub fn new(public_key: &[u8], verify_token: &[u8]) -> anyhow::Result<(Vec<u8>, Self)> {
        let public_key = RsaPublicKey::from_pkcs1_der(&public_key)?;

        let mut rng = OsRng;

        let encrypted_verify_token =
            public_key.encrypt(&mut rng, PaddingScheme::PKCS1v15Encrypt, &verify_token)?;

        let mut shared_secret = vec![0; 16];
        rng.fill_bytes(&mut shared_secret);
        let encrypted_shared_secret =
            public_key.encrypt(&mut rng, PaddingScheme::PKCS1v15Encrypt, &shared_secret)?;

        Ok((
            shared_secret,
            Self {
                shared_secret: (
                    VarInt::try_from(encrypted_shared_secret.len())?,
                    encrypted_shared_secret,
                ),
                verify_token: (
                    VarInt::try_from(encrypted_verify_token.len())?,
                    encrypted_verify_token,
                ),
            },
        ))
    }
}
