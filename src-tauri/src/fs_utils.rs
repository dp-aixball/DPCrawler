use std::path::PathBuf;

/// Detect if running from a development project directory
/// (has python/crawler.py), or installed system-wide.
pub fn is_dev_mode() -> bool {
    let dev_root = dev_project_root();
    dev_root.join("python").join("crawler.py").exists()
}

/// Development project root (CWD-based, for `cargo tauri dev`)
pub fn dev_project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

/// Data directory for config/output (works both dev and installed)
/// Dev: project root; Installed: ~/.config/dpcrawler/ or %APPDATA%/dpcrawler/
pub fn data_dir() -> PathBuf {
    if is_dev_mode() {
        dev_project_root()
    } else {
        let dir = dirs::data_dir()
            .map(|d| d.join("dpcrawler"))
            .unwrap_or_else(|| {
                #[cfg(unix)]
                { PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())).join(".config").join("dpcrawler") }
                #[cfg(windows)]
                { PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string())).join("dpcrawler") }
            });
        std::fs::create_dir_all(&dir).ok();
        dir
    }
}

/// Resolve a relative path against the data directory
pub fn resolve_path(relative: &str) -> PathBuf {
    data_dir().join(relative)
}
