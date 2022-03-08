//! Interaction with Github via its API
use std::sync::Arc;

use anyhow::Result;
pub use octocrab::models::gists::Gist;

pub(crate) const GIST_FILENAME: &str = "main.pony";
pub(crate) const GIST_DESCRIPTION: &str = "Shared via Pony Playground";

pub type Client = Arc<octocrab::Octocrab>;

pub(crate) async fn create_gist(
    client: &Client,
    description: String,
    filename: String,
    code: String,
) -> Result<Gist> {
    let gist = client
        .gists()
        .create()
        .description(description)
        .file(filename, code)
        .public(true)
        .send()
        .await?;
    Ok(gist)
}

pub(crate) async fn update_gist(client: &Client, id: &str, description: String) -> Result<Gist> {
    let gist = client
        .gists()
        .update(id)
        .description(description)
        .send()
        .await?;
    Ok(gist)
}

pub fn init_client(token: String) -> Result<Client> {
    octocrab::initialise(octocrab::Octocrab::builder().personal_token(token))?;
    Ok(octocrab::instance())
}
