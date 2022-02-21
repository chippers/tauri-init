use crate::step::Step;
use crate::Colors;
use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use ureq::AgentBuilder;
use which::which;

pub(crate) enum Rust {
    None,
    Stable { rustup: PathBuf },
    RustupStable { sh: PathBuf },
}

// todo: this should be expanded in the future with version detection
fn check_rustc() -> Result<PathBuf> {
    which("rustc").with_context(|| "rustc not found in PATH")
}

// todo: this should be expanded in the future with version detection
fn check_cargo() -> Result<PathBuf> {
    which("cargo").with_context(|| "cargo not found in PATH")
}

// todo: this should be expanded in the future with version detection
fn check_rustup() -> Result<PathBuf> {
    which("rustup").with_context(|| "rustup not found in PATH")
}

fn install_rustup_and_rust_stable(bar: &ProgressBar, sh: &Path) -> Result<Output> {
    let agent = AgentBuilder::new()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            " (v",
            env!("CARGO_PKG_VERSION"),
            ")"
        ))
        .build();

    bar.set_message("fetching rustup initialization script");

    // connect to the server and read the body
    let mut script = agent
        .get("https://sh.rustup.rs/")
        .call()
        .context("failed to download rustup shell script")?
        .into_reader();

    bar.set_message("preparing child shell to install rustup");

    // set up shell child
    let mut sh = Command::new(sh);
    sh.args(&["-s -y"]);
    let mut sh = sh.spawn()?;
    let mut stdin = sh
        .stdin
        .take()
        .context("unable to get stdin of sh command")?;

    // finally read the body (closing the connection) and write it directly to the sh stdin
    std::io::copy(&mut script, &mut stdin)
        .context("unable to pipe downloaded rustup script to sh stdin")?;

    bar.set_message("installing rustup");

    let output = sh
        .wait_with_output()
        .context("failed to run shell subcommand with rustup init shell script");

    bar.set_message("installed rustup and its default profile (stable rust)");
    output
}

fn install_rust_stable(bar: &ProgressBar, rustup: &Path) -> Result<Output> {
    bar.set_message("installing rust stable with rustup");
    let mut cmd = Command::new(rustup);
    cmd.args(&["toolchain", "install", "stable"]);
    let output = cmd
        .output()
        .context("failed to install rust stable with rustup");

    bar.set_message("installed rust stable");
    output
}

impl Step for Rust {
    fn check(colors: &Colors) -> Result<Self> {
        let rustc = check_rustc();
        let is_ok_rustc = rustc.is_ok();
        if !is_ok_rustc {
            eprintln!("{}", colors.err.apply_to("✗ `rustc` not found"));
        }

        let cargo = check_cargo();
        let is_ok_cargo = cargo.is_ok();
        if !is_ok_cargo {
            // todo: should this be to stdout?
            println!("{}", colors.err.apply_to("✗ `cargo` not found"));
        }

        let rustup = check_rustup();
        let is_ok_rustup = rustup.is_ok();
        if !is_ok_rustup {
            // todo: should this be to stdout?
            println!("{}", colors.err.apply_to("✗ `rustup` not found"));
        }

        let rust_install = if is_ok_rustc && is_ok_cargo && is_ok_rustup {
            // todo: this should probably print the found cargo version
            println!("{}", colors.ok.apply_to("✓ Rust installation found"));
            Self::None
        } else if is_ok_rustup && !(is_ok_rustc && is_ok_cargo) {
            // todo: in the future this would also cover having rustup but an outdated rustc/cargo
            // todo: should this be to stdout?
            println!(
                "{}",
                colors
                    .info
                    .apply_to("ℹ Rust stable will be installed with `rustup`")
            );

            let rustup = rustup.expect("failed to unwrap rustup path after checking it was ok");
            Self::Stable { rustup }
        } else {
            println!(
                "{}",
                colors
                    .info
                    .apply_to("ℹ rustup will be installed, along with Rust stable")
            );

            let sh = which("sh").context("unable to find shell")?;
            Self::RustupStable { sh }
        };

        Ok(rust_install)
    }

    fn needs_install(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn install(self, multibar: &MultiProgress, _colors: &Colors) -> Result<Option<Output>> {
        let bar = multibar.add(ProgressBar::new_spinner());
        let output = match self {
            Self::None => None,
            Self::Stable { rustup } => Some(install_rust_stable(&bar, &rustup)),
            Self::RustupStable { sh } => Some(install_rustup_and_rust_stable(&bar, &sh)),
        }
        .transpose();

        // make sure we clear the bar
        bar.finish();

        output
    }
}
