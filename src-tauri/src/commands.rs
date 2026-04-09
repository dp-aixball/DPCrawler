use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use tauri::{Emitter, Manager};
use std::sync::atomic::Ordering;

use std::path::PathBuf;
use crate::process::{CRAWLER_PID, kill_pid, is_pid_alive};
use crate::fs_utils::{resolve_path, data_dir, dev_project_root, is_dev_mode};

/// Find the crawler executable.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub success: bool,
    pub new_files: Vec<String>,
    pub updated_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlProgress {
    line: String,
    file_name: String,
    status: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreCrawlProgress {
    depth: i32,
    found: i32,
    url: String,
    is_doc: bool,
}

pub fn find_crawler() -> Result<(String, Vec<String>), String> {
    // Dev mode: prefer venv/python so code changes take effect immediately
    if is_dev_mode() {
        let root = dev_project_root();
        let crawler_file = root.join("python").join("crawler.py");
        if crawler_file.exists() {
            let venv_python = if cfg!(windows) {
                root.join(".venv").join("Scripts").join("python.exe")
            } else {
                root.join(".venv").join("bin").join("python3")
            };
            let python_cmd = if venv_python.exists() {
                venv_python.to_string_lossy().to_string()
            } else {
                if cfg!(windows) { "python.exe".to_string() } else { "python3".to_string() }
            };
            return Ok((python_cmd, vec![crawler_file.to_string_lossy().to_string()]));
        }
    }

    // Installed: check next to our own executable (e.g. /usr/bin/crawler or C:\Program Files\...\crawler.exe)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let sidecar_name = if cfg!(windows) { "crawler.exe" } else { "crawler" };
            let sidecar = exe_dir.join(sidecar_name);
            if sidecar.exists() && sidecar.is_file() {
                return Ok((sidecar.to_string_lossy().to_string(), vec![]));
            }
        }
    }

    // Fallback: check src-tauri/binaries/ (pre-built sidecar, e.g. for testing)
    let root = dev_project_root();
    if let Ok(entries) = root.join("src-tauri").join("binaries").read_dir() {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Some(name) = p.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with("crawler-") && p.is_file() {
                    // On Windows, check for .exe
                    if cfg!(windows) && !name_str.ends_with(".exe") {
                        continue;
                    }
                    let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if size > 1000 {  // skip placeholder files
                        return Ok((p.to_string_lossy().to_string(), vec![]));
                    }
                }
            }
        }
    }

    Err("Crawler not found: no sidecar binary and no python/crawler.py".to_string())
}

