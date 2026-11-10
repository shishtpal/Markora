//! Open web links through the OS, while keeping the editor's webview in place.
pub fn open_web_link(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:"))
        || url.chars().any(char::is_control)
    {
        return Err("Only HTTP, HTTPS and email links can open externally.".into());
    }
    #[cfg(windows)]
    {
        let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        // SAFETY: all string arguments are null-terminated and live for the call.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                wide.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("Windows could not open this link.".into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let command = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(command)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
