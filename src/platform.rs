//! Operating-system font operations: install for the current user, temporary
//! activation for this session, uninstall/deactivate and revealing files.

use std::path::{Path, PathBuf};

use crate::fontdb::{norm, user_fonts_dir, FontFace, InstalledInfo};

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn broadcast_font_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_FONTCHANGE};
    unsafe {
        let mut res = 0usize;
        SendMessageTimeoutW(HWND_BROADCAST, WM_FONTCHANGE, 0, 0, SMTO_ABORTIFHUNG, 1000, &mut res);
    }
}

/// Make the font usable by other programs until sign-out (no copy, no registry).
#[cfg(windows)]
pub fn activate_temporarily(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::AddFontResourceW;
    let w = wide(&path.to_string_lossy());
    let n = unsafe { AddFontResourceW(w.as_ptr()) };
    if n > 0 {
        broadcast_font_change();
        Ok(())
    } else {
        Err(format!("Windows could not activate {}", path.display()))
    }
}

#[cfg(windows)]
pub fn deactivate(path: &Path) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::RemoveFontResourceW;
    let w = wide(&path.to_string_lossy());
    let ok = unsafe { RemoveFontResourceW(w.as_ptr()) };
    broadcast_font_change();
    if ok != 0 { Ok(()) } else { Err(format!("{} was not active", path.display())) }
}

/// Install for the current user: copy into the per-user Fonts folder and register it.
#[cfg(windows)]
pub fn install_for_user(face: &FontFace) -> Result<PathBuf, String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let dir = user_fonts_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(&face.file_name);
    if norm(&dest) != norm(&face.key.path) {
        std::fs::copy(&face.key.path, &dest).map_err(|e| format!("Copy failed: {e}"))?;
    }
    let kind = if face.is_postscript() { "OpenType" } else { "TrueType" };
    let value_name = format!("{} ({kind})", face.display_name());
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts")
        .map_err(|e| e.to_string())?;
    key.set_value(&value_name, &dest.to_string_lossy().to_string()).map_err(|e| e.to_string())?;
    let _ = activate_temporarily(&dest);
    Ok(dest)
}

/// Remove a per-user installed font. System-wide fonts need administrator rights.
#[cfg(windows)]
pub fn uninstall(face: &FontFace, info: &InstalledInfo) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;
    let n = norm(&face.key.path);
    if !info.user_files.contains(&n) {
        return Err(format!(
            "{} is installed for all users. Removing it needs administrator rights; use Windows Settings › Fonts.",
            face.display_name()
        ));
    }
    let _ = deactivate(&face.key.path);
    if let Some(name) = info.value_names.get(&n) {
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts", KEY_ALL_ACCESS)
        {
            let _ = key.delete_value(name);
        }
    }
    if norm(&face.key.path).starts_with(&norm(&user_fonts_dir())) {
        let _ = std::fs::remove_file(&face.key.path);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn activate_temporarily(_path: &Path) -> Result<(), String> {
    Err("Temporary activation is only available on Windows".into())
}
#[cfg(not(windows))]
pub fn deactivate(_path: &Path) -> Result<(), String> {
    Err("Only available on Windows".into())
}
#[cfg(not(windows))]
pub fn install_for_user(_face: &FontFace) -> Result<PathBuf, String> {
    Err("Only available on Windows".into())
}
#[cfg(not(windows))]
pub fn uninstall(_face: &FontFace, _info: &InstalledInfo) -> Result<(), String> {
    Err("Only available on Windows".into())
}

pub fn reveal(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(p) = path.parent() {
            let _ = std::process::Command::new("xdg-open").arg(p).spawn();
        }
    }
}

pub fn open_url(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Settings file; `FONTABULOUS_CONFIG` points at another file (portable use).
pub fn config_file() -> PathBuf {
    match std::env::var("FONTABULOUS_CONFIG") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => config_dir().join("config.json"),
    }
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    base.join("Fontabulous")
}
