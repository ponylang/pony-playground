//! API Routes

use crate::github::{self, update_gist, Client, GIST_DESCRIPTION, GIST_FILENAME};
use crate::{highlight, Branch, Playpen};
use anyhow::Result;
use axum::{
    body::Body,
    extract::{Extension, Json},
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
        Ok((status, compiler_output, program_stdout)) => Ok(Json(if status.success() {
            let output = highlight(emit, &program_stdout);
            json!({
                "result": output,
            })
        } else {
            json!({
                "error": compiler_output,
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
    Json(payload): Json<CreateGist>,
    Extension(client): Extension<Client>,
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

// static routes

const APPLICATION_JAVASCRIPT: HeaderValue = HeaderValue::from_static("application/javascript");
const TEXT_CSS: HeaderValue = HeaderValue::from_static("text/css");
const TEXT_HTML: HeaderValue = HeaderValue::from_static("text/html");

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
    static_content(content, APPLICATION_JAVASCRIPT)
}

pub fn static_css(content: &'static [u8]) -> Result<Response<Body>, StatusCode> {
    static_content(content, TEXT_CSS)
}

pub fn static_html(content: &'static [u8]) -> Result<Response<Body>, StatusCode> {
    static_content(content, TEXT_HTML)
}