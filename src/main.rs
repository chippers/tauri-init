use anyhow::{anyhow, Context, Result};
use console::Style;
use indicatif::{MultiProgress, ProgressBar};
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::{tempdir, TempDir};
use which::which;

struct Colors {
    ok: Style,
    info: Style,
    err: Style,
}

enum RustInstall {
    None,
    Stable,
    RustupStable,
}

enum XcodeInstall {
    None,
    All,
}

macro_rules! run_script {
    ($msg:literal, $desc:literal, $multibar:ident, $tmp:ident, $fn:ident) => {
        let bar = ProgressBar::new_spinner().with_message($msg);
        let bar = $multibar.add(bar);
        let script = $fn($tmp)?;

        // set up status watcher
        let (tx, rx) = channel();
        let _ = watch_status(&script, bar, tx, $desc)?;

        // wait for watcher
        rx.recv()?;

        // actually run the script
        open_script_in_new_terminal(&script)?;
    };
}

// wrap included scripts for status file detection
macro_rules! include_script {
    ($path:literal) => {
        concat!(
            "#!/usr/bin/env sh\n",
            include_str!($path),
            "echo $? > \"$0.status\"\n"
        )
    };
}

fn main() {
    let tmp = match tempdir() {
        Ok(tmp) => tmp,
        Err(e) => {
            eprintln!("unable to create temporary directory: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = run(&tmp) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn run(tmp: &TempDir) -> Result<()> {
    let colors = Colors {
        ok: Style::new().green(),
        info: Style::new().yellow().italic(),
        err: Style::new().red(),
    };

    let rust_install = check_rust(&colors);

    // todo: this should be on macos only
    let xcode_install = check_xcode_tools(&colors)?;

    let multibar = MultiProgress::new();

    match rust_install {
        RustInstall::None => {}
        RustInstall::Stable => {
            run_script!(
                "Installing Rust stable",
                "Rust stable",
                multibar,
                tmp,
                install_stable_rust_with_rustup_script
            );
        }
        RustInstall::RustupStable => {
            run_script!(
                "Installing Rustup along with Rust stable",
                "Rustup and Rust stable",
                multibar,
                tmp,
                install_rustup_and_stable_rust_script
            );
        }
    }

    match xcode_install {
        XcodeInstall::None => {}
        XcodeInstall::All => {
            run_script!(
                "Installing Xcode command line tools",
                "Xcode tools",
                multibar,
                tmp,
                install_xcode_tools_script
            );
        }
    }

    multibar.join()?;

    Ok(())
}

// todo: this should be expanded in the future with version detection
fn check_rustc() -> Result<()> {
    which("rustc")
        .map(|_| ())
        .with_context(|| "rustc not found in PATH")
}

// todo: this should be expanded in the future with version detection
fn check_cargo() -> Result<()> {
    which("cargo")
        .map(|_| ())
        .with_context(|| "cargo not found in PATH")
}

// todo: this should be expanded in the future with version detection
fn check_rustup() -> Result<()> {
    which("rustup")
        .map(|_| ())
        .with_context(|| "rustup not found in PATH")
}

fn check_rust(colors: &Colors) -> RustInstall {
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

    if is_ok_rustc && is_ok_cargo && is_ok_rustup {
        // todo: this should probably print the found cargo version
        println!("{}", colors.ok.apply_to("✓ Rust installation found"));
        RustInstall::None
    } else if is_ok_rustup && !(is_ok_rustc && is_ok_cargo) {
        // todo: in the future this would also cover having rustup but an outdated rustc/cargo
        // todo: should this be to stdout?
        println!(
            "{}",
            colors
                .info
                .apply_to("ℹ Rust stable will be installed with `rustup`")
        );
        RustInstall::Stable
    } else {
        println!(
            "{}",
            colors
                .info
                .apply_to("ℹ rustup will be installed, along with Rust stable")
        );
        RustInstall::RustupStable
    }
}

// macos only right now, but it's macos specific
fn check_xcode_tools(colors: &Colors) -> Result<XcodeInstall> {
    let xcode_select = which("xcode-select").with_context(|| "xcode-select not found in PATH")?;
    let mut cmd = Command::new(xcode_select);
    cmd.arg("-p");
    piped(&mut cmd);
    Ok(if cmd.status()?.success() {
        println!("{}", colors.ok.apply_to("✓ Xcode command line tools found"));
        XcodeInstall::None
    } else {
        // todo: should this be to stdout?
        println!(
            "{}",
            colors
                .info
                .apply_to("ℹ Xcode command line tools will be installed")
        );
        XcodeInstall::All
    })
}

fn install_stable_rust_with_rustup_script(tmp: &TempDir) -> Result<PathBuf> {
    static SCRIPT: &str = include_script!("scripts/install-rust.sh");
    write_script(tmp.path().join("install-rust.sh"), SCRIPT)
}

fn install_rustup_and_stable_rust_script(tmp: &TempDir) -> Result<PathBuf> {
    static SCRIPT: &str = include_script!("scripts/install-rustup.sh");
    write_script(tmp.path().join("install-rustup.sh"), SCRIPT)
}

fn install_xcode_tools_script(tmp: &TempDir) -> Result<PathBuf> {
    static SCRIPT: &str = include_script!("scripts/install-xcode-tools.sh");
    write_script(tmp.path().join("install-xcode-tools.sh"), SCRIPT)
}

// macos only right now
fn open_script_in_new_terminal(script: &Path) -> Result<()> {
    let mut cmd = Command::new("open");
    cmd.arg("-a");
    cmd.arg("Terminal.app");
    cmd.arg(script.canonicalize()?);

    if cmd.status()?.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "unable to open {} in new Terminal",
            script.display()
        ))
    }
}

fn write_script(path: PathBuf, script: &'static str) -> Result<PathBuf> {
    let mut f = OpenOptions::new()
        .create(true)
        .write(true)
        .mode(0o744)
        .open(&path)?;
    writeln!(f, "{}", script)?;
    Ok(path)
}

fn watch_status(
    script_path: &Path,
    bar: ProgressBar,
    ready: Sender<()>,
    desc: &'static str,
) -> Result<JoinHandle<Result<()>>> {
    let status_path = PathBuf::from(format!("{}.status", script_path.display()));
    let status_parent = status_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("unable to get script parent directory"))?;
    Ok(std::thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = watcher(tx, Duration::from_secs(1))?;

        // we watch the parent because the status file doesn't currently exist
        watcher.watch(status_parent, RecursiveMode::Recursive)?;

        // let our main thread know it's ok to spawn the script
        ready.send(())?;

        loop {
            match rx.recv() {
                Err(_) => {
                    bar.finish_with_message("Error: unable to watch file for status output");
                    break;
                }
                Ok(DebouncedEvent::Create(path)) if path == status_path.canonicalize()? => {
                    if std::fs::read_to_string(&path)?.trim() == "0" {
                        bar.finish_with_message(format!("Successfully installed {}", desc))
                    } else {
                        bar.finish_with_message(format!(
                            "Unspecified error while installing {}",
                            desc
                        ))
                    }
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }))
}

fn piped(cmd: &mut Command) {
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
}
