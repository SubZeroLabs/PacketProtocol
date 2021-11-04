use aes::Aes128;
use cfb8::cipher::{AsyncStreamCipher, NewCipher};
use cfb8::Cfb8;

pub type EncryptionStream = Cfb8<Aes128>;

pub struct Codec {
    encryption_stream: EncryptionStream,
}

impl Codec {
    pub fn new(shared_secret_bytes: &[u8]) -> anyhow::Result<(Self, Self)> {
        let (stream_read, stream_write) = (
            EncryptionStream::new_from_slices(shared_secret_bytes, shared_secret_bytes),
            EncryptionStream::new_from_slices(shared_secret_bytes, shared_secret_bytes)
        );
        match (stream_read, stream_write) {
            (Ok(stream_read), Ok(stream_write)) => {
                Ok((Codec { encryption_stream: stream_read }, Codec { encryption_stream: stream_write }))
            }
            (Err(error), Ok(_)) => {
                anyhow::bail!("Failed to create read stream {}.", error);
            }
            (Ok(_), Err(error)) => {
                anyhow::bail!("Failed to create write stream {}.", error);
            }
            (Err(error), Err(error2)) => {
                anyhow::bail!("Failed to create both streams {}, {}.", error, error2);
            }
        }
    }

    pub fn from_response(
        response_verify: &[u8],
        shared_secret: &[u8],
        verify: &[u8],
    ) -> anyhow::Result<(Self, Self)> {
        if verify.ne(response_verify) {
            anyhow::bail!("Failed to assert verify token match.");
        }
        Codec::new(&shared_secret)
    }

    pub fn encrypt(&mut self, bytes: &mut [u8]) {
        self.encryption_stream.encrypt(bytes)
    }

    pub fn decrypt(&mut self, bytes: &mut [u8]) {
        self.encryption_stream.decrypt(bytes)
    }
}
