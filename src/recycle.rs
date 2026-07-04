//! Sends a file or folder to the Recycle Bin (reversible), rather than permanently deleting
//! it — used by the TUI's delete action so a mis-press doesn't cause irreversible data loss.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use anyhow::{bail, Result};
use windows::core::PCWSTR;
use windows::Win32::UI::Shell::{
    SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NO_UI, FO_DELETE, SHFILEOPSTRUCTW,
};

pub fn send_to_recycle_bin(path: &Path) -> Result<()> {
    // SHFileOperationW requires the path buffer to be double-NUL-terminated.
    let mut path_w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    path_w.push(0);

    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR(path_w.as_ptr()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NO_UI.0) as u16,
        ..Default::default()
    };

    let result = unsafe { SHFileOperationW(&mut op) };
    if result != 0 || op.fAnyOperationsAborted.as_bool() {
        bail!("failed to send '{}' to the Recycle Bin (error code {})", path.display(), result);
    }
    Ok(())
}
