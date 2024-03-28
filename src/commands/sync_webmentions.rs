use anyhow::Result;

pub(crate) struct SyncWebmentionsCommand<'a> {
    webmention_io_url: &'a str,
    webmention_io_token: &'a str,
}

// How will this work? This tool is stateless. The easiest solution is
// to require last ID passed in, but that doesn't really make sense.
// We can have it operate on a directory of comment files, and store
// the state in the files themselves. Replicate the logic in the nu
// shell stuff.

impl SyncWebmentionsCommand<'_> {
    pub async fn execute(self) -> Result<()> {
        Ok(())
    }
}
