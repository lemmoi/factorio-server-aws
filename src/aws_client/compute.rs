use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use super::ServerInfo;

pub struct ServerAccessor {
    asg_client: aws_sdk_autoscaling::Client,
    ec2_client: aws_sdk_ec2::Client,
    ecs_client: aws_sdk_ecs::Client,
}

impl ServerAccessor {
    pub fn new(config: &aws_config::SdkConfig) -> Self {
        ServerAccessor {
            asg_client: aws_sdk_autoscaling::Client::new(config),
            ec2_client: aws_sdk_ec2::Client::new(config),
            ecs_client: aws_sdk_ecs::Client::new(config),
        }
    }

    /// Given an EC2 instance ID, return it's public IPv4 address
    async fn get_instance_ip(&self, instance_id: &str) -> Result<String> {
        let response = self
            .ec2_client
            .describe_instances()
            .instance_ids(instance_id)
            .send()
            .await?;

        let ip_address = response
            .reservations()
            .first()
            .and_then(|res| res.instances().first())
            .and_then(|instance| instance.public_ip_address());

        Ok(ip_address.expect("No IP address found").to_string())
    }

    /// Return true if the factorio service has at least one actively runing, false otherwise.
    async fn is_ecs_running(&self) -> Result<bool> {
        let response = self
            .ecs_client
            .describe_services()
            .cluster("factorio-ecs-spot-cluster")
            .services("factorio-ecs-spot-ecs-service")
            .send()
            .await?;

        let running_tasks = response
            .services()
            .first()
            .and_then(|service| service.deployments().first())
            .map(|deployment| deployment.running_count);

        Ok(running_tasks.expect("Deployment was not found") > 0)
    }
}

impl ServerInfo for ServerAccessor {
    async fn get_server_ip(&self) -> Result<Value> {
        let _asg_response = self
            .asg_client
            .describe_auto_scaling_groups()
            .auto_scaling_group_names("factorio-ecs-spot-asg")
            .send()
            .await?;
        let asg_instance = _asg_response
            .auto_scaling_groups()
            .first()
            .and_then(|asg| asg.instances().first());

        let content = if let Some(asg_instance) = asg_instance {
            match asg_instance.lifecycle_state().unwrap() {
                aws_sdk_autoscaling::types::LifecycleState::InService => {
                    info!("ASG instance is InService");
                    let ip_future = self.get_instance_ip(asg_instance.instance_id().unwrap());
                    let is_ecs_running_future = self.is_ecs_running();

                    let ip = ip_future.await?;
                    if is_ecs_running_future.await? {
                        format!("Server is up and running at IP: `{}`!", ip)
                    } else {
                        format!(
                            "Server IP will be: `{}`. However, factorio has not started running yet.",
                            ip
                        )
                    }
                }
                _not_running_state => {
                    format!("Server instance is in the {:#?} state", _not_running_state)
                }
            }
        } else {
            "No server is running.".to_string()
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
