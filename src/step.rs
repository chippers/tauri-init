use crate::Colors;
use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use which::which;

pub(crate) mod rust;
pub(crate) mod tauri_cli;
pub(crate) mod xcode_clt;

fn ensure_sudo() -> Result<PathBuf> {
    let sudo = which("sudo").context("unable to find `sudo` binary")?;

    let mut cmd = Command::new(&sudo);
    cmd.arg("-v");
    if cmd
        .status()
        .context("failed to execute sudo validation")?
        .success()
    {
        Ok(sudo)
    } else {
        Err(anyhow!("failed to gain sudo access during validation"))
    }
}

pub(crate) fn sudo(cmd: &Command) -> Result<Command> {
    ensure_sudo().map(|sudo| {
        let mut elevated = Command::new(sudo);
        elevated.arg(cmd.get_program());
        elevated.args(cmd.get_args());
        elevated.envs(cmd.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))));

        if let Some(cwd) = cmd.get_current_dir() {
            elevated.current_dir(cwd);
        }

        elevated
    })
}

pub(crate) trait Step: Sized {
    fn check(colors: &Colors) -> Result<Self>;

    fn needs_install(&self) -> bool;

    fn install(self, colors: &Colors) -> Result<()>;
}
