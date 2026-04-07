use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use tauri::{Emitter, Manager};

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

fn project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.ends_with("src-tauri") {
        cwd.parent().unwrap_or(&cwd).to_path_buf()
    } else {
        cwd
    }
}

fn resolve_path(relative: &str) -> PathBuf {
    project_root().join(relative)
}

#[tauri::command]
async fn run_crawler(app: tauri::AppHandle, config_path: String) -> Result<CrawlResult, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<CrawlResult, String>>();

    // Run crawler in a real OS thread so app.emit works immediately
    std::thread::spawn(move || {
        let result = (|| {
            let root = project_root();
            let abs_config = root.join(&config_path);
            let crawler_file = root.join("python").join("crawler.py");

            let mut python_cmd = "python3".to_string();
            let venv_mac = root.join(".venv").join("bin").join("python3");
            let venv_win = root.join(".venv").join("Scripts").join("python.exe");
            if venv_mac.exists() { 
                python_cmd = venv_mac.to_string_lossy().to_string(); 
            } else if venv_win.exists() { 
                python_cmd = venv_win.to_string_lossy().to_string(); 
            }

            let mut child = Command::new(&python_cmd)
                .current_dir(&root)
                .arg(&crawler_file)
                .arg(&abs_config)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env("PYTHONUNBUFFERED", "1")
                .spawn()
                .map_err(|e| format!("Failed to run crawler: {}", e))?;

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
fn update_delay(delay: f64) -> Result<(), String> {
    // Write delay to a temp file that the Python crawler watches
    let delay_file = project_root().join(".crawl_delay");
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
    for ext in &[".md", ".html", ".txt"] {
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
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            run_crawler,
            read_config,
            write_config,
            read_index,
            open_url,
            read_file_content,
            update_delay,
            clear_output
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
