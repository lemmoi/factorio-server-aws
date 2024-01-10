use anyhow::Result;
use serde_dynamo::{from_items, to_attribute_value, to_item};
use tracing::info;

use crate::model::{
    domain::StartServerInteraction,
    dynamo::{Command, DiscordInteraction},
};

pub struct DynamoDBAccessor {
    client: aws_sdk_dynamodb::Client,
}

impl DynamoDBAccessor {
    pub fn new(config: &aws_config::SdkConfig) -> Self {
        DynamoDBAccessor {
            client: aws_sdk_dynamodb::Client::new(config),
        }
    }

    pub async fn save_interaction<T: Into<DiscordInteraction>>(&self, item: T) -> Result<()> {
        let item = to_item(item.into())?;
        info!(?item, "Saving item");

        self.client
            .put_item()
            .table_name("discord-interaction-tokens")
            .set_item(Some(item))
            .send()
            .await?;
        Ok(())
    }

    pub async fn delete_interaction<T: Into<DiscordInteraction>>(&self, item: T) -> Result<()> {
        let item: DiscordInteraction = item.into();

        self.client
            .delete_item()
            .table_name("discord-interaction-tokens")
            .key("command", to_attribute_value(&item.command)?)
            .key("timestamp", to_attribute_value(&item.timestamp)?)
            .send()
            .await?;
        Ok(())
    }

    pub async fn get_latest_start(&self) -> Result<Option<StartServerInteraction>> {
        let response = self
            .client
            .query()
            .table_name("discord-interaction-tokens")
            .key_condition_expression("command = :command")
            .expression_attribute_values(":command", to_attribute_value(Command::FactorioStart)?)
            // reverse order with limit of one to get the latest timestamp
            .limit(1)
            .scan_index_forward(false)
            .send()
            .await?;

        let items: Vec<DiscordInteraction> = from_items(response.items().to_vec())?;

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(items.into_iter().nth(0).unwrap().try_into()?))
        }
    }
}
