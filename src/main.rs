use aws_sdk_cloudformation::{
    error::{ProvideErrorMetadata, SdkError},
    types::Parameter,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use serde_json::{json, Value};
use tracing::{info, instrument, warn};

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

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
async fn function_handler(request: Request) -> Result<Response<Body>, Error> {
    // Extract some useful information from the request
    info!("recieved request");

    if verify(&request).is_err() {
        warn!("unauthorized request");
        let resp = Response::builder()
            .status(401)
            .header("content-type", "text/html")
            .body("Invalid header".into())
            .map_err(Box::new)?;

        return Ok(resp);
    }

    let body = match request.body() {
        Body::Empty => todo!(),
        Body::Text(text) => text,
        Body::Binary(_) => todo!(),
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

    let response = match parsed_body["data"]["options"][0]["name"]
        .as_str()
        .expect("missing command")
    {
        "start" => start_server().await,
        "stop" => stop_server().await,
        _ => panic!("Unknown command"),
    }?;

    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(response.to_string().into())
        .map_err(Box::new)?;
    Ok(resp)
}

async fn start_server() -> Result<Value, Error> {
    let res = update_server(ServerState::Running).await?;

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

async fn stop_server() -> Result<Value, Error> {
    let res = update_server(ServerState::Stopped).await?;

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

enum UpdateResponse<'a> {
    Success,
    HandledError(&'a str),
}

#[instrument]
async fn update_server(desired_state: ServerState) -> Result<UpdateResponse<'static>, Error> {
    info!("attempting to update server");
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_cloudformation::Client::new(&config);

    let unchanged_params: Vec<Parameter> = [
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
    ]
    .iter()
    .map(|name| {
        Parameter::builder()
            .set_parameter_key(Some(name.to_string()))
            .set_use_previous_value(Some(true))
            .build()
    })
    .collect();

    info!("updating server");
    let res = client
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
                        return UpdateResponse::HandledError("Server is currently being updated");
                    }
                }
                panic!("Unhandled UpdateStackError {:?}", response_error);
            } else {
                panic!("Unhandled SDK error: {:?}", sdk_error)
            }
        });

    Ok(res)
}

fn verify(event: &Request) -> Result<(), Error> {
    let mut sig_bytes: [u8; SIGNATURE_LENGTH] = [0; SIGNATURE_LENGTH];
    hex::decode_to_slice(
        event.headers()["X-Signature-Ed25519"].to_str()?,
        &mut sig_bytes,
    )?;
    let sig: Signature = Signature::from_bytes(&sig_bytes);

    let timestamp = event.headers()["X-Signature-Timestamp"].to_str()?;

    let body = match event.body() {
        Body::Empty => todo!(),
        Body::Text(text) => text,
        Body::Binary(_) => todo!(),
    };

    Ok(get_verifying_key().verify(format!("{}{}", timestamp, body).as_bytes(), &sig)?)
}

fn get_verifying_key() -> VerifyingKey {
    let mut key_bytes: [u8; PUBLIC_KEY_LENGTH] = [0; PUBLIC_KEY_LENGTH];
    hex::decode_to_slice(
        "5de3bcb92187f4dcc23ac1b2d2276caa0ccac57ab592f76ee57c4d7f0e692252",
        &mut key_bytes,
    )
    .expect("PK decoding failed");

    VerifyingKey::from_bytes(&key_bytes).expect("Invalid VerifyingKey")
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

    run(service_fn(function_handler)).await
}
