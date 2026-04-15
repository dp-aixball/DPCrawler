use egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Vec2};
use crate::app::DPCrawlerDemo;
use crate::theme::{ThemeMode, ThemePalette};

impl DPCrawlerDemo {
    fn section_title(&self, ui: &mut egui::Ui, text: &str, palette: ThemePalette) {
        ui.label(
            RichText::new(text)
                .size(11.0)
                .strong()
                .color(palette.primary)
                .family(egui::FontFamily::Proportional),
        );
        ui.add_space(2.0);
    }

    fn field_label(ui: &mut egui::Ui, text: &str, palette: ThemePalette) {
        ui.label(RichText::new(text).size(12.0).color(palette.muted));
    }

    fn styled_input(ui: &mut egui::Ui, text: &mut String, hint: &str, palette: ThemePalette) {
        ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .desired_width(Self::control_width(ui))
                .background_color(palette.input_bg),
        );
    }

    fn action_button(
        ui: &mut egui::Ui,
        label: &str,
        fill: Color32,
        stroke: Stroke,
        text_color: Color32,
        size: Vec2,
    ) -> egui::Response {
        ui.add_sized(
            size,
            egui::Button::new(RichText::new(label).size(13.0).color(text_color))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(8)),
        )
    }

    pub fn select_popup_string(
        ui: &mut egui::Ui,
        id_source: &str,
        current: &mut String,
        options: &[(&str, &str)],
        palette: ThemePalette,
        width: f32,
    ) -> egui::Response {
        let selected_label = options
            .iter()
            .find_map(|(value, label)| (*value == current.as_str()).then_some(*label))
            .unwrap_or(current.as_str());
        let desired_size = Vec2::new(width, 30.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        let visuals = ui.style().interact(&response);
        let fill = if response.hovered() {
            palette.input_focus
        } else {
            palette.input_bg
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            fill,
            Stroke::new(1.0, palette.border),
            egui::epaint::StrokeKind::Inside,
        );

        let text_rect = egui::Rect::from_min_max(
            rect.min + Vec2::new(10.0, 0.0),
            egui::pos2(rect.max.x - 26.0, rect.max.y),
        );
        let arrow_rect = egui::Rect::from_min_max(
            egui::pos2(rect.max.x - 22.0, rect.min.y),
            egui::pos2(rect.max.x - 8.0, rect.max.y),
        );
        ui.painter().text(
            text_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            selected_label,
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            "▾",
            egui::FontId::proportional(12.0),
            palette.muted,
        );
        let popup_id = ui.make_persistent_id(id_source);

        if response.clicked() {
            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
        }

        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::popup::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(width);
                for (value, label) in options {
                    let is_selected = current.as_str() == *value;
                    if ui
                        .selectable_label(
                            is_selected,
                            RichText::new(*label).size(12.0).color(if is_selected {
                                palette.primary
                            } else {
                                palette.text
                            }),
                        )
                        .clicked()
                    {
                        *current = (*value).to_string();
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                }
            },
        );

        response
    }

    pub fn select_popup_theme(ui: &mut egui::Ui, theme_mode: &mut ThemeMode, palette: ThemePalette) {
        let label = match theme_mode {
            ThemeMode::Auto => "自动",
            ThemeMode::Dark => "深色",
            ThemeMode::Light => "浅色",
        };
        let width = Self::control_width(ui);
        let desired_size = Vec2::new(width, 30.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        let visuals = ui.style().interact(&response);
        let fill = if response.hovered() {
            palette.input_focus
        } else {
            palette.input_bg
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(8),
            fill,
            Stroke::new(1.0, palette.border),
            egui::epaint::StrokeKind::Inside,
        );
        let text_rect = egui::Rect::from_min_max(
            rect.min + Vec2::new(10.0, 0.0),
            egui::pos2(rect.max.x - 26.0, rect.max.y),
        );
        let arrow_rect = egui::Rect::from_min_max(
            egui::pos2(rect.max.x - 22.0, rect.min.y),
            egui::pos2(rect.max.x - 8.0, rect.max.y),
        );
        ui.painter().text(
            text_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            visuals.text_color(),
        );
        ui.painter().text(
            arrow_rect.center(),
            egui::Align2::CENTER_CENTER,
            "▾",
            egui::FontId::proportional(12.0),
            palette.muted,
        );
        let popup_id = ui.make_persistent_id("theme_popup");

        if response.clicked() {
            ui.memory_mut(|mem| mem.toggle_popup(popup_id));
        }

        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::popup::PopupCloseBehavior::CloseOnClick,
            |ui| {
                ui.set_min_width(width);
                for (mode, item_label) in [
                    (ThemeMode::Auto, "自动"),
                    (ThemeMode::Dark, "深色"),
                    (ThemeMode::Light, "浅色"),
                ] {
                    let is_selected = *theme_mode == mode;
                    if ui
                        .selectable_label(
                            is_selected,
                            RichText::new(item_label).size(12.0).color(if is_selected {
                                palette.primary
                            } else {
                                palette.text
                            }),
                        )
                        .clicked()
                    {
                        *theme_mode = mode;
                        ui.memory_mut(|mem| mem.close_popup());
                    }
                }
            },
        );
    }

    pub fn render_sidebar(&mut self, ui: &mut egui::Ui, palette: ThemePalette) {
        let lock_config = self.is_running;
        ui.add_space(4.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("🕷").size(28.0).color(palette.primary));
            ui.label(RichText::new("DPCrawler").size(15.0).strong());
            ui.label(
                RichText::new("RAG 知识爬虫")
                    .size(11.0)
                    .color(palette.muted),
            );
        });

        ui.add_space(8.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0, palette.border),
        );
        ui.add_space(8.0);

        self.section_title(ui, "目标 URL", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            Self::styled_input(ui, &mut self.urls, "输入目标网站URL", palette);
        });

        ui.add_space(6.0);
        self.section_title(ui, "文件扩展名", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                for (ext, checked) in &mut self.extensions {
                    let fill = if *checked {
                        palette.selected
                    } else {
                        palette.input_bg
                    };
                    let stroke = if *checked {
                        Stroke::new(1.0, palette.primary)
                    } else {
                        Stroke::new(1.0, palette.border)
                    };
                    let text_color = if *checked {
                        palette.primary
                    } else {
                        palette.muted
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(ext.as_str()).size(12.0).color(text_color),
                            )
                            .fill(fill)
                            .stroke(stroke)
                            .corner_radius(CornerRadius::same(8))
                            .min_size(Vec2::new(56.0, 28.0)),
                        )
                        .clicked()
                    {
                        *checked = !*checked;
                    }
                }
            });
        });

        ui.add_space(6.0);
        self.section_title(ui, "存储设置", palette);
        Self::field_label(ui, "输出目录", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let _ = Self::action_button(
                        ui,
                        "📁",
                        palette.input_bg,
                        Stroke::new(1.0, palette.border),
                        palette.text,
                        Vec2::new(36.0, 36.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.output_dir)
                            .desired_width(f32::INFINITY)
                            .background_color(palette.input_bg),
                    );
                });
            });
        });

        ui.add_space(2.0);
        Self::field_label(ui, "内容格式", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            Self::select_popup_string(
                ui,
                "content_format_popup",
                &mut self.content_format,
                &[
                    ("markdown", "Markdown"),
                    ("json", "JSON"),
                    ("txt", "纯文本"),
                ],
                palette,
                Self::control_width(ui),
            );
        });

        ui.add_space(2.0);
        Self::field_label(ui, "界面主题", palette);
        Self::select_popup_theme(ui, &mut self.theme_mode, palette);

        ui.add_space(6.0);
        self.section_title(ui, "爬取策略", palette);
        Self::field_label(ui, "请求间隔", palette);
        Self::select_popup_string(
            ui,
            "delay_popup",
            &mut self.delay,
            &[
                ("无延迟", "无延迟"),
                ("100ms", "100ms"),
                ("200ms", "200ms"),
                ("300ms", "300ms"),
                ("500ms", "500ms"),
                ("800ms", "800ms"),
                ("1s", "1s"),
                ("2s", "2s"),
            ],
            palette,
            Self::control_width(ui),
        );

        ui.add_space(2.0);
        Self::field_label(ui, "最大深度", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            Self::select_popup_string(
                ui,
                "max_depth_popup",
                &mut self.max_depth,
                &[
                    ("1 层", "1 层"),
                    ("2 层", "2 层"),
                    ("3 层", "3 层"),
                    ("5 层", "5 层"),
                    ("不限制", "不限制"),
                ],
                palette,
                Self::control_width(ui),
            );
        });

        ui.add_space(2.0);
        Self::field_label(ui, "最早年度", palette);
        ui.add_enabled_ui(!lock_config, |ui| {
            Self::select_popup_string(
                ui,
                "min_year_popup",
                &mut self.min_year,
                &[
                    ("2026 年", "2026 年"),
                    ("2025 年", "2025 年"),
                    ("2024 年", "2024 年"),
                    ("2023 年", "2023 年"),
                    ("2022 年", "2022 年"),
                    ("2021 年", "2021 年"),
                ],
                palette,
                Self::control_width(ui),
            );
        });

        ui.add_space(6.0);
        self.section_title(ui, "已爬取站点", palette);
        egui::Frame::new()
            .fill(palette.input_bg)
            .stroke(Stroke::new(1.0, palette.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_height(120.0);
                let mut selected_site: Option<String> = None;
                let mut deleted_site: Option<String> = None;
                for (name, count) in &self.sites {
                    let is_selected = self.active_site.as_deref() == Some(name.as_str());
                    egui::Frame::new()
                        .fill(if is_selected {
                            palette.selected
                        } else {
                            Color32::TRANSPARENT
                        })
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .add(
                                        egui::Button::new(RichText::new(name).size(11.0).color(
                                            if is_selected {
                                                palette.primary
                                            } else {
                                                palette.text
                                            },
                                        ))
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE),
                                    )
                                    .clicked()
                                    && !self.is_running
                                {
                                    selected_site = Some(name.clone());
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_enabled(
                                                !self.is_running,
                                                egui::Button::new(RichText::new("🗑").size(11.0))
                                                    .fill(Color32::TRANSPARENT)
                                                    .stroke(Stroke::NONE),
                                            )
                                            .clicked()
                                        {
                                            deleted_site = Some(name.clone());
                                        }
                                        let _ = ui.add_enabled(
                                            !self.is_running,
                                            egui::Button::new(RichText::new("📂").size(11.0))
                                                .fill(Color32::TRANSPARENT)
                                                .stroke(Stroke::NONE),
                                        );
                                        egui::Frame::new()
                                            .fill(palette.hover)
                                            .corner_radius(CornerRadius::same(255))
                                            .inner_margin(Margin::symmetric(8, 3))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(count.to_string())
                                                        .size(10.0)
                                                        .color(palette.muted),
                                                );
                                            });
                                    },
                                );
                            });
                        });
                    ui.add_space(4.0);
                }
                if let Some(site_name) = selected_site {
                    self.apply_active_site_to_form(&site_name);
                }
                if let Some(site_name) = deleted_site {
                    self.files
                        .retain(|(name, _, _)| !name.starts_with(&format!("{site_name}/")));
                    self.sites.retain(|(name, _)| name != &site_name);
                    if self.active_site.as_deref() == Some(site_name.as_str()) {
                        self.active_site = None;
                        self.selected_file.clear();
                        self.preview = "单击左侧文件即可预览内容".to_string();
                    }
                    self.append_log(format!("[20:16:22] 已删除站点: {site_name}"));
                }
                if self.sites.is_empty() {
                    ui.label(RichText::new("暂无站点").size(11.0).color(palette.muted));
                }
            });
    }

    pub fn render_topbar(&mut self, ui: &mut egui::Ui, palette: ThemePalette) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("爬取控制").size(18.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!self.is_running, |ui| {
                        if Self::action_button(
                            ui,
                            "清空结果",
                            palette.danger.linear_multiply(0.14),
                            Stroke::new(1.0, palette.danger.linear_multiply(0.5)),
                            palette.danger,
                            Vec2::new(92.0, 34.0),
                        )
                        .clicked()
                        {
                            self.clear_results();
                        }
                    });

                    ui.add_enabled_ui(self.is_running, |ui| {
                        if Self::action_button(
                            ui,
                            "停止",
                            palette.input_bg,
                            Stroke::new(1.0, palette.border),
                            palette.text,
                            Vec2::new(72.0, 34.0),
                        )
                        .clicked()
                        {
                            self.stop_current_task();
                        }
                    });

                    ui.add_enabled_ui(!self.is_running, |ui| {
                        if Self::action_button(
                            ui,
                            "开始爬取",
                            palette.primary,
                            Stroke::NONE,
                            Color32::WHITE,
                            Vec2::new(96.0, 34.0),
                        )
                        .clicked()
                        {
                            self.begin_crawl();
                        }
                    });

                    ui.add_enabled_ui(!self.is_running, |ui| {
                        if Self::action_button(
                            ui,
                            "全站预爬",
                            palette.input_bg,
                            Stroke::new(1.0, palette.border),
                            palette.text,
                            Vec2::new(96.0, 34.0),
                        )
                        .clicked()
                        {
                            self.begin_pre_crawl();
                        }
                    });
                });
            });
        });
    }

    pub fn render_progress(&self, ui: &mut egui::Ui, palette: ThemePalette) {
        self.glass_frame(palette).show(ui, |ui| {
            let width = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 6.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(255), palette.input_bg);

            let progress_width = rect.width() * self.progress.clamp(0.0, 1.0);
            if progress_width > 0.0 {
                let progress_rect =
                    egui::Rect::from_min_size(rect.min, Vec2::new(progress_width, rect.height()));
                ui.painter()
                    .rect_filled(progress_rect, CornerRadius::same(255), palette.primary);
            }

            ui.add_space(8.0);
            ui.label(
                RichText::new(&self.progress_text)
                    .size(12.0)
                    .color(palette.muted),
            );
        });
    }

    pub fn render_tabs(&mut self, ui: &mut egui::Ui, palette: ThemePalette) {
        let tab_names = ["日志", "文件列表", "结果"];
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            for (i, name) in tab_names.iter().enumerate() {
                let selected = self.active_tab == i;
                let response = ui.add(
                    egui::Button::new(RichText::new(*name).size(14.0).color(if selected {
                        palette.primary
                    } else {
                        palette.muted
                    }))
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE)
                    .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    self.active_tab = i;
                }
                let underline_rect = egui::Rect::from_min_max(
                    egui::pos2(response.rect.left(), response.rect.bottom() - 1.0),
                    egui::pos2(response.rect.right(), response.rect.bottom() + 1.0),
                );
                if selected {
                    ui.painter().rect_filled(
                        underline_rect,
                        CornerRadius::same(1),
                        palette.primary,
                    );
                }
            }
        });
        ui.add_space(4.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0, palette.border),
        );
        ui.add_space(12.0);
    }

    pub fn render_logs(&self, ui: &mut egui::Ui, palette: ThemePalette) {
        self.glass_frame(palette).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(&self.logs)
                            .size(12.0)
                            .family(egui::FontFamily::Monospace)
                            .color(palette.muted),
                    );
                });
        });
    }

    pub fn render_files(&mut self, ui: &mut egui::Ui, palette: ThemePalette) {
        let visible_files = self.visible_files();
        ui.horizontal(|ui| {
            let resizer_width = 6.0f32;
            let spacing = ui.spacing().item_spacing.x;
            let available_height = ui.available_height();
            let max_left_width = (ui.available_width() - resizer_width - spacing * 2.0 - 360.0)
                .max(260.0);
            self.file_list_width = self.file_list_width.clamp(260.0, max_left_width);
            let left_width = self.file_list_width;
            let right_width = (ui.available_width() - left_width - resizer_width - spacing * 2.0)
                .max(360.0);

            ui.allocate_ui_with_layout(
                Vec2::new(left_width, available_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.glass_frame(palette).show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if visible_files.is_empty() {
                                    ui.label(RichText::new("暂无文件").size(12.0).color(palette.muted));
                                }
                                for (name, status, url) in &visible_files {
                                    let selected = self.selected_file == *name;
                                    let status_text = match status.as_str() {
                                        "new" => "新增",
                                        "updated" => "更新",
                                        _ => "未变",
                                    };
                                    let status_color = match status.as_str() {
                                        "new" => palette.success,
                                        "updated" => palette.warning,
                                        _ => palette.muted,
                                    };

                                    let item_fill = if selected {
                                        palette.selected
                                    } else {
                                        Color32::TRANSPARENT
                                    };
                                    let response = egui::Frame::new()
                                        .fill(item_fill)
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new("📄")
                                                        .size(14.0)
                                                        .color(palette.muted),
                                                );
                                                ui.label(
                                                    RichText::new(
                                                        name.split_once('/')
                                                            .map(|(_, file_name)| file_name)
                                                            .unwrap_or(name.as_str()),
                                                    )
                                                        .size(12.0)
                                                        .color(if selected {
                                                            palette.primary
                                                        } else {
                                                            palette.text
                                                        }),
                                                );
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            RichText::new(status_text)
                                                                .size(11.0)
                                                                .color(status_color),
                                                        );
                                                    },
                                                );
                                            });
                                        })
                                        .response
                                        .interact(egui::Sense::click());

                                    if response.clicked() {
                                        self.selected_file = name.clone();
                                        self.preview = format!(
                                            "# {}\n\n来源状态：{}\n来源链接：{}\n\n这是 {} 的预览内容。\n\n- 采集时间：2026-04-09\n- 内容格式：Markdown\n- 抽取状态：已完成\n\n| 字段 | 值 |\n| --- | --- |\n| 标题 | {} |\n| 分类 | 政策通知 |\n| 标签 | RAG / 爬虫 / 文档 |\n",
                                            name, status_text, url, name, name
                                        );
                                    }
                                    ui.add_space(4.0);
                                }
                            });
                    });
                },
            );

            ui.allocate_ui(Vec2::new(resizer_width, available_height), |ui| {
                let rect = ui.max_rect();
                let response = ui.interact(
                    rect,
                    ui.id().with("file_panel_resizer"),
                    egui::Sense::click_and_drag(),
                );
                if response.drag_started() {
                    self.drag_start_file_list_width = Some(self.file_list_width);
                }
                if response.dragged() {
                    if let Some(start_width) = self.drag_start_file_list_width {
                        self.file_list_width =
                            (start_width + response.drag_delta().x).clamp(260.0, max_left_width);
                    }
                }
                if response.drag_stopped() {
                    self.drag_start_file_list_width = None;
                }

                let fill = if response.dragged() || response.hovered() {
                    palette.primary
                } else {
                    palette.resizer
                };
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(255), fill);
            });

            ui.allocate_ui_with_layout(
                Vec2::new(right_width, available_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    self.glass_frame(palette).show(ui, |ui| {
                        egui::Frame::new()
                            .fill(palette.input_bg)
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                let title = if self.selected_file.is_empty() {
                                    "选择文件预览".to_string()
                                } else {
                                    self.selected_file.clone()
                                };
                                ui.label(RichText::new(title).size(13.0).strong());
                            });

                        ui.add_space(8.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.set_min_height(ui.available_height());
                                self.render_markdown_preview(ui, palette);
                            });
                    });
                },
            );
        });
    }

    pub fn render_results(&self, ui: &mut egui::Ui, palette: ThemePalette) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            let cards = [
                ("总计", self.total_count, palette.primary),
                ("新增", self.new_count, palette.success),
                ("更新", self.updated_count, palette.warning),
                ("未变", self.unchanged_count, palette.muted),
                ("错误", self.error_count, palette.danger),
            ];

            for (label, count, color) in cards {
                egui::Frame::new()
                    .fill(palette.surface)
                    .stroke(Stroke::new(1.0, palette.border))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(Margin::symmetric(24, 20))
                    .show(ui, |ui| {
                        ui.set_min_width(120.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(count.to_string())
                                    .size(32.0)
                                    .strong()
                                    .color(color),
                            );
                            ui.add_space(4.0);
                            ui.label(RichText::new(label).size(12.0).color(palette.muted));
                        });
                    });
            }
        });
    }

    pub fn render_markdown_preview(&self, ui: &mut egui::Ui, palette: ThemePalette) {
        let lines: Vec<&str> = self.preview.lines().collect();
        let mut i = 0usize;

        while i < lines.len() {
            let line = lines[i].trim_end();
            let trimmed = line.trim();

            if trimmed.is_empty() {
                ui.add_space(6.0);
                i += 1;
                continue;
            }

            if let Some(title) = trimmed.strip_prefix("# ") {
                ui.label(RichText::new(title).size(22.0).strong().color(palette.text));
                ui.add_space(6.0);
                i += 1;
                continue;
            }

            if trimmed.starts_with("| ") && trimmed.ends_with(" |") && i + 1 < lines.len() {
                egui::Grid::new(format!("preview_table_{i}"))
                    .striped(true)
                    .spacing(Vec2::new(12.0, 8.0))
                    .show(ui, |ui| {
                        while i < lines.len() {
                            let row = lines[i].trim();
                            if !(row.starts_with('|') && row.ends_with('|')) {
                                break;
                            }
                            if row.chars().all(|c| c == '|' || c == ' ' || c == '-') {
                                i += 1;
                                continue;
                            }

                            let cells: Vec<&str> = row
                                .trim_matches('|')
                                .split('|')
                                .map(|cell| cell.trim())
                                .collect();
                            for (cell_index, cell) in cells.iter().enumerate() {
                                let rich = if cell_index == 0 {
                                    RichText::new(*cell).strong().color(palette.text)
                                } else {
                                    RichText::new(*cell).color(palette.muted)
                                };
                                ui.label(rich);
                            }
                            ui.end_row();
                            i += 1;
                        }
                    });
                ui.add_space(4.0);
                continue;
            }

            if let Some(item) = trimmed.strip_prefix("- ") {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("•").size(14.0).color(palette.primary));
                    ui.label(RichText::new(item).size(12.0).color(palette.text));
                });
                i += 1;
                continue;
            }

            if let Some(url) = trimmed.strip_prefix("来源链接：") {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("来源链接：").size(12.0).color(palette.muted));
                    ui.hyperlink_to(url, url);
                });
                i += 1;
                continue;
            }

            let color = if trimmed.contains("来源状态") || trimmed.contains("采集时间") {
                palette.muted
            } else {
                palette.text
            };
            ui.label(RichText::new(trimmed).size(12.0).color(color));
            i += 1;
        }
    }
}
