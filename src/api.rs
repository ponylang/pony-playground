use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};

use crate::routes::{compile, create_gist, evaluate, static_css, static_html, static_js};
use crate::GithubClient;
use std::net::SocketAddr;
use http::HeaderValue;
use tower_http::cors::CorsLayer;

const layer = CorsLayer::new().allow_origin(
    "https://tutorial.ponylang.io".parse::<HeaderValue>().unwrap(),
);

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
        .route("/evaluate.json", post(evaluate))
        .layer(layer) // applies to every route() call before on `router`
        .route(
            "/",
            get(|| async { static_html(include_bytes!("../static/web.html")) }),
        )
        .route("/compile.json", post(compile))
        .route("/gist.json", post(create_gist))
        .with_state(github_client)
        .nest("/static", static_routes);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    Ok(axum::serve(listener, router).await?)
}
