use std::ops::Add;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::domain::StartServerInteraction;
use time::{ext::NumericalDuration, OffsetDateTime};

#[derive(Serialize, Deserialize)]
pub enum Command {
    FactorioStart,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordInteraction {
    pub command: Command,
    pub timestamp: i64,
    token: String,
    ttl: i64,
}

impl From<StartServerInteraction> for DiscordInteraction {
    fn from(value: StartServerInteraction) -> Self {
        DiscordInteraction {
            command: Command::FactorioStart,
            timestamp: value.timestamp.unix_timestamp(),
            token: value.token,
            ttl: value.timestamp.add(15.minutes()).unix_timestamp(),
        }
    }
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("Could not deserialize")]
    Error,
}

impl TryInto<StartServerInteraction> for DiscordInteraction {
    type Error = DeserializeError;

    fn try_into(self) -> Result<StartServerInteraction, Self::Error> {
        if !matches!(self.command, Command::FactorioStart) {
            Err(DeserializeError::Error)
        } else {
            Ok(StartServerInteraction {
                timestamp: OffsetDateTime::from_unix_timestamp(self.timestamp)
                    .expect("Invalid unix timestamp"),
                token: self.token,
            })
        }
    }
}
