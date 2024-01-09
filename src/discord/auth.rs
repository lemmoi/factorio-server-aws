use ed25519_dalek::{Signature, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

use super::{DiscordAuthError, SignedRequest, VerifyDiscordReq};

pub struct DiscordAuthenticator {
    public_key: VerifyingKey,
}

impl DiscordAuthenticator {
    pub fn new() -> Self {
        DiscordAuthenticator {
            public_key: get_verifying_key(),
        }
    }
}

impl Default for DiscordAuthenticator {
    fn default() -> Self {
        DiscordAuthenticator::new()
    }
}

impl VerifyDiscordReq for DiscordAuthenticator {
    fn verify(&self, event: SignedRequest) -> Result<(), DiscordAuthError> {
        let mut sig_bytes: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
        hex::decode_to_slice(
            event
                .headers
                .get("X-Signature-Ed25519")
                .ok_or(DiscordAuthError::MissingHeaders)?
                .to_str()?,
            &mut sig_bytes,
        )?;
        let sig: Signature = Signature::from_bytes(&sig_bytes);

        let timestamp = event
            .headers
            .get("X-Signature-Timestamp")
            .ok_or(DiscordAuthError::MissingHeaders)?
            .to_str()?;

        Ok(self
            .public_key
            .verify(format!("{}{}", timestamp, event.body).as_bytes(), &sig)?)
    }
}

fn get_verifying_key() -> VerifyingKey {
    let mut key_bytes: [u8; PUBLIC_KEY_LENGTH] = [0; PUBLIC_KEY_LENGTH];
    hex::decode_to_slice(
        "5de3bcb92187f4dcc23ac1b2d2276caa0ccac57ab592f76ee57c4d7f0e692252",
        &mut key_bytes,
    )
    .expect("PK decoding failed");

    VerifyingKey::from_bytes(&key_bytes).expect("Invalid VerifyingKey")
}
