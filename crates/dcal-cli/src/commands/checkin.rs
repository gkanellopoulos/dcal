use anyhow::Result;

pub fn run(target: Option<String>, auto: bool, project_from_cwd: bool) -> Result<()> {
    let _ = (target, auto, project_from_cwd);
    anyhow::bail!("not yet implemented: dcal checkin")
}
