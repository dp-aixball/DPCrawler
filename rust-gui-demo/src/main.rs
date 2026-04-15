mod app;
mod components;
mod theme;
mod crawler;

use app::DPCrawlerDemo;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([960.0, 680.0])
            .with_title("DPCrawler - RAG知识爬虫"),
        ..Default::default()
    };

    eframe::run_native(
        "DPCrawler - RAG知识爬虫",
        options,
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            let chinese_font_paths = [
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/opentype/noto-cjk/NotoSansCJKsc-Regular.otf",
                "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            ];

            for path in &chinese_font_paths {
                if std::path::Path::new(path).exists() {
                    if let Ok(data) = std::fs::read(path) {
                        fonts.font_data.insert(
                            "chinese".to_owned(),
                            std::sync::Arc::new(egui::FontData::from_owned(data)),
                        );
                        fonts
                            .families
                            .get_mut(&egui::FontFamily::Proportional)
                            .unwrap()
                            .insert(0, "chinese".to_owned());
                        fonts
                            .families
                            .get_mut(&egui::FontFamily::Monospace)
                            .unwrap()
                            .insert(0, "chinese".to_owned());
                        break;
                    }
                }
            }

            cc.egui_ctx.set_fonts(fonts);
            Ok(Box::new(DPCrawlerDemo::default()))
        }),
    )
}
