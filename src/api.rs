use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};

use crate::routes::{compile, create_gist, evaluate, static_css, static_html, static_js};
use crate::GithubClient;
use std::net::SocketAddr;

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

/// serve the api
pub async fn serve(addr: SocketAddr, github_client: GithubClient) -> Result<()> {
    let static_routes = Router::new()
        .route(
            "/web.css",
            get(|| async { static_css(include_bytes!("../static/web.css")) }),
        )
        .route(
            "/web.js",
            get(|| async { static_js(include_bytes!("../static/web.js")) }),
        )
        .route(
            "/mode-pony.js",
            get(|| async { static_js(include_bytes!("../static/mode-pony.js")) }),
        );
    let router = Router::new()
        .route(
            "/",
            get(get_web_html),
        )
        .route("/evaluate.json", post(evaluate))
        .route("/compile.json", post(compile))
        .route("/gist.json", post(create_gist))
        .with_state(github_client)
        .nest("/static", static_routes);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    Ok(axum::serve(listener, router).await?)
}
