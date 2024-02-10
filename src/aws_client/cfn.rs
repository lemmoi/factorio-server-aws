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
    Running(String),
    Stopped,
}

impl ServerState {
    fn as_template_value(&self) -> &str {
        match self {
            ServerState::Running(_) => "Running",
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
        let mut builder = self
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
            );

        if let ServerState::Running(mount_dir) = desired_state {
            builder = builder.parameters(
                Parameter::builder()
                    .set_parameter_key(Some("MountingDir".to_string()))
                    .set_parameter_value(Some(format!("/{}/", mount_dir)))
                    .build(),
            )
        }

        let res = builder
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
    async fn start_server(&self, mount_dir: &str) -> Result<Value> {
        let res = self
            .update_server(ServerState::Running(mount_dir.to_string()))
            .await?;

         let (title, description) =match res {
            UpdateResponse::Success => {
                ("Starting the server!", Some(format!("Using the `{}` save. This message will update when the server is ready to join.", mount_dir)))
            }
            UpdateResponse::HandledError(msg) => (msg, None),
        };

        Ok(json!({
            "type": 4,
            "data": {
                "tts": false,
                "content": "",
                "embeds": [
                    {
                      "type": "rich",
                      "title": title,
                      "description": description,
                      "color": 0x00FFFF,
                      "thumbnail": {
                        "url": "https://factorio.com/static/img/factorio-wheel.png",
                        "height": 0,
                        "width": 0
                      }
                    }
                  ],
                "allowed_mentions": { "parse": [] }
            }
        }))
    }

    async fn stop_server(&self) -> Result<Value> {
        let res = self.update_server(ServerState::Stopped).await?;

        let title = match res {
            UpdateResponse::Success => "Stopping the server!",
            UpdateResponse::HandledError(msg) => msg,
        };

        Ok(json!({
            "type": 4,
            "data": {
                "tts": false,
                "content": "",
                "embeds": [
                    {
                      "type": "rich",
                      "title": title,
                      "color": 0x930707,
                      "thumbnail": {
                        "url": "https://factorio.com/static/img/factorio-wheel.png",
                        "height": 0,
                        "width": 0
                      }
                    }
                  ],
                "allowed_mentions": { "parse": [] }
            }
        }))
    }
}
