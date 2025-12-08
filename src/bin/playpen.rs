use anyhow::Result;

use pony_playground::api;
use pony_playground::{GithubClient, init_github_client};
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
    let github_client: GithubClient = init_github_client(token)?;

    // TODO: determine either by env var or command line argument
    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    log::info!("Listening on  {addr}...");
    api::serve(addr, github_client).await
}
