use crate::theme::{ThemeMode, ThemePalette};
use crate::crawler::{CrawlerSidecar, CrawlerConfig};

#[derive(Clone, Copy, PartialEq)]
pub enum RunMode {
    Idle,
    PreCrawl,
    Crawling,
}

pub struct DPCrawlerDemo {
    pub urls: String,
    pub extensions: Vec<(String, bool)>,
    pub output_dir: String,
    pub content_format: String,
    pub theme_mode: ThemeMode,
    pub delay: String,
    pub max_depth: String,
    pub min_year: String,
    pub sites: Vec<(String, usize)>,
    pub active_site: Option<String>,
    pub active_tab: usize,
    pub logs: String,
    pub files: Vec<(String, String, String)>,
    pub file_list_width: f32,
    pub drag_start_file_list_width: Option<f32>,
    pub selected_file: String,
    pub preview: String,
    pub is_running: bool,
    pub run_mode: RunMode,
    pub task_progress: f32,
    pub progress: f32,
    pub progress_text: String,
    pub status_text: String,
    pub status_active: bool,
    pub total_count: usize,
    pub new_count: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub error_count: usize,
    pub crawler: Option<CrawlerSidecar>,
}

impl Default for DPCrawlerDemo {
    fn default() -> Self {
        Self {
            urls: "https://www.example.gov.cn".to_string(),
            extensions: vec![
                (".pdf".to_string(), true),
                (".doc".to_string(), true),
                (".docx".to_string(), true),
                (".xls".to_string(), true),
                (".xlsx".to_string(), true),
                (".ppt".to_string(), true),
                (".pptx".to_string(), true),
                (".csv".to_string(), true),
            ],
            output_dir: "./output".to_string(),
            content_format: "markdown".to_string(),
            theme_mode: ThemeMode::Auto,
            delay: "500ms".to_string(),
            max_depth: "1 层".to_string(),
            min_year: "2025 年".to_string(),
            sites: vec![
                ("www.example.gov.cn".to_string(), 128),
                ("hrss.sz.gov.cn".to_string(), 508),
            ],
            active_site: None,
            active_tab: 0,
            logs:
                "[20:16:00] DPCrawler 已就绪\n[20:16:01] 配置已加载\n[20:16:02] 等待开始爬取...\n"
                    .to_string(),
            files: vec![
                (
                    "www.example.gov.cn/政策通知_2025".to_string(),
                    "new".to_string(),
                    "https://example.gov.cn/notice1".to_string(),
                ),
                (
                    "www.example.gov.cn/考试安排".to_string(),
                    "updated".to_string(),
                    "https://example.gov.cn/exam".to_string(),
                ),
                (
                    "hrss.sz.gov.cn/招生简章".to_string(),
                    "unchanged".to_string(),
                    "https://example.edu.cn/admission".to_string(),
                ),
            ],
            file_list_width: 320.0,
            drag_start_file_list_width: None,
            selected_file: String::new(),
            preview: "单击左侧文件即可预览内容".to_string(),
            is_running: false,
            run_mode: RunMode::Idle,
            task_progress: 0.0,
            progress: 0.0,
            progress_text: "准备就绪".to_string(),
            status_text: "就绪".to_string(),
            status_active: false,
            total_count: 0,
            new_count: 0,
            updated_count: 0,
            unchanged_count: 0,
            error_count: 0,
            crawler: CrawlerSidecar::new().ok(),
        }
    }
}

impl DPCrawlerDemo {
    const SIDEBAR_SCROLLBAR_GUTTER: f32 = 18.0;

    pub fn control_width(ui: &egui::Ui) -> f32 {
        (ui.available_width() - 6.0).max(0.0)
    }

    pub fn append_log(&mut self, line: impl AsRef<str>) {
        self.logs.push_str(line.as_ref());
        self.logs.push('\n');
    }

    pub fn visible_files(&self) -> Vec<(String, String, String)> {
        let mut files: Vec<(String, String, String)> = self
            .files
            .iter()
            .filter(|(name, _, _)| {
                self.active_site
                    .as_ref()
                    .map(|site| name.starts_with(&format!("{site}/")))
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        files.sort_by_key(|(_, status, _)| match status.as_str() {
            "new" => 0,
            "updated" => 1,
            "unchanged" => 2,
            "error" => 3,
            _ => 9,
        });
        files
    }

    pub fn apply_active_site_to_form(&mut self, site_name: &str) {
        self.active_site = Some(site_name.to_string());
        self.urls = format!("https://{site_name}");
        self.delay = if site_name.contains("hrss") {
            "300ms".to_string()
        } else {
            "500ms".to_string()
        };
        self.max_depth = if site_name.contains("hrss") {
            "2 层".to_string()
        } else {
            "1 层".to_string()
        };
        self.min_year = if site_name.contains("hrss") {
            "2024 年".to_string()
        } else {
            "2025 年".to_string()
        };
        self.progress = 1.0;
        let count = self
            .sites
            .iter()
            .find(|(name, _)| name == site_name)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        self.progress_text = format!("{site_name}: {count} 个文件");
        self.total_count = count;
        self.new_count = 0;
        self.updated_count = 0;
        self.unchanged_count = count;
        self.error_count = 0;
    }

    pub fn glass_frame(&self, palette: ThemePalette) -> egui::Frame {
        use egui::{Color32, CornerRadius, Margin, Stroke};
        egui::Frame::new()
            .fill(palette.glass)
            .stroke(Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 255, 255, 51)))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
    }

    fn parse_delay(&self) -> f64 {
        let s = self.delay.trim().to_lowercase();
        if let Some(ms_pos) = s.find("ms") {
            if let Ok(val) = s[..ms_pos].trim().parse::<f64>() {
                return val / 1000.0;
            }
        }
        if let Ok(val) = s.parse::<f64>() {
            return val;
        }
        0.5
    }

