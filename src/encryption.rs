use aes::Aes128;
use cfb8::cipher::{AsyncStreamCipher, NewCipher};
use cfb8::Cfb8;
use minecraft_data_types::packets::login::client::{EncryptionRequest, EncryptionRequestServerId};
use minecraft_data_types::packets::login::server::EncryptionResponse;
use minecraft_data_types::VarInt;
use rand::rngs::OsRng;
use rand::{Rng, RngCore};
use rsa::pkcs1::{FromRsaPublicKey, ToRsaPublicKey};
use rsa::{PaddingScheme, PublicKey, RsaPrivateKey, RsaPublicKey};

pub type EncryptionStream = Cfb8<Aes128>;

pub struct Codec {
    encryption_stream: EncryptionStream,
}

impl Codec {
    pub fn new(shared_secret_bytes: &[u8]) -> anyhow::Result<Self> {
        match EncryptionStream::new_from_slices(shared_secret_bytes, shared_secret_bytes) {
            Ok(encryption_stream) => Ok(Codec { encryption_stream }),
            Err(_) => anyhow::bail!("Invalid length for encryption stream."),
        }
    }

    pub fn from_response(
        private_key: &RsaPrivateKey,
        response: &EncryptionResponse,
        verify: &[u8],
    ) -> anyhow::Result<Self> {
        let decrypted_verify =
            private_key.decrypt(PaddingScheme::PKCS1v15Encrypt, &response.verify_token.1)?;

        if verify.ne(&decrypted_verify) {
            anyhow::bail!("Failed to assert verify token match.");
        }

        let decrypted_shared_secret =
            private_key.decrypt(PaddingScheme::PKCS1v15Encrypt, &response.shared_secret.1)?;
        Codec::new(&decrypted_shared_secret)
    }

    pub fn encrypt(&mut self, bytes: &mut [u8]) {
        self.encryption_stream.encrypt(bytes)
    }

    pub fn decrypt(&mut self, bytes: &mut [u8]) {
        self.encryption_stream.decrypt(bytes)
    }

    pub fn client_bound_encryption_request() -> anyhow::Result<(RsaPrivateKey, EncryptionRequest)> {
        let mut rng = OsRng;
        let bits = 1024;
        let private_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
        let public_key = RsaPublicKey::from(&private_key);

        let server_id = "";

        let mut verify_token: Vec<u8> = vec![0; 4];
        rng.fill_bytes(&mut verify_token);

        let pem = Vec::from(public_key.to_pkcs1_der()?.as_ref());

        Ok((
            private_key,
            EncryptionRequest {
                server_id: EncryptionRequestServerId::from(server_id),
                public_key: (VarInt::from(pem.len()), pem),
                verify_token: (VarInt::from(4), verify_token),
            },
        ))
    }

    pub fn server_bound_encryption_response(
        public_key: Vec<u8>,
        verify_token: Vec<u8>,
    ) -> anyhow::Result<(Vec<u8>, EncryptionResponse)> {
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
            EncryptionResponse {
                shared_secret: (
                    VarInt::from(encrypted_shared_secret.len()),
                    encrypted_shared_secret,
                ),
                verify_token: (
                    VarInt::from(encrypted_verify_token.len()),
                    encrypted_verify_token,
                ),
            },
        ))
    }
}
