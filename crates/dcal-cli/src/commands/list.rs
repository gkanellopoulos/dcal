use anyhow::Result;

pub fn run(status: Option<String>, stale: Option<String>) -> Result<()> {
    let _ = (status, stale);
    anyhow::bail!("not yet implemented: dcal list")
}
