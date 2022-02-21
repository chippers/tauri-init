use crate::{Colors, Step};
use anyhow::{anyhow, Context, Result};
use dirs::home_dir;
use std::env::current_dir;
use std::path::PathBuf;
use std::process::Command;
use which::{which, which_in};

// todo: we need to figure out a way to specify locked in the future. likely through cli args
// we will also eventually have "auto" where it doesn't specify a version, but that will not work
// properly until we have a stable release (i think).
const EXACT: &str = "1.0.0-rc.6";

pub(crate) enum TauriCli {
    None,
    Exact { version: String },
}

// cargo may not be in the path yet
fn cargo() -> Result<PathBuf> {
    which("cargo").or_else(|_| {
        current_dir()
            .context("unable to find current working directory")
            .and_then(|cwd| {
                which_in("cargo", home_dir(), cwd)
                    .context("unable to find cargo in default rustup path")
            })
    })
}

fn install_locked(version: &str) -> Result<()> {
    let mut cargo = Command::new(cargo()?);
    cargo.arg("install");
    cargo.arg("tauri-cli");
    cargo.arg("--version");
    cargo.arg(version);
    if cargo
        .status()
        .context("unable to install tauri-cli with `cargo-install`")?
        .success()
    {
        Ok(())
    } else {
        Err(anyhow!("unable to install tauri-cli with `cargo-install`"))
    }
}

impl Step for TauriCli {
    fn check(colors: &Colors) -> Result<Self> {
        let mut check = Command::new("cargo");
        check.arg("help");
        check.arg("tauri");
        if check.status()?.success() {
            println!("{}", colors.ok.apply_to("✓ Tauri CLI found"));
            Ok(Self::None)
        } else {
            println!("{}", colors.info.apply_to("ℹ Tauri CLI will be installed"));
            Ok(Self::Exact {
                version: EXACT.into(),
            })
        }
    }

    fn needs_install(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn install(self, _: &Colors) -> Result<()> {
        match self {
            Self::None => Ok(()),
            Self::Exact { version } => install_locked(&version),
        }
    }
}
