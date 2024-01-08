use anyhow::Result;
use serde_json::Value;

pub mod cfn;
pub mod compute;

pub trait ServerUpdater {
    async fn start_server(&self) -> Result<Value>;
    async fn stop_server(&self) -> Result<Value>;
}

pub trait ServerInfo {
    async fn get_server_ip(&self) -> Result<Value>;
}