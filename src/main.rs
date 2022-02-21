use crate::step::rust::Rust;
use crate::step::Step;
use anyhow::{Context, Result};
use console::Style;
use indicatif::MultiProgress;

mod step;

struct Colors {
    ok: Style,
    info: Style,
    err: Style,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let colors = Colors {
        ok: Style::new().green(),
        info: Style::new().yellow().italic(),
        err: Style::new().red(),
    };

    let multibar = MultiProgress::new();

    let rust_install = Rust::check(&colors)?;
    if rust_install.needs_install() {
        if let Some(output) = rust_install.install(&multibar, &colors)? {
            if !output.status.success() {
                eprintln!(
                    "critical rust install step failed: {}",
                    String::from_utf8(output.stderr)
                        .context("rust install stderr had invalid utf-8")?
                );
                eprintln!("exiting with code 1 because this is a critical step");
                std::process::exit(1);
            }
        }
    }

    multibar.join()?;

    Ok(())
}
