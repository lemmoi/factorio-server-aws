use anyhow::Result;
use aws_lambda_events::event::cloudwatch_events::CloudWatchEvent;
use factorio_server_lambda::aws_client::ddb::DynamoDBAccessor;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde_json::json;
use time::OffsetDateTime;
use tracing::info;

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
/// - https://github.com/aws-samples/serverless-rust-demo/
async fn function_handler(
    ddb: &DynamoDBAccessor,
    event: LambdaEvent<CloudWatchEvent>,
) -> Result<(), Error> {
    info!(?event.payload, "Received event");
    let _detail = event.payload.detail.expect("No detail was provided");
    let stack_status = _detail["status-details"]["status"]
        .as_str()
        .expect("No stack status was provided");

    match stack_status {
        "UPDATE_COMPLETE" => Ok(handle_stack_update(ddb).await?),
        _ => Ok(()),
    }
}

const APP_ID: &str = "1192583719236665424";

async fn handle_stack_update(ddb: &DynamoDBAccessor) -> Result<()> {
    let retrieved = ddb.get_latest_start().await?;
    if retrieved.is_none() {
        info!("No token was retrieved for this event.");
        return Ok(());
    }
    let retrieved = retrieved.unwrap();
    let time_gap = OffsetDateTime::now_utc() - retrieved.timestamp;

    info!(?retrieved, "Retrieved token");
    let url = format!(
        "https://discord.com/api/v10/webhooks/{}/{}/messages/@original",
        APP_ID, retrieved.token
    );
    info!(url, "Sending patch to URL");

    reqwest::Client::new()
        .patch(url)
        .body(json!({
                "content": format!("Server has now been started and factorio is running. Start up took: {} minutes", time_gap.whole_minutes())
            }
        ).to_string())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .send()
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_current_span(false)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();

    let ddb = DynamoDBAccessor::new(&aws_config::load_from_env().await);
    run(service_fn(|event: LambdaEvent<CloudWatchEvent>| async {
        function_handler(&ddb, event).await
    }))
    .await
}
