use anyhow::{anyhow, Error, Result};

use writefreely_client::{
    post::{PostCreateRequest, Slug},
    Client,
};

pub async fn slugs_on_writefreely(client: &Client, alias: &str) -> Result<Vec<String>> {
    let posts = client.collections().posts(alias).list().await?;
    let slugs: Vec<_> = posts
        .into_iter()
        .flat_map(|post| post.slug)
        .map(|slug| slug.to_string())
        .collect();
    Ok(slugs)
}
