use anyhow::Result;
use pony_playground::api::serve;
use pony_playground::init_github_client;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::task::JoinHandle;

#[tokio::test]
#[ignore]
async fn evaluate() -> Result<()> {
    let _ = env_logger::try_init();
    // reqwest wants ring, octocrab wants aws_lc_rs
    // as this is ambiguous
    // we gotta set a default here, otherwise nothing works
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let port = portpicker::pick_unused_port().expect("No port available");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let serve_addr = addr;
    let gh_client = init_github_client("FOO".to_string())?;
    let handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { serve(serve_addr, gh_client).await });
    // TODO: how to better ensure the background task is already serving?
    tokio::time::sleep(Duration::from_secs(1)).await;
    let client = Client::new();

    let req_data = EvaluateInput {
        code: "actor Main\n  new create(env: Env) => env.out.print(U32(42).string())".to_string(),
        branch: None,
    };
    let req = client
        .post(format!("http://127.0.0.1:{port}/evaluate.json"))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&req_data)?);
    let res = req.send().await?;
    assert_eq!(StatusCode::OK, res.status());

    let payload = res.json::<EvaluateOutput>().await?;
    assert!(payload.success);
    assert!(payload.compiler.contains("Compiled with: LLVM"));
    assert_eq!("42\n", payload.stdout);
    assert_eq!("", payload.stderr);

    let req_data = EvaluateInput {
        code: r#"use buffered = "buffered"
use json = "json"

actor Main
  new create(env: Env) =>
    let reader = buffered.Reader
    for chunk in buffered.Writer.>u32_be(42).done().values() do
      reader.append(chunk)
    end
    env.out.print(reader.size().string())
    match json.JSONParser.parse("42")
    | let value: I64 => env.out.print(value.string())
    else env.exitcode(1)
    end
"#
        .to_string(),
        branch: None,
    };
    let res = client
        .post(format!("http://127.0.0.1:{port}/evaluate.json"))
        .json(&req_data)
        .send()
        .await?;
    assert_eq!(StatusCode::OK, res.status());
    let payload = res.json::<EvaluateOutput>().await?;
    assert!(payload.success, "{payload:?}");
    assert_eq!("4\n42\n", payload.stdout);
    assert_eq!("", payload.stderr);

    let req_data = EvaluateInput {
        code: r#"use pt = "pony_test"

actor \nodoc\ Main is pt.TestList
  new create(env: Env) => pt.PonyTest(env, this)
  new make() => None
  fun tag tests(test: pt.PonyTest) => test(_UserTest)

class \nodoc\ _UserTest is pt.UnitTest
  fun name(): String => "playground/user-test"
  fun apply(h: pt.TestHelper) => h.assert_eq[U32](42, 42)
"#
        .to_string(),
        branch: None,
    };
    let res = client
        .post(format!("http://127.0.0.1:{port}/evaluate.json"))
        .json(&req_data)
        .send()
        .await?;
    assert_eq!(StatusCode::OK, res.status());
    let payload = res.json::<EvaluateOutput>().await?;
    assert!(payload.success, "{payload:?}");
    assert!(payload.stdout.contains("playground/user-test complete"));
    assert!(payload.stdout.contains("1 test ran."));
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
#[ignore]
async fn compile() -> Result<()> {
    let _ = env_logger::try_init();
    // reqwest wants ring, octocrab wants aws_lc_rs
    // as this is ambiguous
    // we gotta set a default here, otherwise nothing works
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let port = portpicker::pick_unused_port().expect("No port available");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let serve_addr = addr;
    let gh_client = init_github_client("FOO".to_string())?;
    let handle: JoinHandle<Result<()>> =
        tokio::spawn(async move { serve(serve_addr, gh_client).await });
    // TODO: how to better ensure the background task is already serving?
    tokio::time::sleep(Duration::from_secs(1)).await;
    let client = Client::new();

    // compile with llvm-ir output
    let req_data = CompileInput {
        emit: "llvm-ir".to_string(),
        code: "actor Main\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = client
        .post(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&req_data)?);
    let res = req.send().await?;
    assert_eq!(StatusCode::OK, res.status());

    let payload: CompileOutput = res.json().await?;
    assert!(payload.error.is_none());
    assert!(
        payload
            .result
            .unwrap_or_default()
            .contains("source_filename")
    );

    // compile with asm output
    let req_data = CompileInput {
        emit: "asm".to_string(),
        code: "actor Main\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = client
        .post(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&req_data)?);
    let res = req.send().await?;
    assert_eq!(StatusCode::OK, res.status());

    let payload: CompileOutput = res.json().await?;
    assert!(payload.error.is_none());
    assert!(
        payload
            .result
            .unwrap_or_default()
            .contains("Main_tag_create_ioo")
    );

    // invalid input
    let req_data = CompileInput {
        emit: "asm".to_string(),
        code: "actor Maine\n  new create(env: Env) => None".to_string(),
        branch: None,
    };
    let req = client
        .post(format!("http://127.0.0.1:{port}/compile.json"))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&req_data)?);
    let res = req.send().await?;
    assert_eq!(StatusCode::OK, res.status());

    let payload: CompileOutput = res.json().await?;
    assert!(payload.result.is_none());
    assert!(
        payload
            .error
            .unwrap_or_default()
            .contains("no Main actor found in module 'main'")
    );

    for (emit, marker) in [
        ("llvm-ir", "source_filename"),
        ("asm", "Main_tag_create_ioo"),
    ] {
        let req_data = CompileInput {
            emit: emit.to_string(),
            code: r#"use buffered = "buffered"
use json = "json"

actor Main
  new create(env: Env) =>
    let reader = buffered.Reader
    for chunk in buffered.Writer.>u32_be(42).done().values() do
      reader.append(chunk)
    end
    env.out.print(reader.size().string())
    match json.JSONParser.parse("42")
    | let value: I64 => env.out.print(value.string())
    else env.exitcode(1)
    end
"#
            .to_string(),
            branch: None,
        };
        let res = client
            .post(format!("http://127.0.0.1:{port}/compile.json"))
            .json(&req_data)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, res.status());
        let payload = res.json::<CompileOutput>().await?;
        assert!(payload.error.is_none(), "{emit}: {payload:?}");
        assert!(payload.result.unwrap_or_default().contains(marker));
    }
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

#[test]
#[ignore]
fn standard_library_image() -> Result<()> {
    let output = std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "ponylang-playpen",
            "sh",
            "-exc",
            r#"packages="$(dirname "$(dirname "$(readlink -f "$(command -v ponyc)")")")/packages"
test -d "$packages"
for file in builtin/builtin.pony pony_test/pony_test.pony \
    pony_test/_test_runner.pony pony_test/_test_record.pony \
    pony_test/_property_test.pony; do
  test -f "$packages/$file"
done
for file in buffered/_test.pony json/_test.pony json/_stream_test.pony \
    files/_non_root_test.pony net/_test_connection.pony; do
  test ! -e "$packages/$file"
done"#,
        ])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
