pub mod auth;

use lambda_http::http::{HeaderMap, HeaderValue};
use thiserror::Error;

pub struct SignedRequest<'a> {
    pub body: &'a String,
    pub headers: &'a HeaderMap<HeaderValue>,
}

#[derive(Error, Debug)]
pub enum DiscordAuthError {
    #[error("Header was missing")]
    MissingHeaders,
    #[error("Invalid signature")]
    InvalidSignature(#[from] hex::FromHexError),
    #[error("Invalid header")]
    InvalidHeader(#[from] lambda_http::http::header::ToStrError),
    #[error("Request was not verified")]
    NotAuthenticated(#[from] ed25519_dalek::SignatureError),
}

pub trait VerifyDiscordReq {
    fn verify(&self, event: SignedRequest) -> Result<(), DiscordAuthError>;
}
