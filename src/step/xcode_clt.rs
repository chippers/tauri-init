use crate::step::sudo;
use crate::{Colors, Step};
use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;
use which::which;

// there's a lot of binaries we can test, but rustc specifically uses cc so just use that
const TESTED_PATH: &str = "/Library/Developer/CommandLineTools/usr/bin/cc";

// this temporary file prompts the 'softwareupdate' utility to list the Command Line Tools
const CLT_MAGIC_FILE: &str = "/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress";

// just rely on the output of the software update for now
const LABEL_PREFIX: &str = "* Label: ";

const DIR: &str = "/Library/Developer/CommandLineTools";

pub(crate) enum XcodeClt {
    None,
    Stealth,
}

fn install_stealth() -> Result<()> {
    let touch = which("touch").context("cannot find `touch` binary")?;
    let mut touch = Command::new(touch);
    touch.arg(CLT_MAGIC_FILE);
    let mut touch = sudo(&touch)?;
    if !touch
        .status()
        .context("unable to touch touch with sudo")?
        .success()
    {
        return Err(anyhow!("failed to touch Command Line Tools magic file"));
    }

    let softwareupdate = which("softwareupdate").context("cannot find `softwareupdate` binary")?;
    let mut list = Command::new(&softwareupdate);
    list.arg("-l");
    let output = list.output()?;
    if !output.status.success() {
        return Err(anyhow!("failed to list software updates"));
    }
    let output =
        String::from_utf8(output.stdout).context("softwareupdate -l gave us invalid utf-8")?;

    let possible_labels: Vec<String> = output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with(LABEL_PREFIX)
                && trimmed.contains("Command Line Tools")
                && !trimmed.contains("beta")
            {
                Some((&trimmed[LABEL_PREFIX.len()..]).to_string())
            } else {
                None
            }
        })
        .collect();

    // ensure we have exactly 1 matching label
    let label = match possible_labels.len() {
        1 => &possible_labels[0],
        0 => {
            return Err(anyhow!(
                "no valid Command Line Tools labels found in softwareupdater"
            ))
        }
        _ => {
            return Err(anyhow!(
                "found multiple valid Command Line Tool labels: {:?}",
                possible_labels
            ))
        }
    };

    let mut install = Command::new(&softwareupdate);
    install.arg("-i");
    install.arg(label);
    let install = sudo(&install)?.status()?;
    if !install.success() {
        return Err(anyhow!(
            "failed to install label \"{}\" with `softwareupdate`",
            label
        ));
    }

    let xcode_select = which("xcode-select").context("unable to find `xcode-select` binary")?;
    let mut switch = Command::new(xcode_select);
    switch.arg("--switch");
    switch.arg(DIR);
    let switch = sudo(&switch)?.status()?;
    if !switch.success() {
        return Err(anyhow!(
            "failed to switch Xcode Command Line Tools directory to {}",
            DIR
        ));
    }

    let rm = which("rm").context("unable to find `rm` binary")?;
    let mut rm = Command::new(rm);
    rm.arg("-f");
    rm.arg(CLT_MAGIC_FILE);
    let rm = sudo(&rm)?.status()?;
    if !rm.success() {
        return Err(anyhow!("failed to remove magic Command Line Tools file"));
    }

    Ok(())
}

impl Step for XcodeClt {
    fn check(colors: &Colors) -> Result<Self> {
        if Path::new(TESTED_PATH).exists() {
            println!("{}", colors.ok.apply_to("✓ Xcode Command Line Tools found"));
            Ok(Self::None)
        } else {
            println!(
                "{}",
                colors
                    .info
                    .apply_to("ℹ Xcode Command Line Tools will be installed (requires sudo)")
            );
            Ok(Self::Stealth)
        }
    }

    fn needs_install(&self) -> bool {
        !matches!(self, Self::None)
    }

    fn install(self, _: &Colors) -> Result<()> {
        match self {
            Self::None => Ok(()),
            Self::Stealth => install_stealth(),
        }
    }
}
