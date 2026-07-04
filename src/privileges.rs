//! Windows privilege/elevation helpers. The MFT engine needs `SeBackupPrivilege` (to open a
//! raw volume handle and read protected file records) and an elevated token (volume access
//! is restricted to administrators regardless of privileges held).

use anyhow::{bail, Result};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES,
    TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn enable_privilege(name: &str) -> Result<()> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token)?;

        let mut luid = LUID::default();
        let name_w = wide(name);
        LookupPrivilegeValueW(PCWSTR::null(), PCWSTR(name_w.as_ptr()), &mut luid)?;

        let mut tp = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [windows::Win32::Security::LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };

        let ok = AdjustTokenPrivileges(token, false, Some(&mut tp), 0, None, None);
        let _ = CloseHandle(token);
        ok?;
        Ok(())
    }
}

/// Enables SeBackupPrivilege (bypass read ACL checks) and SeRestorePrivilege for the
/// current process token. Both require the process to already be elevated to succeed.
pub fn enable_backup_privileges() -> Result<()> {
    enable_privilege("SeBackupPrivilege")?;
    enable_privilege("SeRestorePrivilege")?;
    Ok(())
}

pub fn is_elevated() -> bool {
    use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION};
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        let _ = CloseHandle(token);
        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

/// Relaunches the current executable elevated (UAC prompt) with the same arguments.
pub fn relaunch_elevated() -> Result<()> {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let exe = std::env::current_exe()?;
    let exe_w = wide(&exe.to_string_lossy());
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args_joined = args.join(" ");
    let args_w = wide(&args_joined);
    let verb = wide("runas");

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR(args_w.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // ShellExecuteW returns a value > 32 on success (HINSTANCE-shaped return code).
    if (result.0 as isize) <= 32 {
        bail!("failed to relaunch elevated (user may have declined the UAC prompt)");
    }
    Ok(())
}
