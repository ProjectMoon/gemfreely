use crate::commands::sync::SyncCommand;
use clap::{Parser, Subcommand};
use commands::{login::LoginCommand, logout::LogoutCommand};

use anyhow::Result;

mod webmentions;
mod gemfeed;
mod sanitization;
mod wf;
mod commands;

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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(ref cmd) = cli.command {
        match cmd {
            Command::Login { .. } => LoginCommand::try_from(&cli)?.execute().await,
            Command::Logout { .. } => LogoutCommand::try_from(&cli)?.execute().await,
            Command::Sync { .. } => SyncCommand::try_from(&cli)?.execute().await,
        }
    } else {
        Ok(())
    }
}
