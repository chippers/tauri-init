use crate::step::rust::Rust;
use crate::step::tauri_cli::TauriCli;
use crate::step::xcode_clt::XcodeClt;
use crate::step::Step;
use anyhow::Result;
use console::Style;

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

    let rust_install = Rust::check(&colors)?;
    if rust_install.needs_install() {
        critical_step(rust_install.install(&colors), "rust");
    }

    // todo: this should be macOS only (but tauri-init is also macOS only for right now now now)
    let xcode_clt = XcodeClt::check(&colors)?;
    if xcode_clt.needs_install() {
        critical_step(xcode_clt.install(&colors), "Xcode Command Line Tools")
    }

    // we haven't exited yet, so assume everything is good
    let tauri_cli = TauriCli::check(&colors)?;
    if tauri_cli.needs_install() {
        tauri_cli.install(&colors)?
    }

    println!("Success! You should be able to re-run this command in a new terminal to see all the checks success.");

    Ok(())
}

fn critical_step(result: Result<()>, msg: &str) {
    if let Err(e) = result {
        eprintln!("critical {} install step failed: {}", msg, e);
        eprintln!("exiting with code 1 because this is a critical step");
        std::process::exit(1);
    }
}
