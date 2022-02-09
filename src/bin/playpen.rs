use anyhow::Result;

use axum::AddExtensionLayer;
use axum::{
    routing::{get, post},
    Router, Server,
};
use pony_playground::routes::{compile, create_gist, evaluate, static_css, static_html, static_js};
use std::net::SocketAddr;
use std::process::Command;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    // Make sure pygmentize is installed before starting the server
    match Command::new("pygmentize").arg("-V").status() {
        Ok(status) if status.success() => (),
        _ => panic!("Cannot find pygmentize."),
    };

    let token = match std::env::var("GITHUB_TOKEN") {
        Ok(token) => token,
        Err(_) => panic!("Missing GITHUB_TOKEN environment variable."),
    };
    let github_client = pony_playground::init_github_client(token)?;

    // TODO: determine either by env var or command line argument
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    let static_routes = Router::new()
        .route(
            "/web.css",
            get(|| async { static_css(include_bytes!("../../static/web.css")) }),
        )
        .route(
            "/web.js",
            get(|| async { static_js(include_bytes!("../../static/web.js")) }),
        )
        .route(
            "/mode-pony.js",
            get(|| async { static_js(include_bytes!("../../static/mode-pony.js")) }),
        );
    let router = Router::new()
        .route(
            "/",
            get(|| async { static_html(include_bytes!("../../static/web.html")) }),
        )
        .route("/evaluate.json", post(evaluate))
        .route("/compile.json", post(compile))
        .route("/gist.json", post(create_gist))
        .layer(AddExtensionLayer::new(github_client))
        .nest("/static", static_routes);
    Ok(Server::bind(&addr)
        .serve(router.into_make_service())
        .await?)
}
