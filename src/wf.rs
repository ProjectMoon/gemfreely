use anyhow::Result;
use std::result::Result as StdResult;
use url::Url;

use writefreely_client::{
    post::{Post, PostCreateRequest},
    Client, Timestamp,
};

use crate::gemfeed::GemfeedEntry;

/// Wrapper struct for managing the WriteFreely connection.
pub struct WriteFreely {
    client: Client,
    alias: String,
}

pub enum WriteFreelyCredentials<'a> {
    UsernameAndPassword(&'a str, &'a str),
    AccessToken(&'a str),
}

#[allow(dead_code)]
impl WriteFreely {
    /// Attempts to create and log in to the WriteFreely server.
    pub async fn new(
        url: &Url,
        alias: &str,
        creds: &WriteFreelyCredentials<'_>,
    ) -> Result<WriteFreely> {
        use WriteFreelyCredentials::*;
        let client = match creds {
            UsernameAndPassword(user, pw) => Client::new(url)?.login(user, pw).await?,
            AccessToken(token) => Client::new(url)?.with_token(token),
        };

        Ok(WriteFreely {
            client,
            alias: alias.to_owned(),
        })
    }

    pub async fn user(&self) -> Result<String> {
        Ok(self.client.get_authenticated_user().await?)
    }

    pub fn access_token(&self) -> Option<&str> {
        self.client.access_token.as_deref()
    }

    /// Logs the client out and renders this instance of the wrapper
    /// unusable.
    pub async fn logout(mut self) -> Result<()> {
        self.client.logout().await?;
        Ok(())
    }

    /// Get the slugs on the server for the alias/user.
    pub async fn slugs(&self) -> Result<Vec<String>> {
        let posts = self.client.collections().posts(&self.alias).list().await?;
        let slugs: Vec<_> = posts
            .into_iter()
            .flat_map(|post| post.slug)
            .map(|slug| slug.to_string())
            .collect();
        Ok(slugs)
    }

    pub async fn create_post(&self, entry: &GemfeedEntry) -> Result<Post> {
        let blog = self.client.collections().posts(&self.alias);
        let post = blog.create(entry.try_into()?).await?;
        Ok(post)
    }
}

impl TryFrom<GemfeedEntry> for PostCreateRequest {
    type Error = anyhow::Error;

    fn try_from(entry: GemfeedEntry) -> StdResult<Self, Self::Error> {
        PostCreateRequest::try_from(&entry)
    }
}

impl TryFrom<&GemfeedEntry> for PostCreateRequest {
    type Error = anyhow::Error;

    fn try_from(entry: &GemfeedEntry) -> StdResult<Self, Self::Error> {
        let published = entry.published().map(|date| Timestamp::from(*date));
        let req = PostCreateRequest::new()
            .slug(entry.slug().into())
            .title(entry.title())
            .body(entry.body_as_markdown()?);

        let req = match published {
            Some(publish_date) => req.created(publish_date),
            _ => req,
        };

        Ok(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tryfrom_to_request_handles_gt_lt() {
        let gemtext: String = r#"
        # This is gemtext <dyn>

        With a > in it.
        "#
        .lines()
        .map(|line| line.trim_start())
        .map(|line| format!("{}\n", line))
        .collect();

        let entry = GemfeedEntry::default().with_body(gemtext);
        let result = PostCreateRequest::try_from(entry);
        assert!(result.is_ok());
    }
}