#[tauri::command]
pub async fn run_crawler(app: tauri::AppHandle, config_path: String) -> Result<CrawlResult, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<CrawlResult, String>>();

    // Run crawler in a real OS thread so app.emit works immediately
    std::thread::spawn(move || {
        let result = (|| {
            let abs_config = resolve_path(&config_path);
            let work_dir = data_dir();

            // Find crawler executable (sidecar or python)
            let (cmd, extra_args) = find_crawler()?;

            let mut args: Vec<String> = extra_args;
            args.push(abs_config.to_string_lossy().to_string());

            let mut cmd_obj = Command::new(&cmd);
            cmd_obj.current_dir(&work_dir)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .env("PYTHONUNBUFFERED", "1");

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd_obj.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }

            let mut child = cmd_obj.spawn()
                .map_err(|e| format!("Failed to run crawler: {} (cmd: {})", e, cmd))?;

            CRAWLER_PID.store(child.id(), Ordering::SeqCst);

            let stdout = child.stdout.take().unwrap();
            let reader = BufReader::new(stdout);
            let app_clone = app.clone();

            // Read stdout in a dedicated thread to avoid blocking the crawler
            let stdout_thread = std::thread::spawn(move || {
                let mut result_json = String::new();
                let mut in_result = false;
                let mut last_url = String::new();

                for line in reader.lines() {
                    let line = line.unwrap_or_default();

                    if line.contains("=== RESULT ===") {
                        in_result = true;
                        continue;
                    }

                    if in_result {
                        result_json = line;
                        continue;
                    }

                    let mut file_name = String::new();
                    let mut status = "info".to_string();
                    let mut current_url = String::new();

                    if line.contains("Crawling:") {
                        if let Some(url_part) = line.split("Crawling:").last() {
                            current_url = url_part.trim().to_string();
                            last_url = current_url.clone();
                        }
                    } else if line.contains("[skip]") {
                        if let Some(url_part) = line.split(": ").last() {
                            current_url = url_part.trim().to_string();
                            last_url = current_url.clone();
                        }
                        status = "skip".to_string();
                    }

                    if line.contains("-> New:") {
                        file_name = line.split("-> New:").last().unwrap_or("").trim().to_string();
                        status = "new".to_string();
                        current_url = last_url.clone();
                    } else if line.contains("-> Updated:") {
                        file_name = line.split("-> Updated:").last().unwrap_or("").trim().to_string();
                        status = "updated".to_string();
                        current_url = last_url.clone();
                    } else if line.contains("-> Unchanged:") {
                        file_name = line.split("-> Unchanged:").last().unwrap_or("").trim().to_string();
                        status = "unchanged".to_string();
                        current_url = last_url.clone();
                    } else if line.contains("-> Error:") {
                        status = "error".to_string();
                    }

                    // Non-blocking emit (ignore errors to avoid blocking stdout read)
                    let _ = app_clone.emit("crawl-progress", CrawlProgress {
                        line: line.clone(),
                        file_name,
                        status,
                        url: current_url,
                    });
                }

                result_json
            });

            let _exit = child.wait().map_err(|e| format!("Wait error: {}", e))?;
            CRAWLER_PID.store(0, Ordering::SeqCst);

            // Get result from stdout reading thread
            let result_json = stdout_thread.join()
                .map_err(|e| format!("Stdout reader thread panicked: {:?}", e.downcast_ref::<&str>().unwrap_or(&"unknown")))?;

            if result_json.is_empty() {
                return Err("No result from crawler".to_string());
            }

            serde_json::from_str::<CrawlResult>(&result_json)
                .map_err(|e| format!("Parse error: {} (raw: {})", e, result_json))
        })();

        let _ = tx.send(result);
    });

    // Await result from the thread without blocking the main thread
    tokio::task::spawn_blocking(move || {
        rx.recv().map_err(|e| format!("Channel error: {}", e))?
    }).await.map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn run_pre_crawl(app: tauri::AppHandle, config_path: String) -> Result<String, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

    std::thread::spawn(move || {
        let result = (|| {
            let abs_config = resolve_path(&config_path);
            let work_dir = data_dir();
            let (cmd, extra_args) = find_crawler()?;

            let mut args: Vec<String> = extra_args;
            args.push(abs_config.to_string_lossy().to_string());
            args.push("--pre-crawl".to_string());

            let mut cmd_obj = Command::new(&cmd);
            cmd_obj.current_dir(&work_dir)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .env("PYTHONUNBUFFERED", "1");

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd_obj.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }

            let mut child = cmd_obj.spawn()
                .map_err(|e| format!("Failed to run pre-crawl: {} (cmd: {})", e, cmd))?;

            CRAWLER_PID.store(child.id(), Ordering::SeqCst);

            let stdout = child.stdout.take().unwrap();
            let reader = BufReader::new(stdout);
            let mut result_json = String::new();
            let mut in_result = false;

            for line in reader.lines() {
                let line = line.unwrap_or_default();

                if line.contains("=== RESULT ===") {
                    in_result = true;
                    continue;
                }

                if in_result {
                    result_json = line;
                    continue;
                }

                // Parse [pre-crawl] lines for structured progress
                if line.contains("[pre-crawl]") {
                    let mut depth = 0i32;
                    let mut found = 0i32;
                    let mut url = String::new();
                    let is_doc = line.contains("(doc)");

                    if let Some(d) = line.split("depth=").nth(1) {
                        depth = d.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
                    }
                    if let Some(f) = line.split("found=").nth(1) {
                        found = f.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
                    }
                    if let Some(u) = line.split_whitespace().last() {
                        if u.starts_with("http") {
                            url = u.to_string();
                        }
                    }

                    let _ = app.emit("pre-crawl-progress", PreCrawlProgress {
                        depth, found, url, is_doc,
                    });
                }

                // Forward ALL lines as log via crawl-progress event
                let _ = app.emit("crawl-progress", CrawlProgress {
                    line: line.clone(),
                    file_name: String::new(),
                    status: "info".to_string(),
                    url: String::new(),
                });
            }

            CRAWLER_PID.store(0, Ordering::SeqCst);
            let _exit = child.wait().map_err(|e| format!("Wait error: {}", e))?;
            // SIGTERM (code 143) is expected when user stops - still parse result if available
            if !_exit.success() && result_json.is_empty() {
                return Err("Pre-crawl process failed".to_string());
            }
            if result_json.is_empty() {
                return Err("No result from pre-crawl".to_string());
            }
            Ok(result_json)
        })();
        let _ = tx.send(result);
    });

    tokio::task::spawn_blocking(move || {
        rx.recv().map_err(|e| format!("Channel error: {}", e))?
    }).await.map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub fn stop_crawler() -> Result<String, String> {
    let pid = CRAWLER_PID.load(Ordering::SeqCst);
    if pid == 0 {
        return Ok("No running process".to_string());
    }
    
    // Send SIGTERM first for graceful shutdown
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }
        // Wait up to 5 seconds for graceful exit, then SIGKILL
        let handle_pid = pid;
        std::thread::spawn(move || {
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if !is_pid_alive(handle_pid) {
                    return;
                }
            }
            // Force kill after 5 seconds
            kill_pid(handle_pid);
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T") // Tree kill
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn();
    }
    
    // Don't clear PID here - let the reader thread clear it when process actually exits
    Ok(format!("Stopping process {}", pid))
}

