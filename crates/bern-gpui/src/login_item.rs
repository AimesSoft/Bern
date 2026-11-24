//! "Open at Login" (`Switch(login_item: true)` and `StatusItem(login_item: true)`):
//! - macOS: `SMAppService.mainAppService` (with LaunchAgent plist fallback when unbundled).
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (+ `StartupApproved`).
//! - Linux: `$XDG_CONFIG_HOME/autostart/<bundle_id>.desktop`.

use crate::ctx::Model;
use crate::protocol::Value;
use std::cell::Cell;
use std::rc::Rc;

thread_local! {
    /// Cached login-item state so rendering never hits XPC/registry/filesystem on every frame.
    static CACHED: Cell<Option<bool>> = const { Cell::new(None) };
}

/// Whether the application is currently registered to open at login.
pub(crate) fn enabled() -> bool {
    CACHED.with(|cell| {
        cell.get().unwrap_or_else(|| {
            let on = platform::enabled();
            cell.set(Some(on));
            on
        })
    })
}

/// Refresh cached state from the OS and redraw if it changed.
pub(crate) fn refresh() -> bool {
    let now = platform::enabled();
    let changed = CACHED.with(|cell| {
        let prev = cell.replace(Some(now));
        prev != Some(now)
    });
    if changed {
        crate::host::redraw();
    }
    now
}

/// Toggle "Open at Login" in the OS and redraw.
pub(crate) fn set(on: bool) {
    platform::set(on);
    let now = platform::enabled();
    CACHED.with(|cell| cell.set(Some(now)));
    crate::host::redraw();
}

