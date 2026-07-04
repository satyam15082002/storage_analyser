//! Opens a path in Windows Explorer: a folder is opened directly, a file is opened with
//! Explorer scrolled to and highlighting that file (`/select,`).

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

pub fn open_in_explorer(path: &Path, is_dir: bool) -> Result<()> {
    let mut cmd = Command::new("explorer.exe");
    if is_dir {
        cmd.arg(path);
    } else {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path.as_os_str());
        cmd.arg(arg);
    }
    // explorer.exe often exits non-zero even on success, so only the spawn itself is checked.
    cmd.spawn().context("failed to launch explorer.exe")?;
    Ok(())
}
