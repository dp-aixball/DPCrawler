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
                {
                    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()))
                        .join(".config")
                        .join("dpcrawler")
                }
                #[cfg(windows)]
                {
                    PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()))
                        .join("dpcrawler")
                }
            });
        std::fs::create_dir_all(&dir).ok();
        dir
    }
}

/// Resolve a relative path against the data directory
pub fn resolve_path(relative: &str) -> PathBuf {
    data_dir().join(relative)
}

pub fn read_site_index_core(output_dir: &str, site_name: &str) -> Result<String, String> {
    let base = resolve_path(output_dir);
    let site_dir = base.join(site_name);
    let index_path = site_dir.join("index.json");

    let mut prefixed_tree = serde_json::Map::new();

    // Try reading from index.json first
    if index_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&index_path) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(tree) = data.get("file_tree").and_then(|t| t.as_object()) {
                    for (name, meta) in tree {
                        let full_name = format!("{}/{}", site_name, name);
                        prefixed_tree.insert(full_name, meta.clone());
                    }
                }
            }
        }
    } else {
        // Fallback: traverse meta folder to build index dynamically
        let meta_dir = site_dir.join("meta");
        if meta_dir.exists() && meta_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&meta_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() && path.extension().unwrap_or_default() == "json" {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            let full_name = format!("{}/{}", site_name, stem);
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                if let Ok(meta_json) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    prefixed_tree.insert(full_name, meta_json);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if prefixed_tree.is_empty() {
        return Err(format!("No index found for site: {}", site_name));
    }

    let mut result_json = serde_json::Map::new();
    result_json.insert(
        "file_tree".to_string(),
        serde_json::Value::Object(prefixed_tree),
    );

    serde_json::to_string(&result_json).map_err(|e| e.to_string())
}

pub fn get_processed_file_path_core(output_dir: &str, filename: &str) -> Result<String, String> {
    let base = resolve_path(output_dir);
    let parts: Vec<&str> = filename.splitn(2, '/').collect();
    let (site_dir, file_base) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("", filename)
    };

    for ext in &[
        ".md", ".html", ".htm", ".txt", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
        ".csv", ".xml", ".json", ".rtf", ".odt", ".epub", ".rst", ".yaml", ".yml", ".log", ".tex",
    ] {
        let docs_path = if site_dir.is_empty() {
            base.join("docs").join(format!("{}{}", file_base, ext))
        } else {
            base.join(site_dir)
                .join("docs")
                .join(format!("{}{}", file_base, ext))
        };
        if docs_path.exists() {
            return Ok(docs_path.to_string_lossy().into_owned());
        }
    }

    Err(format!("Processed file not found: {}", filename))
}