#[tauri::command]
pub fn save_pre_crawl_result(config_path: String, data: String) -> Result<(), String> {
    // Extract first URL from config to use in filename
    let mut first_url = String::from("unknown");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('-') {
                first_url = trimmed.trim_start_matches('-').trim().to_string();
                break;
            }
        }
    }
    // Sanitize URL for filename
    let safe_name: String = first_url
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    
    let filename = format!(".pre_crawl_{}.json", safe_name);
    let path = data_dir().join(&filename);
    std::fs::write(&path, &data)
        .map_err(|e| format!("Failed to save pre-crawl result: {}", e))
}

#[tauri::command]
pub fn load_pre_crawl_result(config_path: String) -> Result<String, String> {
    // Extract first URL from config to find matching file
    let mut first_url = String::from("unknown");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('-') {
                first_url = trimmed.trim_start_matches('-').trim().to_string();
                break;
            }
        }
    }
    let safe_name: String = first_url
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    
    let filename = format!(".pre_crawl_{}.json", safe_name);
    let path = data_dir().join(&filename);
    std::fs::read_to_string(&path)
        .map_err(|e| format!("No pre-crawl data: {}", e))
}

#[tauri::command]
pub fn update_delay(delay: f64) -> Result<(), String> {
    let delay_file = data_dir().join(".crawl_delay");
    std::fs::write(&delay_file, format!("{}", delay))
        .map_err(|e| format!("Failed to update delay: {}", e))
}

#[tauri::command]
pub fn clear_output(output_dir: String, subdirs: Vec<String>) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let mut removed = 0u32;
    for subdir in &subdirs {
        let path = base.join(subdir);
        if path.exists() && path.is_dir() {
            let docs = path.join("docs");
            let meta = path.join("meta");
            let index = path.join("index.json");
            
            if docs.exists() { let _ = std::fs::remove_dir_all(&docs); }
            if meta.exists() { let _ = std::fs::remove_dir_all(&meta); }
            if index.exists() { let _ = std::fs::remove_file(&index); }
            
            removed += 1;
        }
    }
    Ok(format!("Cleared contents of {} directories", removed))
}

