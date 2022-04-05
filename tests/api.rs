use anyhow::Result;
use hyper::body::Buf;
use hyper::{Body, Client, Method, Request, StatusCode};
use pony_playground::api::serve;
use pony_playground::init_github_client;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::task::JoinHandle;

#[tokio::test]
async fn evaluate() -> Result<()> {
    let _ = env_logger::try_init();
    let port = portpicker::pick_unused_port().expect("No port available");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let serve_addr = addr.clone();
    let gh_client = init_github_client("FOO".to_string())?;
    let handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { Ok(serve(serve_addr, gh_client).await?) });
    // TODO: how to better ensure the background task is already serving?
    tokio::time::sleep(Duration::from_secs(1)).await;
    let client = Client::new();

    let req_data = EvaluateInput {
        code: "actor Main\n  new create(env: Env) => env.out.print(U32(42).string())".to_string(),
        branch: None,
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/evaluate.json"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_data)?))?;
    let res = client.request(req).await?;
    let (parts, body) = res.into_parts();
    assert_eq!(StatusCode::OK, parts.status);
    let body = hyper::body::aggregate(body).await?;

    // try to parse as json with serde_json
    let payload: EvaluateOutput = serde_json::from_reader(body.reader())?;
    assert!(payload.success);
    assert!(payload.compiler.contains("Compiled with: LLVM"));
    assert_eq!("42\n", payload.stdout);
    assert_eq!("", payload.stderr);

    handle.abort();
    Ok(())
}

#[derive(Serialize, Debug)]
struct EvaluateInput {
    code: String,
    branch: Option<String>,
}

#[derive(Deserialize, Debug)]
struct EvaluateOutput {
    success: bool,
    compiler: String,
    stdout: String,
    stderr: String,
}

#[tokio::test]
async fn compile() -> Result<()> {
    let _ = env_logger::try_init();
    let port = portpicker::pick_unused_port().expect("No port available");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let serve_addr = addr.clone();
    let gh_client = init_github_client("FOO".to_string())?;
    let handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { Ok(serve(serve_addr, gh_client).await?) });
    // TODO: how to better ensure the background task is already serving?
    tokio::time::sleep(Duration::from_secs(1)).await;
    let client = Client::new();

    // compile with llvm-ir output
    let req_data = CompileInput {
        emit: "llvm-ir".to_string(),
        code: "actor Main\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_data)?))?;
    let res = client.request(req).await?;
    let (parts, body) = res.into_parts();
    assert_eq!(StatusCode::OK, parts.status);
    let body = hyper::body::aggregate(body).await?;

    let payload: CompileOutput = serde_json::from_reader(body.reader())?;
    assert!(payload.error.is_none());
    assert!(payload
        .result
        .unwrap_or_default()
        .contains("@Main_tag_create_oo"));

    // compile with asm output
    let req_data = CompileInput {
        emit: "asm".to_string(),
        code: "actor Main\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_data)?))?;
    let res = client.request(req).await?;
    let (parts, body) = res.into_parts();
    assert_eq!(StatusCode::OK, parts.status);
    let body = hyper::body::aggregate(body).await?;

    let payload: CompileOutput = serde_json::from_reader(body.reader())?;
    assert!(payload.error.is_none());
    assert!(payload
        .result
        .unwrap_or_default()
        .contains("Main_tag_create_oo:"));

    // invalid input
    let req_data = CompileInput {
        emit: "asm".to_string(),
        code: "actor Maine\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&req_data)?))?;
    let res = client.request(req).await?;
    let (parts, body) = res.into_parts();
    assert_eq!(StatusCode::OK, parts.status);
    let body = hyper::body::aggregate(body).await?;

    let payload: CompileOutput = serde_json::from_reader(body.reader())?;
    assert!(payload.result.is_none());
    assert!(payload
        .error
        .unwrap_or_default()
        .contains("no Main actor found in package 'main'"));
    handle.abort();
    Ok(())
}

#[derive(Serialize, Debug)]
struct CompileInput {
    emit: String,
    code: String,
    branch: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CompileOutput {
    result: Option<String>,
    error: Option<String>,
}
