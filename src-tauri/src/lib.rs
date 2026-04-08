use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{Emitter, Manager};

/// Disable WebKit cache & persistence to prevent IPC corruption
fn disable_webkit_cache() {
    // Kill stale process via PID file
    let pid_file = dirs::cache_dir()
        .map(|d| d.join("com.dpcrawler.app").join(".pid"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/dpcrawler.pid"));
    
    if pid_file.exists() {
        if let Ok(mut f) = std::fs::File::open(&pid_file) {
            let mut buf = String::new();
            if f.read_to_string(&mut buf).is_ok() {
                if let Ok(pid) = buf.trim().parse::<i32>() {
                    // Check if process is still alive
                    let alive = unsafe { libc::kill(pid, 0) == 0 };
                    if alive {
                        unsafe { libc::kill(pid, libc::SIGKILL); }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
    }

    // Nuke and recreate WebKit data directories
    let dirs_to_nuke = [
        dirs::data_local_dir().map(|d| d.join("com.dpcrawler.app")),
        dirs::cache_dir().map(|d| d.join("com.dpcrawler.app")),
    ];
    for path in dirs_to_nuke.into_iter().flatten() {
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
        let _ = std::fs::create_dir_all(&path);
    }
    
    // Write our PID so next run can clean us up
    let _ = std::fs::create_dir_all(pid_file.parent().unwrap());
    if let Ok(mut f) = std::fs::File::create(&pid_file) {
        let _ = f.write_all(std::process::id().to_string().as_bytes());
    }
}

/// Global child process PID for stop support
static CRAWLER_PID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub success: bool,
    pub new_files: Vec<String>,
    pub updated_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
struct CrawlProgress {
    line: String,
    file_name: String,
    status: String,
    url: String,
}

#[derive(Debug, Clone, Serialize)]
struct PreCrawlProgress {
    depth: i32,
    found: i32,
    url: String,
    is_doc: bool,
}

/// Detect if running from a development project directory
/// (has python/crawler.py), or installed system-wide.
fn is_dev_mode() -> bool {
    let dev_root = dev_project_root();
    dev_root.join("python").join("crawler.py").exists()
}

/// Development project root (CWD-based, for `cargo tauri dev`)
fn dev_project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

/// Data directory for config/output (works both dev and installed)
/// Dev: project root; Installed: ~/.config/dpcrawler/
fn data_dir() -> PathBuf {
    if is_dev_mode() {
        dev_project_root()
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let dir = PathBuf::from(home).join(".config").join("dpcrawler");
        std::fs::create_dir_all(&dir).ok();
        dir
    }
}

/// Resolve a relative path against the data directory
fn resolve_path(relative: &str) -> PathBuf {
    data_dir().join(relative)
}

/// Find the crawler executable.
/// Installed: next to our own binary (e.g. /usr/bin/crawler)
/// Dev: use venv python or system python (skip pre-built sidecar)
fn find_crawler() -> Result<(String, Vec<String>), String> {
    // Dev mode: prefer venv/python so code changes take effect immediately
    if is_dev_mode() {
        let root = dev_project_root();
        let crawler_file = root.join("python").join("crawler.py");
        if crawler_file.exists() {
            let venv_python = root.join(".venv").join("bin").join("python3");
            let python_cmd = if venv_python.exists() {
                venv_python.to_string_lossy().to_string()
            } else {
                "python3".to_string()
            };
            return Ok((python_cmd, vec![crawler_file.to_string_lossy().to_string()]));
        }
    }

    // Installed: check next to our own executable (e.g. /usr/bin/crawler)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let sidecar = exe_dir.join("crawler");
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
                if name.to_string_lossy().starts_with("crawler-") && p.is_file() {
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
async fn run_crawler(app: tauri::AppHandle, config_path: String) -> Result<CrawlResult, String> {
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

            let mut child = Command::new(&cmd)
                .current_dir(&work_dir)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("PYTHONUNBUFFERED", "1")
                .spawn()
                .map_err(|e| format!("Failed to run crawler: {} (cmd: {})", e, cmd))?;

            CRAWLER_PID.store(child.id(), Ordering::SeqCst);

            let stdout = child.stdout.take().unwrap();
            let reader = BufReader::new(stdout);
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

                let _ = app.emit("crawl-progress", CrawlProgress {
                    line: line.clone(),
                    file_name,
                    status,
                    url: current_url,
                });
            }

            CRAWLER_PID.store(0, Ordering::SeqCst);
            let exit = child.wait().map_err(|e| format!("Wait error: {}", e))?;

            if !exit.success() {
                return Err("Crawler process failed".to_string());
            }

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
async fn run_pre_crawl(app: tauri::AppHandle, config_path: String) -> Result<String, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();

    std::thread::spawn(move || {
        let result = (|| {
            let abs_config = resolve_path(&config_path);
            let work_dir = data_dir();
            let (cmd, extra_args) = find_crawler()?;

            let mut args: Vec<String> = extra_args;
            args.push(abs_config.to_string_lossy().to_string());
            args.push("--pre-crawl".to_string());

            let mut child = Command::new(&cmd)
                .current_dir(&work_dir)
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("PYTHONUNBUFFERED", "1")
                .spawn()
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
            let exit = child.wait().map_err(|e| format!("Wait error: {}", e))?;
            // SIGTERM (code 143) is expected when user stops - still parse result if available
            if !exit.success() && result_json.is_empty() {
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
fn stop_crawler() -> Result<String, String> {
    let pid = CRAWLER_PID.load(Ordering::SeqCst);
    if pid == 0 {
        return Ok("No running process".to_string());
    }
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    CRAWLER_PID.store(0, Ordering::SeqCst);
    Ok(format!("Stopped process {}", pid))
}

#[tauri::command]
fn save_pre_crawl_result(data: String) -> Result<(), String> {
    let path = data_dir().join(".pre_crawl_result.json");
    std::fs::write(&path, &data)
        .map_err(|e| format!("Failed to save pre-crawl result: {}", e))
}

#[tauri::command]
fn load_pre_crawl_result() -> Result<String, String> {
    let path = data_dir().join(".pre_crawl_result.json");
    std::fs::read_to_string(&path)
        .map_err(|e| format!("No pre-crawl data: {}", e))
}

#[tauri::command]
fn update_delay(delay: f64) -> Result<(), String> {
    let delay_file = data_dir().join(".crawl_delay");
    std::fs::write(&delay_file, format!("{}", delay))
        .map_err(|e| format!("Failed to update delay: {}", e))
}

#[tauri::command]
fn clear_output(output_dir: String, subdirs: Vec<String>) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    let mut removed = 0u32;
    for subdir in &subdirs {
        let path = base.join(subdir);
        if path.exists() && path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("Failed to remove {}: {}", subdir, e))?;
            removed += 1;
        }
    }
    Ok(format!("Cleared {} directories", removed))
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
fn read_config(config_path: String) -> Result<String, String> {
    std::fs::read_to_string(resolve_path(&config_path))
        .map_err(|e| format!("Failed to read config: {}", e))
}

#[tauri::command]
fn write_config(config_path: String, content: String) -> Result<(), String> {
    std::fs::write(resolve_path(&config_path), &content)
        .map_err(|e| format!("Failed to write config: {}", e))
}

#[tauri::command]
fn read_file_content(output_dir: String, filename: String) -> Result<String, String> {
    let base = resolve_path(&output_dir);
    // filename may contain subdir like "www.example.com/page_name"
    for ext in &[".md", ".html", ".htm", ".txt", ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".csv", ".xml", ".json", ".rtf", ".odt", ".epub", ".rst", ".yaml", ".yml", ".log", ".tex"] {
        let path = base.join(format!("{}{}", filename, ext));
        if path.exists() {
            return std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file: {}", e));
        }
    }
    Err(format!("File not found: {}", filename))
}

#[tauri::command]
fn read_index(output_dir: String) -> Result<String, String> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    disable_webkit_cache();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            run_crawler,
            run_pre_crawl,
            stop_crawler,
            save_pre_crawl_result,
            load_pre_crawl_result,
            read_config,
            write_config,
            read_index,
            open_url,
            read_file_content,
            update_delay,
            clear_output
        ])
        .setup(|app| {
            // Ensure app exits cleanly when main window closes
            let window = app.get_webview_window("main").unwrap();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api: _, .. } = event {
                    // Kill any running crawler process
                    let pid = CRAWLER_PID.load(Ordering::SeqCst);
                    if pid > 0 {
                        unsafe { libc::kill(pid as i32, libc::SIGTERM); }
                        CRAWLER_PID.store(0, Ordering::SeqCst);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
