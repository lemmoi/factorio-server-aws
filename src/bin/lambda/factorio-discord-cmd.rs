use factorio_server_lambda::{
    aws_client::{
        cfn::CfnAccessor, compute::ServerAccessor, ddb::DynamoDBAccessor, ServerInfo, ServerUpdater,
    },
    model::domain::StartServerInteraction,
};
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use serde_json::Value;
use time::OffsetDateTime;
use tracing::{info, warn};

use factorio_server_lambda::discord::{
    auth::DiscordAuthenticator, SignedRequest, VerifyDiscordReq,
};

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
async fn function_handler(
    discord_auth: &DiscordAuthenticator,
    server_accessor: &ServerAccessor,
    cfn_accessor: &CfnAccessor,
    ddb: &DynamoDBAccessor,
    request: Request,
) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    info!("recieved request");

    let body = match request.body() {
        Body::Empty => todo!(),
        Body::Text(text) => text,
        Body::Binary(_) => todo!(),
    };

    if let Some(auth_err) = discord_auth
        .verify(SignedRequest {
            body,
            headers: request.headers(),
        })
        .err()
    {
        warn!(?auth_err, "unauthorized request");
        let resp = Response::builder()
            .status(401)
            .header("content-type", "text/html")
            .body("Invalid header".into())
            .map_err(Box::new)?;

        return Ok(resp);
    };

    let parsed_body: Value = serde_json::from_str(body.as_str())?;
    info!(%parsed_body);

    let msg_type = parsed_body["type"].as_i64().expect("No type found");

    if msg_type == 1 {
        info!("ping event");
        let resp = Response::builder()
            .status(200)
            .header("content-type", "application/json")
            .body("{\"type\": 1}".into())
            .map_err(Box::new)?;

        return Ok(resp);
    }

    let interaction = StartServerInteraction {
        token: parsed_body["token"]
            .as_str()
            .expect("Missing interaction token")
            .to_string(),
        timestamp: OffsetDateTime::now_utc(),
    };

    let response = match parsed_body["data"]["options"][0]["name"]
        .as_str()
        .expect("missing command")
    {
        "start" => {
            let mount_dir = parsed_body["data"]["options"][0]["options"][0]["value"]
                .as_str()
                .expect("missing mount dir");
            let response = cfn_accessor.start_server(mount_dir).await?;
            ddb.save_interaction(interaction).await?;
            response
        }
        "stop" => cfn_accessor.stop_server().await?,
        "ip" => server_accessor.get_server_ip_response().await?,
        _ => panic!("Unknown command"),
    };

    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(response.to_string().into())
        .map_err(Box::new)?;
    Ok(resp)
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

    let discord_auth = DiscordAuthenticator::new();

    let aws_config = aws_config::load_from_env().await;
    let service_accessor = ServerAccessor::new(&aws_config);
    let cfn_accessor = CfnAccessor::new(&aws_config);
    let ddb = DynamoDBAccessor::new(&aws_config);

    run(service_fn(|event: Request| async {
        function_handler(&discord_auth, &service_accessor, &cfn_accessor, &ddb, event).await
    }))
    .await
}
