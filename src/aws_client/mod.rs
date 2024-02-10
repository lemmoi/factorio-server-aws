use anyhow::Result;
use serde_json::Value;

pub mod cfn;
pub mod compute;
pub mod ddb;

pub trait ServerUpdater {
    async fn start_server(&self, mount_dir: &str) -> Result<Value>;
    async fn stop_server(&self) -> Result<Value>;
}

pub trait ServerInfo {
    async fn get_server_ip_response(&self) -> Result<Value>;
    async fn get_running_server_ip(&self) -> Result<Option<String>>;
}
