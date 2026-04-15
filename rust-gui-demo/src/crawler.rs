use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub success: bool,
    pub new_files: Vec<String>,
    pub updated_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCrawlResult {
    pub total: usize,
    pub max_depth: u32,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerConfig {
    pub urls: Vec<String>,
    pub file_extensions: Vec<String>,
    pub content_format: String,
    pub output_dir: String,
    pub delay: f64,
    pub max_depth: u32,
    pub min_year: u32,
}

pub struct CrawlerSidecar {
    python_path: String,
    crawler_script: PathBuf,
}

impl CrawlerSidecar {
    pub fn new() -> Result<Self, String> {
        // 查找 Python 解释器
        let python_cmd = if cfg!(windows) {
            "python.exe".to_string()
        } else {
            "python3".to_string()
        };

        // 查找爬虫脚本位置 - 尝试多个位置
        let possible_paths = vec![
            PathBuf::from("../../../python/crawler.py"),
            PathBuf::from("../../python/crawler.py"),
            PathBuf::from("../python/crawler.py"),
        ];

        let mut crawler_script = PathBuf::from("python/crawler.py");
        
        for path in possible_paths {
            if path.exists() {
                crawler_script = path;
                break;
            }
        }

        if !crawler_script.exists() {
            return Err(format!("爬虫脚本不存在: {:?}", crawler_script));
        }

        Ok(CrawlerSidecar {
            python_path: python_cmd,
            crawler_script,
        })
    }

    pub fn generate_config_yaml(config: &CrawlerConfig, output_path: &PathBuf) -> Result<(), String> {
        let yaml_content = format!(
            r#"crawler:
  urls: {}
  file_extensions: {}
  content_format: {}
  output_dir: {}
  delay: {}
  max_depth: {}
  min_year: {}
"#,
            serde_yaml::to_string(&config.urls)
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            serde_yaml::to_string(&config.file_extensions)
                .map(|s| s.trim().to_string())
                .unwrap_or_default(),
            config.content_format,
            config.output_dir,
            config.delay,
            config.max_depth,
            config.min_year,
        );

        std::fs::write(output_path, yaml_content)
            .map_err(|e| format!("写入配置文件失败: {}", e))
    }

    pub fn run_crawl(
        &self,
        config: &CrawlerConfig,
        progress_callback: Arc<Mutex<Box<dyn Fn(&str) + Send>>>,
    ) -> Result<CrawlResult, String> {
        // 生成临时配置文件
        let temp_config = std::env::temp_dir().join("dpcrawler_config.yaml");
        Self::generate_config_yaml(config, &temp_config)?;

        // 启动爬虫进程
        let mut child = Command::new(&self.python_path)
            .arg(self.crawler_script.to_str().unwrap())
            .arg(temp_config.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动爬虫失败: {}", e))?;

        // 监听 stdout
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut result_json = String::new();
            let mut in_result = false;

            for line in reader.lines() {
                if let Ok(line) = line {
                    // 检查是否到达结果部分
                    if line.contains("=== RESULT ===") {
                        in_result = true;
                        continue;
                    }

                    if in_result {
                        result_json.push_str(&line);
                        result_json.push('\n');
                    } else {
                        // 传递日志回调
                        if let Ok(mut cb) = progress_callback.lock() {
                            cb(&line);
                        }
                    }
                }
            }

            // 解析结果 JSON
            if !result_json.is_empty() {
                match serde_json::from_str::<CrawlResult>(&result_json) {
                    Ok(result) => {
                        // 清理临时配置文件
                        let _ = std::fs::remove_file(&temp_config);
                        return Ok(result);
                    }
                    Err(e) => {
                        // 如果解析失败，尝试解析为数据结构
                        return Err(format!("解析爬虫输出失败: {}", e));
                    }
                }
            }
        }

        // 等待进程完成
        let status = child
            .wait()
            .map_err(|e| format!("等待爬虫完成失败: {}", e))?;

        if !status.success() {
            return Err(format!("爬虫执行失败，返回码: {:?}", status.code()));
        }

        Ok(CrawlResult {
            success: false,
            new_files: vec![],
            updated_files: vec![],
            deleted_files: vec![],
            message: "未能解析爬虫结果".to_string(),
        })
    }

    pub fn run_pre_crawl(
        &self,
        config: &CrawlerConfig,
        progress_callback: Arc<Mutex<Box<dyn Fn(&str) + Send>>>,
    ) -> Result<CrawlResult, String> {
        // 生成临时配置文件
        let temp_config = std::env::temp_dir().join("dpcrawler_config.yaml");
        Self::generate_config_yaml(config, &temp_config)?;

        // 启动爬虫进程，带 --pre-crawl 参数
        let mut child = Command::new(&self.python_path)
            .arg(self.crawler_script.to_str().unwrap())
            .arg(temp_config.to_str().unwrap())
            .arg("--pre-crawl")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("启动预爬虫失败: {}", e))?;

        // 监听 stdout
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut result_json = String::new();
            let mut in_result = false;

            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.contains("=== RESULT ===") {
                        in_result = true;
                        continue;
                    }

                    if in_result {
                        result_json.push_str(&line);
                        result_json.push('\n');
                    } else {
                        if let Ok(cb) = progress_callback.lock() {
                            cb(&line);
                        }
                    }
                }
            }

            // 解析结果
            if !result_json.is_empty() {
                match serde_json::from_str::<Value>(&result_json) {
                    Ok(json_val) => {
                        let _ = std::fs::remove_file(&temp_config);
                        
                        // 预爬返回的是 URLs 统计，转换为 CrawlResult
                        let total = json_val.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        let max_depth = json_val.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                        let urls: Vec<String> = json_val
                            .get("urls")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        
                        return Ok(CrawlResult {
                            success: total > 0,
                            new_files: urls,
                            updated_files: vec![],
                            deleted_files: vec![],
                            message: format!("发现 {} 个URL，最大深度 {}", total, max_depth),
                        });
                    }
                    Err(e) => {
                        return Err(format!("解析预爬虫输出失败: {}", e));
                    }
                }
            }
        }

        let status = child
            .wait()
            .map_err(|e| format!("等待预爬虫完成失败: {}", e))?;

        if !status.success() {
            return Err(format!("预爬虫执行失败，返回码: {:?}", status.code()));
        }

        Ok(CrawlResult {
            success: false,
            new_files: vec![],
            updated_files: vec![],
            deleted_files: vec![],
            message: "未能解析预爬虫结果".to_string(),
        })
    }
}
