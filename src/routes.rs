//! API Routes

use crate::github::{self, update_gist, Client, GIST_DESCRIPTION, GIST_FILENAME};
use crate::{highlight, Branch, Playpen};
use anyhow::Result;
use axum::{
    body::Body,
    extract::{Json, State},
    http::{header::CONTENT_TYPE, HeaderValue, Response, StatusCode},
};
use serde::Deserialize;
use serde_json::{json, Value};
use url::Url;

/// evaluate payload
#[derive(Deserialize)]
pub struct Evaluate {
    code: String,
    branch: Option<String>,
}

/// evaluate the given code
pub async fn evaluate(Json(payload): Json<Evaluate>) -> Result<Json<Value>, StatusCode> {
    let branch = payload
        .branch
        .map(|branch| branch.parse().unwrap())
        .unwrap_or(Branch::Release);

    match Playpen::evaluate(branch, payload.code).await {
        Ok((status, compiler, program_stdout)) => {
            let stderr = String::from_utf8_lossy(status.stderr()).into_owned();
            Ok(Json(json!({
                "success": status.success(),
                "compiler": compiler,
                "stdout": program_stdout,
                "stderr": stderr
            })))
        }
        Err(e) => {
            error!("Error evaluating playground code: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// compile code payload
#[derive(Deserialize)]
pub struct Compile {
    emit: String,
    code: String,
    branch: Option<String>,
}

/// compile the given code
pub async fn compile(Json(payload): Json<Compile>) -> Result<Json<Value>, StatusCode> {
    let emit = payload.emit.parse().unwrap();
    let branch = payload
        .branch
        .map(|branch| branch.parse().unwrap())
        .unwrap_or(Branch::Release);

    match Playpen::compile(branch, payload.code, emit).await {
        Ok((result, _compiler_output, program_stdout)) => Ok(Json(if result.success() {
            let output = highlight(emit, &program_stdout);
            json!({
                "result": output,
            })
        } else {
            json!({
                "error": String::from_utf8_lossy(result.stderr()),
            })
        })),
        Err(e) => {
            error!("Error compiling: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// create gist payload
#[derive(Deserialize)]
pub struct CreateGist {
    code: String,
    base_url: Url,
    branch: String,
}

/// create a gist
pub async fn create_gist(
    State(client): State<Client>,
    Json(payload): Json<CreateGist>,
) -> Result<Json<Value>, StatusCode> {
    match github::create_gist(
        &client,
        GIST_DESCRIPTION.into(),
        GIST_FILENAME.into(),
        payload.code,
    )
    .await
    {
        Ok(gist) => {
            let mut url = payload.base_url;
            url.query_pairs_mut().append_pair("gist", &gist.id);
            if payload.branch != "release" {
                url.query_pairs_mut().append_pair("branch", &payload.branch);
            }
            let url: String = url.into();

            match update_gist(
                &client,
                &gist.id,
                format!("{} ({})", GIST_DESCRIPTION, url.clone()),
            )
            .await
            {
                Ok(gist) => Ok(Json(json!({
                    "gist_id": gist.id,
                    "gist_url": gist.html_url,
                    "play_url": url,
                }))),
                Err(e) => {
                    error!("Error updating gist to append URL to description: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            error!("Error creating gist: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

use serde::{Serialize,Deserialize};
use minijinja::render;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
struct Metadata {
    title: String,
    description: String,
}

const WEB_HTML_TEMPLATE: &'static str = r#"
<!doctype html>
<html lang="en">

<head>
    <title>Pony Playground</title>
    <meta charset="utf-8" />
    <meta name=viewport id=meta-viewport content="width=720">
    <script>if (screen.width > 720) { document.getElementById("meta-viewport").setAttribute('content', 'width=device-width'); }</script>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css?family=Source+Code+Pro:400,700" />
    <link rel="shortcut icon" href="https://avatars1.githubusercontent.com/u/12997238" />
    <script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.4.13/ace.js"
        integrity="sha512-OMjy8oWtPbx9rJmoprdaQdS2rRovgTetHjiBf7RL7LvRSouoMLks5aIcgqHb6vGEAduuPdBTDCoztxLR+nv45g=="
        crossorigin="anonymous" charset="utf-8"></script>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/ace/1.4.13/ext-themelist.min.js"
        integrity="sha512-Y7MEa84tKNdub5CgOzVCI7jCitaDVPUUrSQmoACBnhOxMQUtxSqZllJ5HsYJvJFQXLfqcbGMCzwid0xMaI7MCA=="
        crossorigin="anonymous" charset="utf-8"></script>

    <link rel="stylesheet" href="/static/web.css">
    <script src="/static/web.js"></script>
    <script src="/static/mode-pony.js"></script>

    <meta property="og:type" content="website" />
    <meta name="twitter:card" content="app" />
    <meta name="twitter:site" content="ponylang" />
    <meta name="twitter:creator" content="ponylang" />
    {% for (key, value) in metadata %}
        <meta property="og:{{ key }}" content="{{ value }}">
        <meta name="twitter:{{ key }}" content="{{ value }}">
    {% endfor %}
</head>

<body>
    <form id="control">
        <div><button type="button" class="primary" id="evaluate"
                title="Execute your code (you can also press Ctrl+Enter when editing code)">Run</button><button
                type="button" id="asm" title="Compile to ASM">ASM</button><button type="button" id="llvm-ir"
                title="Compile to LLVM IR">LLVM IR</button></div><wbr>
        <div><button type="button" id="gist" title="Share a link to your code via Gist">Share</button></div><wbr>
        <div class="right-c-e"><button type="button" id="configure-editor"><span>Configure editor</span></button>
            <div class="dropdown">
                <p><label for="keyboard">Keyboard bindings:</label>
                    <select name="keyboard" id="keyboard">
                        <option>Ace</option>
                        <option>Emacs</option>
                        <option>Vim</option>
                    </select>
                <p><label for="themes">Theme:</label>
                    <select name="themes" id="themes"></select>
            </div>
        </div>
    </form>
    <main>
        <div id="editor">actor Main
  new create(env: Env) =>
    env.out.print("Hello, world!")</div>
        <div id="result" data-empty>
            <div></div><button type="button" id="clear-result" title="Close this pane">×</button>
        </div>
    </main>
</body>

</html>
</body>
</html>
"#;

#[derive(Deserialize)]
struct URLSearchParams {
    snippet: Option<String>,
    gist: Option<String>,
    code: Option<String>,
    run: Option<bool>,
    branch: Option<String>,
}

fn extract_docstring(url: String) -> String {
    let re = Regex::new(r"^\"\"\"\n(?<docstring>.*?)\n\"\"\"").unwrap();
    let Some(caps) = re.captures(resp.text()?) else { return };
    let end = &caps["docstring"].chars().map(|c| c.len_utf8()).take(60).sum();
    return &caps["docstring"][..end];
}

#[derive(Deserialize)]
struct Gist {
    files: HashMap<String, GistFile>,
}

#[derive(Deserialize)]
struct GistFile {
    filename: String,
    r#type: String,
    language: String,
    raw_url: String,
    size: u128,
    truncated: bool,
    content: String,
}

async fn get_web_html(urlSearchParams: Query<URLSearchParams>) -> Html<String> {
    let metadata_defaults = Metadata {
        title: "Pony Playground",
        description: "Run ponylang code or compile it to ASM/LLVM IR",
    };

    if Query.snippet.is_some() {
        let snippet_name = Query.snippet.as_ref().unwrap();
        let resp = reqwest::blocking::get(concat!("https://raw.githubusercontent.com/ponylang/pony-tutorial/main/code-samples/", snippet_name))?;
        if resp.status().is_success() {
            metadata_defaults.title = snippet_name
            metadata_defaults.description = extract_docstring(resp.text()?);
        }
    } else if Query.gist.is_some() {
        let snippet_name = Query.snippet.as_ref().unwrap();
        let resp = reqwest::blocking::get(concat!("https://raw.githubusercontent.com/ponylang/pony-tutorial/main/code-samples/", snippet_name))?;
        if description.is_some() {
            metadata_defaults.title = snippet_name
            let json: Gist = resp.json().unwrap();
            metadata_defaults.description = extract_docstring(json.content);
        }
    }
    
    let r = render!(WEB_HTML_TEMPLATE, metadata => metadata_defaults );
    Html(r)
}

// static routes

static APPLICATION_JAVASCRIPT: HeaderValue = HeaderValue::from_static("application/javascript");
static TEXT_CSS: HeaderValue = HeaderValue::from_static("text/css");
static TEXT_HTML: HeaderValue = HeaderValue::from_static("text/html");

pub fn static_content(
    content: &'static [u8],
    content_type: HeaderValue,
) -> Result<Response<Body>, StatusCode> {
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(content.into())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn static_js(content: &'static [u8]) -> Result<Response<Body>, StatusCode> {
    static_content(content, APPLICATION_JAVASCRIPT.clone())
}

pub fn static_css(content: &'static [u8]) -> Result<Response<Body>, StatusCode> {
    static_content(content, TEXT_CSS.clone())
}