/// `Switch(login_item: true)`: binds a switch to the system "Open at Login" setting.
pub fn login_model() -> Model {
    Model::Controlled(
        Value::Bool(enabled()),
        Rc::new(|_, value| set(crate::value::truthy(&value))),
    )
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use std::path::PathBuf;

    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    const STATUS_ENABLED: isize = 1;
    const STATUS_REQUIRES_APPROVAL: isize = 2;

    fn sm_app_service() -> Option<(&'static AnyClass, *mut AnyObject)> {
        if !crate::host::bundled() {
            return None;
        }
        let cls = AnyClass::get(c"SMAppService")?;
        // SAFETY: `SMAppService.mainAppService` returns a singleton instance on macOS 13+.
        let svc: *mut AnyObject = unsafe { msg_send![cls, mainAppService] };
        (!svc.is_null()).then_some((cls, svc))
    }

    fn plist_path() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        let bundle = crate::host::host().def.bundle_id;
        Some(
            PathBuf::from(home)
                .join("Library/LaunchAgents")
                .join(format!("{bundle}.plist")),
        )
    }

    pub(super) fn enabled() -> bool {
        if let Some((_, svc)) = sm_app_service() {
            // SAFETY: `svc` is a valid `SMAppService` instance.
            let status: isize = unsafe { msg_send![svc, status] };
            return status == STATUS_ENABLED;
        }
        let Some(path) = plist_path() else {
            return false;
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        contents.contains(exe.to_string_lossy().as_ref())
    }

    pub(super) fn set(on: bool) {
        if let Some((cls, svc)) = sm_app_service() {
            let mut err: *mut AnyObject = std::ptr::null_mut();
            // SAFETY: `registerAndReturnError:` / `unregisterAndReturnError:` are standard macOS 13+ methods.
            let _ok: bool = if on {
                unsafe { msg_send![svc, registerAndReturnError: &mut err] }
            } else {
                unsafe { msg_send![svc, unregisterAndReturnError: &mut err] }
            };
            if on {
                let status: isize = unsafe { msg_send![svc, status] };
                if status == STATUS_REQUIRES_APPROVAL {
                    let _: () = unsafe { msg_send![cls, openSystemSettingsLoginItems] };
                }
            }
            return;
        }
        let Some(path) = plist_path() else {
            return;
        };
        if !on {
            let _ = std::fs::remove_file(&path);
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let label = crate::host::host().def.bundle_id;
        let xml_escape = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        };
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--at-login</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
            xml_escape(label),
            xml_escape(&exe.to_string_lossy()),
        );
        let _ = std::fs::write(&path, plist);
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use win32::Win32::Foundation::ERROR_SUCCESS;
    use win32::Win32::System::LibraryLoader::GetModuleFileNameW;
    use win32::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegDeleteKeyValueW,
        RegGetValueW, RegSetKeyValueW,
    };
    use win32::core::PCWSTR;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn run_key() -> Vec<u16> {
        wide(r"Software\Microsoft\Windows\CurrentVersion\Run")
    }

    fn approved_key() -> Vec<u16> {
        wide(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run")
    }

    fn value_name() -> Vec<u16> {
        wide(crate::host::host().def.bundle_id)
    }

    fn command_string() -> Option<String> {
        let mut buf = vec![0u16; 260];
        loop {
            // SAFETY: writing into our owned wide buffer.
            let len = unsafe { GetModuleFileNameW(None, &mut buf) } as usize;
            if len == 0 {
                return None;
            }
            if len < buf.len() {
                buf.truncate(len);
                break;
            }
            buf.resize(buf.len() * 2, 0);
        }
        let exe = String::from_utf16_lossy(&buf);
        Some(format!("\"{exe}\" --at-login"))
    }

    pub(super) fn enabled() -> bool {
        let Some(expected) = command_string() else {
            return false;
        };
        let key = run_key();
        let name = value_name();
        let mut size: u32 = 0;
        // SAFETY: querying registry string size.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                None,
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS || size < 2 {
            return false;
        }
        let mut data = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: reading registry string into owned buffer.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(key.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(data.as_mut_ptr() as *mut c_void),
                Some(&mut size),
            )
        };
        if status != ERROR_SUCCESS {
            return false;
        }
        let nul = data.iter().position(|&c| c == 0).unwrap_or(data.len());
        let current = String::from_utf16_lossy(&data[..nul]);
        if !current.eq_ignore_ascii_case(&expected) {
            return false;
        }
        let approved = approved_key();
        let mut bytes = [0u8; 64];
        let mut bsize = bytes.len() as u32;
        // SAFETY: reading StartupApproved binary flag if present.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                PCWSTR(approved.as_ptr()),
                PCWSTR(name.as_ptr()),
                RRF_RT_REG_BINARY,
                None,
                Some(bytes.as_mut_ptr() as *mut c_void),
                Some(&mut bsize),
            )
        };
        let disabled = status == ERROR_SUCCESS && bsize > 0 && (bytes[0] & 1) != 0;
        !disabled
    }

    pub(super) fn set(on: bool) {
        let key = run_key();
        let approved = approved_key();
        let name = value_name();
        if on {
            if let Some(cmd) = command_string() {
                let w = wide(&cmd);
                let byte_len = (w.len() * std::mem::size_of::<u16>()) as u32;
                // SAFETY: writing UTF-16 NUL-terminated command under HKCU Run.
                let status = unsafe {
                    RegSetKeyValueW(
                        HKEY_CURRENT_USER,
                        PCWSTR(key.as_ptr()),
                        PCWSTR(name.as_ptr()),
                        REG_SZ.0,
                        Some(w.as_ptr() as *const c_void),
                        byte_len,
                    )
                };
                if status != ERROR_SUCCESS {
                    eprintln!("bern: cannot add application to startup apps: {status:?}");
                }
            }
        } else {
            // SAFETY: removing our own Run value if present.
            let _ = unsafe {
                RegDeleteKeyValueW(
                    HKEY_CURRENT_USER,
                    PCWSTR(key.as_ptr()),
                    PCWSTR(name.as_ptr()),
                )
            };
        }
        // Clear any Task Manager disabled override for our value.
        let _ = unsafe {
            RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                PCWSTR(approved.as_ptr()),
                PCWSTR(name.as_ptr()),
            )
        };
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use std::path::{Path, PathBuf};

    fn entry_path() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        let bundle = crate::host::host().def.bundle_id;
        Some(base.join("autostart").join(format!("{bundle}.desktop")))
    }

    fn exec_line(executable: &Path) -> String {
        let mut quoted = String::from("\"");
        for ch in executable.to_string_lossy().chars() {
            match ch {
                '"' | '`' | '$' | '\\' => {
                    quoted.push('\\');
                    quoted.push(ch);
                }
                '%' => quoted.push_str("%%"),
                _ => quoted.push(ch),
            }
        }
        quoted.push_str("\" --at-login");
        quoted
    }

    pub(super) fn enabled() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let Some(path) = entry_path() else {
            return false;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        let mut in_group = false;
        let mut hidden = false;
        let mut gnome = true;
        let mut exec = None;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_group = line == "[Desktop Entry]";
                continue;
            }
            if !in_group {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "Hidden" => hidden = v.trim().eq_ignore_ascii_case("true"),
                    "X-GNOME-Autostart-enabled" => {
                        gnome = !v.trim().eq_ignore_ascii_case("false");
                    }
                    "Exec" => exec = Some(v.trim().to_string()),
                    _ => {}
                }
            }
        }
        exec.as_deref() == Some(exec_line(&exe).as_str()) && !hidden && gnome
    }

    pub(super) fn set(on: bool) {
        let Some(path) = entry_path() else {
            return;
        };
        if !on {
            if let Err(err) = std::fs::remove_file(&path)
                && err.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("bern: cannot remove {}: {err}", path.display());
            }
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let name = crate::text::tr(crate::host::host().def.name);
        let content = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nExec={}\nX-GNOME-Autostart-enabled=true\n",
            exec_line(&exe)
        );
        if let Err(err) = std::fs::write(&path, content) {
            eprintln!("bern: cannot write {}: {err}", path.display());
        }
    }
}
