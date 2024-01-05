use ed25519_dalek::{Signature, Verifier, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use lambda_http::{run, service_fn, Body, Error, Request, Response, RequestExt};
use serde_json::{Value, json};
use tracing::{instrument, info, warn};

/// This is the main body for the function.
/// Write your code inside it.
/// There are some code example in the following URLs:
/// - https://github.com/awslabs/aws-lambda-rust-runtime/tree/main/examples
/// 
// #[instrument(skip(request), fields(req_id = %request.lambda_context().request_id))]
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

    let message = json!({
        "type": 4,
        "data": {
            "tts": false,
            "content": "Congrats on sending your command!",
            "embeds": [],
            "allowed_mentions": { "parse": [] }
        }
    }).to_string();

    // Return something that implements IntoResponse.
    // It will be serialized to the right response event automatically by the runtime
    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(message.into())
        .map_err(Box::new)?;
    Ok(resp)
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
    tracing_subscriber::fmt().json()
        .with_max_level(tracing::Level::INFO)
        .with_current_span(false)
        // disable printing the name of the module in every log line.
        .with_target(false)
        // disabling time is handy because CloudWatch will add the ingestion time.
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}
