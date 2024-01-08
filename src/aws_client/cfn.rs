use anyhow::Result;
use serde_json::{json, Value};
use tracing::{info, instrument};

use super::ServerUpdater;

use aws_sdk_cloudformation::{
    error::{ProvideErrorMetadata, SdkError},
    types::Parameter,
};

#[derive(Debug)]
pub struct CfnAccessor {
    client: aws_sdk_cloudformation::Client,
}

#[derive(Debug)]
enum ServerState {
    Running,
    Stopped,
}

impl ServerState {
    fn as_template_value(&self) -> &str {
        match self {
            ServerState::Running => "Running",
            ServerState::Stopped => "Stopped",
        }
    }
}

enum UpdateResponse<'a> {
    Success,
    HandledError(&'a str),
}

impl CfnAccessor {
    pub fn new(config: &aws_config::SdkConfig) -> Self {
        CfnAccessor {
            client: aws_sdk_cloudformation::Client::new(config),
        }
    }

    /// Parameters of the CFN template that do not change.
    const UNCHANGED_PARAMS: &'static [&'static str] = &[
        "ECSAMI",
        "EnableRcon",
        "FactorioImageTag",
        "HostedZoneId",
        "InstanceType",
        "KeyPairName",
        "RecordName",
        "SpotPrice",
        "UpdateModsOnStart",
        "YourIp",
    ];

    /// Updates the factorio CFN template to put the server in the desired state.
    ///
    /// If the server is already in the desired state, or an update is already
    /// in progress, no change is made and `UpdateResponse::HandledError` is returned.
    #[instrument]
    async fn update_server(&self, desired_state: ServerState) -> Result<UpdateResponse<'static>> {
        info!("attempting to update server");

        let unchanged_params: Vec<Parameter> = Self::UNCHANGED_PARAMS
            .iter()
            .map(|name| {
                Parameter::builder()
                    .set_parameter_key(Some(name.to_string()))
                    .set_use_previous_value(Some(true))
                    .build()
            })
            .collect();

        info!("updating server");
        let res = self
            .client
            .update_stack()
            .stack_name("factorio-ecs-spot")
            .use_previous_template(true)
            .capabilities(aws_sdk_cloudformation::types::Capability::CapabilityIam)
            .set_parameters(Some(unchanged_params))
            .parameters(
                Parameter::builder()
                    .set_parameter_key(Some("ServerState".to_string()))
                    .set_parameter_value(Some(desired_state.as_template_value().to_string()))
                    .build(),
            )
            .send()
            .await
            .map(|_| UpdateResponse::Success)
            .unwrap_or_else(|sdk_error| {
                tracing::error!(?sdk_error, "UpdateStackError");

                if let SdkError::ServiceError(response_error) = sdk_error {
                    let response_error = response_error.into_err();
                    if ProvideErrorMetadata::code(&response_error) == Some("ValidationError") {
                        let message = response_error.message().unwrap();
                        info!(message, "UpdateStack ValidationError");
                        if message == "No updates are to be performed." {
                            return UpdateResponse::HandledError(
                                "Server is already in the desired state.",
                            );
                        } else if message
                            .contains("is in UPDATE_IN_PROGRESS state and can not be updated")
                        {
                            return UpdateResponse::HandledError(
                                "Server is currently being updated",
                            );
                        }
                    }
                    panic!("Unhandled UpdateStackError {:?}", response_error);
                } else {
                    panic!("Unhandled SDK error: {:?}", sdk_error)
                }
            });

        Ok(res)
    }
}

impl ServerUpdater for CfnAccessor {
    async fn start_server(&self) -> Result<Value> {
        let res = self.update_server(ServerState::Running).await?;

        let content = match res {
            UpdateResponse::Success => "Starting the server!",
            UpdateResponse::HandledError(msg) => msg,
        };

        Ok(json!({
            "type": 4,
            "data": {
                "tts": false,
                "content": content,
                "embeds": [],
                "allowed_mentions": { "parse": [] }
            }
        }))
    }

    async fn stop_server(&self) -> Result<Value> {
        let res = self.update_server(ServerState::Stopped).await?;

        let content = match res {
            UpdateResponse::Success => "Stopping the server!",
            UpdateResponse::HandledError(msg) => msg,
        };

        Ok(json!({
            "type": 4,
            "data": {
                "tts": false,
                "content": content,
                "embeds": [],
                "allowed_mentions": { "parse": [] }
            }
        }))
    }
}
