use crate::{
    wf::{WriteFreely, WriteFreelyCredentials},
    Cli, Command,
};
use anyhow::{anyhow, Result};
use std::result::Result as StdResult;
use url::Url;

pub(crate) struct LoginCommand<'a> {
    wf_url: &'a str,
    username: &'a str,
    password: &'a str,
}

impl<'a> TryFrom<&'a Cli> for LoginCommand<'a> {
    type Error = anyhow::Error;

    fn try_from(cli: &'a Cli) -> StdResult<Self, Self::Error> {
        if let Some(Command::Login {
            ref wf_url,
            ref username,
            ref password,
        }) = cli.command
        {
            Ok(Self {
                wf_url,
                username,
                password,
            })
        } else {
            Err(anyhow!("Not a valid login command"))
        }
    }
}

impl<'a> From<&'a LoginCommand<'a>> for WriteFreelyCredentials<'a> {
    fn from(cmd: &'a LoginCommand<'a>) -> Self {
        WriteFreelyCredentials::UsernameAndPassword(cmd.username, cmd.password)
    }
}

impl LoginCommand<'_> {
    pub async fn execute(self) -> Result<()> {
        let wf_url = Url::parse(self.wf_url)?;
        let creds = WriteFreelyCredentials::from(&self);
        let wf_client = WriteFreely::new(&wf_url, &self.username, &creds).await?;

        println!(
            "{}",
            wf_client.access_token().unwrap_or("[No Token Returned]")
        );

        Ok(())
    }
}
