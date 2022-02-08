//! Interaction with Github via its API
use anyhow::Result;
pub use hubcaps::gists::Gist;
use hubcaps::gists::GistOptions;
use std::collections::HashMap;

pub(crate) const GIST_FILENAME: &str = "main.pony";
pub(crate) const GIST_DESCRIPTION: &str = "Shared via Pony Playground";

pub type Client = hubcaps::Github;

pub(crate) async fn create_gist(
    client: &Client,
    description: String,
    filename: String,
    code: String,
) -> Result<Gist> {
    let mut files = HashMap::new();
    files.insert(filename, code);
    let gist_options = GistOptions::builder(files)
        .description(description)
        .public(true)
        .build();
    Ok(client.gists().create(&gist_options).await?)
}

pub(crate) async fn update_gist(client: &Client, id: &str, description: String) -> Result<Gist> {
    let options = GistOptions::builder(HashMap::<String, String>::new())
        .description(description)
        .build();

    Ok(client.gists().edit(id, &options).await?)
}
