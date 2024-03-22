use crate::wf::{WriteFreely, WriteFreelyCredentials};
use crate::{Cli, Command};
use anyhow::{anyhow, Result};
use std::result::Result as StdResult;
use url::Url;

pub(crate) struct LogoutCommand<'a> {
    wf_url: &'a str,
    wf_alias: &'a str,
    wf_access_token: &'a str,
}

impl<'a> TryFrom<&'a Cli> for LogoutCommand<'a> {
    type Error = anyhow::Error;
    fn try_from(cli: &'a Cli) -> StdResult<Self, Self::Error> {
        if let Some(Command::Logout { ref wf_url }) = cli.command {
            let wf_access_token = cli
                .wf_access_token
                .as_deref()
                .ok_or(anyhow!("WriteFreely access token required"))?;

            let wf_alias = cli
                .wf_alias
                .as_deref()
                .ok_or(anyhow!("WriteFreely alias required"))?;

            Ok(Self {
                wf_url,
                wf_access_token,
                wf_alias,
            })
        } else {
            Err(anyhow!("Not a valid logout command"))
        }
    }
}

impl<'a> From<&LogoutCommand<'a>> for WriteFreelyCredentials<'a> {
    fn from(cmd: &LogoutCommand<'a>) -> Self {
        WriteFreelyCredentials::AccessToken(cmd.wf_access_token)
    }
}

impl LogoutCommand<'_> {
    pub async fn execute(self) -> Result<()> {
        let wf_url = Url::parse(self.wf_url)?;
        let creds = WriteFreelyCredentials::from(&self);

        let wf_client = WriteFreely::new(&wf_url, &self.wf_alias, &creds).await?;
        wf_client.logout().await?;

        println!("Successfully logged out from {}", wf_url);

        Ok(())
    }
}