    fn parse_max_depth(&self) -> u32 {
        let s = self.max_depth.trim();
        if let Some(num_str) = s.split_whitespace().next() {
            if let Ok(val) = num_str.parse::<u32>() {
                return val;
            }
        }
        1
    }

    fn parse_min_year(&self) -> u32 {
        let s = self.min_year.trim();
        if let Some(num_str) = s.split_whitespace().next() {
            if let Ok(val) = num_str.parse::<u32>() {
                return val;
            }
        }
        2020
    }

    fn parse_extensions(&self) -> Vec<String> {
        self.extensions
            .iter()
            .filter(|(_, enabled)| *enabled)
            .map(|(ext, _)| ext.clone())
            .collect()
    }

    pub fn begin_pre_crawl(&mut self) {
        if self.crawler.is_none() {
            self.append_log("[ERROR] 爬虫不可用，请检查 Python 环境");
            return;
        }

        self.is_running = true;
        self.run_mode = RunMode::PreCrawl;
        self.task_progress = 0.0;
        self.status_active = true;
        self.status_text = "预爬中...".to_string();
        self.progress = 0.0;
        self.progress_text = "正在发现 URL...".to_string();
        self.logs.clear();
        self.append_log("[*] 全站预爬开始...");
        self.append_log(format!("[*] 目标URL: {}", self.urls));

        let config = CrawlerConfig {
            urls: vec![self.urls.clone()],
            file_extensions: self.parse_extensions(),
            content_format: self.content_format.clone(),
            output_dir: self.output_dir.clone(),
            delay: self.parse_delay(),
            max_depth: self.parse_max_depth(),
            min_year: self.parse_min_year(),
        };

        self.append_log(format!("[*] 配置: {} 文件类型, max_depth={}, delay={}s",
            config.file_extensions.join(","),
            config.max_depth,
            config.delay
        ));
    }

    pub fn begin_crawl(&mut self) {
        if self.crawler.is_none() {
            self.append_log("[ERROR] 爬虫不可用，请检查 Python 环境");
            return;
        }

        self.is_running = true;
        self.run_mode = RunMode::Crawling;
        self.task_progress = 0.0;
        self.status_active = true;
        self.status_text = "爬取中...".to_string();
        self.progress = 0.0;
        self.progress_text = "0 / ?".to_string();
        self.logs.clear();
        self.new_count = 0;
        self.updated_count = 0;
        self.unchanged_count = 0;
        self.error_count = 0;
        self.total_count = 0;
        self.append_log("[*] 开始爬取...");
        self.append_log(format!("[*] 目标URL: {}", self.urls));

        let config = CrawlerConfig {
            urls: vec![self.urls.clone()],
            file_extensions: self.parse_extensions(),
            content_format: self.content_format.clone(),
            output_dir: self.output_dir.clone(),
            delay: self.parse_delay(),
            max_depth: self.parse_max_depth(),
            min_year: self.parse_min_year(),
        };

        self.append_log(format!("[*] 配置: {} 文件类型, max_depth={}, delay={}s",
            config.file_extensions.join(","),
            config.max_depth,
            config.delay
        ));
        self.append_log("[*] 爬虫已启动，等待结果...");
    }

    pub fn stop_current_task(&mut self) {
        self.is_running = false;
        self.run_mode = RunMode::Idle;
        self.task_progress = 0.0;
        self.status_active = false;
        self.status_text = "已停止".to_string();
        self.append_log("[20:16:50] 正在停止...");
        self.append_log("[20:16:51] 已停止");
    }

    pub fn clear_results(&mut self) {
        if let Some(site) = self.active_site.clone() {
            self.files
                .retain(|(name, _, _)| !name.starts_with(&format!("{site}/")));
            self.sites.retain(|(name, _)| name != &site);
            self.append_log(format!("[20:16:20] 已清空站点结果: {site}"));
            self.active_site = None;
        } else {
            self.files.clear();
            self.sites.clear();
            self.append_log("[20:16:20] 已清空全部结果");
        }
        self.selected_file.clear();
        self.preview = "单击左侧文件即可预览内容".to_string();
        self.new_count = 0;
        self.updated_count = 0;
        self.unchanged_count = 0;
        self.error_count = 0;
        self.total_count = 0;
        self.progress = 0.0;
        self.progress_text = "准备就绪".to_string();
        self.status_text = "就绪".to_string();
    }
}

impl eframe::App for DPCrawlerDemo {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let palette = ThemePalette::get_palette(self.theme_mode);

        use egui::{CornerRadius, Margin, Stroke};
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .width_range(240.0..=400.0)
            .frame(
                egui::Frame::new()
                    .fill(palette.surface)
                    .inner_margin(Margin::same(0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let content_width =
                            (ui.available_width() - Self::SIDEBAR_SCROLLBAR_GUTTER).max(0.0);
                        ui.allocate_ui_with_layout(
                            egui::Vec2::new(content_width, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::Frame::new()
                                    .inner_margin(Margin::same(20))
                                    .show(ui, |ui| self.render_sidebar(ui, palette));
                            },
                        );
                    });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.bg).inner_margin(Margin::same(24)))
            .show(ctx, |ui| {
                self.render_topbar(ui, palette);
                ui.add_space(20.0);
                self.render_progress(ui, palette);
                ui.add_space(16.0);
                self.render_tabs(ui, palette);

                match self.active_tab {
                    0 => self.render_logs(ui, palette),
                    1 => self.render_files(ui, palette),
                    2 => self.render_results(ui, palette),
                    _ => {}
                }
            });
    }
}
