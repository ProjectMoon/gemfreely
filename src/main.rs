use crate::{gemfeed::Gemfeed, wf::WriteFreelyCredentials};
use clap::{Parser, Subcommand};
use gemfeed::GemfeedParserSettings;
use std::collections::HashSet;
use url::Url;

use anyhow::Result;
use wf::WriteFreely;

mod gemfeed;
mod sanitization;
mod wf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// WriteFreely access token. Required for sync and logout.
    #[arg(short = 't', long, value_name = "TOKEN")]
    wf_access_token: Option<String>,

    /// WriteFreely blog name/alias. Usually the same as username.
    #[arg(short = 'a', long, value_name = "ALIAS")]
    wf_alias: Option<String>,

    /// Optional date format override for parsing Gemlog Atom publish dates.
    #[arg(long, value_name = "FMT")]
    date_format: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Logs in to WriteFreely and prints an access token.
    Login {
        /// Root URL of WriteFreely instance.
        #[arg(long, value_name = "URL")]
        wf_url: String,

        /// WriteFreely username.
        #[arg(short, long)]
        username: String,

        /// WriteFreely password.
        #[arg(short, long)]
        password: String,
    },

    /// Logs out from WriteFreely.
    Logout {
        /// Root URL of WriteFreely instance.
        #[arg(long, value_name = "URL")]
        wf_url: String,
    },

    /// Synchronize Gemlog posts from Gemini to WriteFreely.
    Sync {
        /// Full gemini:// URL of Gemlog (Atom feed or Gemfeed).
        #[arg(long, value_name = "URL")]
        gemlog_url: String,

        /// Root URL of WriteFreely instance.
        #[arg(long, value_name = "URL")]
        wf_url: String,

        /// Optional santization rule: Remove all text BEFORE this
        /// marker in the Gemlog post.
        #[arg(long)]
        strip_before_marker: Option<String>,

        /// Optional santization rule: Remove all text AFTER this
        /// marker in the Gemlog post.
        #[arg(long)]
        strip_after_marker: Option<String>,
    },
}

struct SanitizeConfig<'a> {
    strip_before_marker: &'a Option<String>,
    strip_after_marker: &'a Option<String>,
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

async fn sync(
    cli: &Cli,
    gemlog_url: &str,
    wf_url: &str,
    config: &SanitizeConfig<'_>,
) -> Result<()> {
    let wf_token = cli
        .wf_access_token
        .as_deref()
        .expect("WriteFreely access token required");

    let settings = GemfeedParserSettings::from(cli);
    let gemfeed_url = Url::parse(gemlog_url)?;
    let wf_url = Url::parse(wf_url)?;

    let wf_creds = WriteFreelyCredentials::AccessToken(wf_token);
    let wf_alias = cli.wf_alias.as_deref().expect("WriteFreely Alias required");
    let wf_client = wf::WriteFreely::new(&wf_url, wf_alias, &wf_creds).await?;

    let mut gemfeed = Gemfeed::load_with_settings(&gemfeed_url, &settings)?;
    sync_gemlog(&config, &mut gemfeed, &wf_client).await?;

    Ok(())
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
        let post = wf.create_post(entry).await?;
        count += 1;
        println!(
            "Created post: {} [title={}]",
            post.id,
            post.title.unwrap_or_default()
        );
    }

    println!("Post synchronization complete [posts synced={}]", count);

    Ok(())
}

async fn wf_login(wf_url: &str, username: &str, password: &str) -> Result<()> {
    let wf_url = Url::parse(wf_url)?;
    let creds = WriteFreelyCredentials::UsernameAndPassword(username, password);

    let wf_client = wf::WriteFreely::new(&wf_url, &username, &creds).await?;

    println!(
        "{}",
        wf_client.access_token().unwrap_or("[No Token Returned]")
    );

    Ok(())
}

async fn wf_logout(wf_url: &str, wf_alias: &str, access_token: &str) -> Result<()> {
    let wf_url = Url::parse(wf_url)?;
    let creds = WriteFreelyCredentials::AccessToken(access_token);

    let wf_client = wf::WriteFreely::new(&wf_url, &wf_alias, &creds).await?;
    wf_client.logout().await?;

    println!("Successfully logged out from {}", wf_url);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(ref cmd) = cli.command {
        match cmd {
            Command::Login {
                ref wf_url,
                ref username,
                ref password,
            } => wf_login(wf_url, username, password).await,
            Command::Logout { ref wf_url } => {
                wf_logout(
                    wf_url,
                    &cli.wf_alias.as_deref().expect("WriteFreely alias required"),
                    &cli.wf_access_token.expect("Access token required"),
                )
                .await
            }
            Command::Sync {
                wf_url,
                gemlog_url,
                strip_before_marker,
                strip_after_marker,
            } => {
                let sanitize_cfg = SanitizeConfig {
                    strip_before_marker,
                    strip_after_marker,
                };
                sync(&cli, gemlog_url, wf_url, &sanitize_cfg).await
            }
        }
    } else {
        Ok(())
    }
}