#[tauri::command]
pub fn delete_site(output_dir: String, site_name: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let path = base.join(&site_name);
    if path.exists() && path.is_dir() {
        std::fs::remove_dir_all(&path)
            .map_err(|e| format!("Failed to delete {}: {}", site_name, e))?;
        Ok(format!("Deleted {}", site_name))
    } else {
        Err("Site directory not found".to_string())
    }
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let target = if url.starts_with("http://") || url.starts_with("https://") {
        url.clone()
    } else {
        let path = resolve_path(url.strip_prefix("./").unwrap_or(&url));
        path.to_string_lossy().to_string()
    };

    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(target_os = "windows")]
    let opener = "cmd";

    #[cfg(target_os = "windows")]
    {
        Command::new(opener)
            .args(&["/C", "start", "", &target])
            .spawn()
            .map_err(|e| format!("Failed to open {}: {}", target, e))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new(opener)
            .arg(&target)
            .spawn()
            .map_err(|e| format!("Failed to open {}: {}", target, e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn read_config(config_path: String) -> Result<String, String> {
    std::fs::read_to_string(resolve_path(&config_path))
        .map_err(|e| format!("Failed to read config: {}", e))
}

#[tauri::command]
pub fn write_config(config_path: String, content: String) -> Result<(), String> {
    std::fs::write(resolve_path(&config_path), &content)
        .map_err(|e| format!("Failed to write config: {}", e))
}

#[tauri::command]
pub fn read_file_content(output_dir: String, filename: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    // filename format: "site_name/page_name", content is in "site_name/docs/page_name.ext"
    let parts: Vec<&str> = filename.splitn(2, '/').collect();
    let (site_dir, file_base) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("", filename.as_str())
    };
    
    for ext in &[".md", ".html", ".htm", ".txt", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".csv", ".xml", ".json", ".rtf", ".odt", ".epub", ".rst", ".yaml", ".yml", ".log", ".tex"] {
        // New structure: site_name/docs/file.ext
        let docs_path = if site_dir.is_empty() {
            base.join("docs").join(format!("{}{}", file_base, ext))
        } else {
            base.join(site_dir).join("docs").join(format!("{}{}", file_base, ext))
        };
        if docs_path.exists() {
            return std::fs::read_to_string(&docs_path)
                .map_err(|e| format!("Failed to read file: {}", e));
        }
        // Legacy flat structure: site_name/file.ext
        let legacy_path = base.join(format!("{}{}", filename, ext));
        if legacy_path.exists() {
            return std::fs::read_to_string(&legacy_path)
                .map_err(|e| format!("Failed to read file: {}", e));
        }
    }
    Err(format!("File not found: {}", filename))
}

#[tauri::command]
pub fn list_crawled_sites(output_dir: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let mut sites: Vec<serde_json::Value> = Vec::new();
    if base.exists() {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let index_path = path.join("index.json");
                    let mut file_count = 0u32;
                    let mut last_updated = String::new();
                    if index_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&index_path) {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(tree) = data.get("file_tree").and_then(|t| t.as_object()) {
                                    file_count = tree.len() as u32;
                                }
                                if let Some(ts) = data.get("last_updated").and_then(|v| v.as_str()) {
                                    last_updated = ts.to_string();
                                }
                            }
                        }
                    }
                    // Fallback: if index.json has no entries, count actual files in docs/
                    if file_count == 0 {
                        let docs_path = path.join("docs");
                        if docs_path.is_dir() {
                            if let Ok(files) = std::fs::read_dir(&docs_path) {
                                file_count = files.flatten().filter(|f| f.path().is_file()).count() as u32;
                            }
                        }
                    }
                    // Show sites that have content or a saved config
                    let has_config = path.join("crawl_config.json").exists();
                    if file_count > 0 || index_path.exists() || has_config {
                        sites.push(serde_json::json!({
                            "name": name,
                            "file_count": file_count,
                            "last_updated": last_updated
                        }));
                    }
                }
            }
        }
    }
    // Sort by name
    sites.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
    Ok(serde_json::to_string(&sites).unwrap_or_else(|_| "[]".to_string()))
}

#[tauri::command]
pub fn read_site_config(output_dir: String, site_name: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let config_path = base.join(&site_name).join("crawl_config.json");
    if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read site config: {}", e))
    } else {
        Ok("{}".to_string())
    }
}

#[tauri::command]
pub fn read_site_index(output_dir: String, site_name: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let site_dir = base.join(&site_name);
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
    }

    // Fallback: if index.json has no entries, scan docs/ directory for actual files
    if prefixed_tree.is_empty() {
        let docs_dir = site_dir.join("docs");
        if docs_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&docs_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                            // Strip the extension to get the display name
                            let display = fname.rsplit_once('.').map(|(n, _)| n).unwrap_or(fname);
                            let full_name = format!("{}/{}", site_name, display);
                            prefixed_tree.insert(full_name, serde_json::json!({
                                "source_url": ""
                            }));
                        }
                    }
                }
            }
        }
    }

    let result = serde_json::json!({
        "file_tree": prefixed_tree,
        "total_files": prefixed_tree.len()
    });
    Ok(result.to_string())
}

#[tauri::command]
pub fn read_index(output_dir: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let mut merged_tree = serde_json::Map::new();
    
    // Scan subdirectories for index.json files
    if base.exists() {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let subdir_name = entry.file_name().to_string_lossy().to_string();
                    let index_path = path.join("index.json");
                    if index_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&index_path) {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                                if let Some(tree) = data.get("file_tree").and_then(|t| t.as_object()) {
                                    for (name, meta) in tree {
                                        // Key includes subdir: "www.example.com/page_name"
                                        let full_name = format!("{}/{}", subdir_name, name);
                                        merged_tree.insert(full_name, meta.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Also check for legacy root-level index.json
    let root_index = base.join("index.json");
    if root_index.exists() {
        if let Ok(content) = std::fs::read_to_string(&root_index) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(tree) = data.get("file_tree").and_then(|t| t.as_object()) {
                    for (name, meta) in tree {
                        if !merged_tree.contains_key(name) {
                            merged_tree.insert(name.clone(), meta.clone());
                        }
                    }
                }
            }
        }
    }
    
    let result = serde_json::json!({
        "file_tree": merged_tree,
        "total_files": merged_tree.len()
    });
    Ok(result.to_string())
}

#[tauri::command]
pub fn force_quit(app: tauri::AppHandle) {
    let pid = CRAWLER_PID.load(Ordering::SeqCst);
    if pid > 0 {
        kill_pid(pid);
        CRAWLER_PID.store(0, Ordering::SeqCst);
    }
    app.exit(0);
}
