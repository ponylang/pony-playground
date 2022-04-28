use anyhow::Result;
use axum::{
    extract::Extension,
    routing::{get, post},
    Router, Server,
};

use crate::routes::{compile, create_gist, evaluate, static_css, static_html, static_js};
use crate::GithubClient;
use std::net::SocketAddr;

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
            get(|| async { static_html(include_bytes!("../static/web.html")) }),
        )
        .route("/evaluate.json", post(evaluate))
        .route("/compile.json", post(compile))
        .route("/gist.json", post(create_gist))
        .layer(Extension(github_client))
        .nest("/static", static_routes);
    Ok(Server::bind(&addr)
        .serve(router.into_make_service())
        .await?)
}
