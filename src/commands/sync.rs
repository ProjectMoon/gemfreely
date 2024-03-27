use anyhow::{anyhow, Result};
use url::Url;

use crate::gemfeed::{Gemfeed, GemfeedParserSettings};
use crate::sanitization;
use crate::wf::{WriteFreely, WriteFreelyCredentials};
use crate::Cli;
use crate::Command;
use std::collections::HashSet;

struct SanitizeConfig<'a> {
    strip_before_marker: &'a Option<String>,
    strip_after_marker: &'a Option<String>,
}

pub(crate) struct SyncCommand<'a> {
    parser_settings: GemfeedParserSettings<'a>,
    wf_alias: &'a str,
    wf_token: &'a str,
    gemlog_url: &'a str,
    wf_url: &'a str,
    config: SanitizeConfig<'a>,
}

impl<'a> TryFrom<&'a Cli> for SyncCommand<'a> {
    type Error = anyhow::Error;

    fn try_from(cli: &'a Cli) -> std::prelude::v1::Result<Self, Self::Error> {
        if let Some(Command::Sync {
            ref wf_url,
            ref gemlog_url,
            ref strip_before_marker,
            ref strip_after_marker,
        }) = cli.command
        {
            let wf_token = cli
                .wf_access_token
                .as_deref()
                .ok_or(anyhow!("WriteFreely access token required"))?;

            let sanitize_cfg = SanitizeConfig {
                strip_before_marker,
                strip_after_marker,
            };

            Ok(Self {
                wf_url,
                gemlog_url,
                wf_token,
                config: sanitize_cfg,
                parser_settings: GemfeedParserSettings::from(cli),
                wf_alias: cli.wf_alias.as_deref().expect("WriteFreely Alias required"),
            })
        } else {
            Err(anyhow!("Invalid sync command"))
        }
    }
}

impl SyncCommand<'_> {
    pub async fn execute(self) -> Result<()> {
        let gemfeed_url = Url::parse(self.gemlog_url)?;
        let wf_url = Url::parse(self.wf_url)?;

        let wf_creds = WriteFreelyCredentials::AccessToken(self.wf_token);
        let wf_client = WriteFreely::new(&wf_url, self.wf_alias, &wf_creds).await?;

        let mut gemfeed = Gemfeed::load_with_settings(&gemfeed_url, &self.parser_settings)?;
        sync_gemlog(&self.config, &mut gemfeed, &wf_client).await?;

        Ok(())
    }
}

async fn sync_gemlog(
    config: &SanitizeConfig<'_>,
    gemfeed: &mut Gemfeed,
    wf: &WriteFreely,
) -> Result<()> {
    println!(
        "Beginning sync of posts for WriteFreely user: {}",
        wf.user().await?
    );

    let wf_slugs: HashSet<_> = wf.slugs().await?.into_iter().collect();
    let gemfeed_slugs: HashSet<_> = gemfeed.slugs().into_iter().collect();
    let slugs_to_post: Vec<_> = gemfeed_slugs.difference(&wf_slugs).collect();

    sanitize_gemlogs(gemfeed, config)?;

    let gemlogs_to_post = slugs_to_post
        .into_iter()
        .flat_map(|slug| gemfeed.find_entry_by_slug(slug));

    let mut count = 0;
    for entry in gemlogs_to_post {
        let result = wf.create_post(entry).await;
        count += 1;

        if let Ok(post) = result {
            println!(
                "Created post: {} [title={}]",
                post.id,
                post.title.unwrap_or_default()
            );
        } else {
            println!("Error creating post: {} ", result.unwrap_err());
        }
    }

    println!("Post synchronization complete [posts synced={}]", count);

    Ok(())
}

fn sanitize_gemlogs(gemfeed: &mut Gemfeed, config: &SanitizeConfig) -> Result<()> {
    for entry in gemfeed.entries_mut() {
        if let Some(ref before_marker) = config.strip_before_marker {
            sanitization::strip_before(entry, before_marker)?;
        }

        if let Some(ref after_marker) = config.strip_after_marker {
            sanitization::strip_after(entry, after_marker)?;
        }
    }

    Ok(())
}
