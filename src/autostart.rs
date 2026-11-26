//! "Start with Windows": a `Clock` value under the current user's `Run`
//! registry key.
//!
//! The registry is the only source of truth, so the Clock tab, the tray menu
//! and the installer (which writes the same value, see `installer/clock.iss`)
//! always agree. No admin rights are needed for `HKEY_CURRENT_USER`.

use std::path::Path;

/// Passed by the `Run` entry, so a start at login goes straight to the tray.
pub const TRAY_FLAG: &str = "--tray";

/// The `Run` value's data: the quoted exe path plus [`TRAY_FLAG`].
pub fn command_line(exe: &Path) -> String {
    format!("\"{}\" {TRAY_FLAG}", exe.display())
}

pub use imp::{is_enabled, set};

#[cfg(windows)]
mod imp {
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "Clock";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    /// True when the `Run` entry exists.
    pub fn is_enabled() -> bool {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        // SAFETY: both names are NUL-terminated and outlive the call. No data
        // buffer is passed, so this only checks that the value exists.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        status == ERROR_SUCCESS
    }

    /// Adds the `Run` entry for this exe, or removes it.
    pub fn set(enabled: bool) -> Result<(), String> {
        let (key, value) = (wide(RUN_KEY), wide(VALUE));
        let status = if enabled {
            let exe = std::env::current_exe()
                .map_err(|err| format!("could not find the app's own path: {err}"))?;
            let data = wide(&super::command_line(&exe));
            let bytes = u32::try_from(data.len() * 2).map_err(|_| "app path is too long")?;
            // SAFETY: names and data are NUL-terminated UTF-16 that outlive the
            // call, and `bytes` is the data's exact size including the NUL.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    value.as_ptr(),
                    REG_SZ,
                    data.as_ptr().cast(),
                    bytes,
                )
            }
        } else {
            // SAFETY: both names are NUL-terminated and outlive the call.
            match unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) } {
                ERROR_FILE_NOT_FOUND => ERROR_SUCCESS,
                status => status,
            }
        };

        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!(
                "could not update the Start with Windows registry entry (error {status})"
            ))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_enabled() -> bool {
        false
    }

    pub fn set(_enabled: bool) -> Result<(), String> {
        Err("Start with Windows only works on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_the_path_and_starts_in_the_tray() {
        let line = command_line(Path::new(r"C:\Program Files\Clock\clock-app.exe"));
        assert_eq!(line, r#""C:\Program Files\Clock\clock-app.exe" --tray"#);
    }
}
