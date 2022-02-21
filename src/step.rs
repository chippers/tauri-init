use crate::Colors;
use anyhow::Result;
use indicatif::MultiProgress;
use std::process::Output;

pub(crate) mod rust;
//pub(crate) mod tauri_cli;
//pub(crate) mod xcode_clt;

pub(crate) trait Step: Sized {
    fn check(colors: &Colors) -> Result<Self>;

    fn needs_install(&self) -> bool;

    fn install(self, multibar: &MultiProgress, colors: &Colors) -> Result<Option<Output>>;
}
