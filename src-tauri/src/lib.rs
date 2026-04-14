pub mod process;
pub mod fs_utils;
pub mod commands;
use process::disable_webkit_cache;
use process::CRAWLER_PID;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    disable_webkit_cache();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build());

    // MCP Bridge only in debug builds (for AI testing); excluded from release
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .invoke_handler(tauri::generate_handler![
            commands::run_crawler,
            commands::run_pre_crawl,
            commands::stop_crawler,
            commands::save_pre_crawl_result,
            commands::load_pre_crawl_result,
            commands::read_config,
            commands::write_config,
            commands::read_index,
            commands::open_url,
            commands::read_file_content,
            commands::update_delay,
            commands::clear_output,
            commands::delete_site,
            commands::list_crawled_sites,
            commands::read_site_config,
            commands::read_site_index,
            commands::force_quit,
            commands::get_app_version
        ])
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();


            let w = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let pid = CRAWLER_PID.load(Ordering::SeqCst);
                    if pid > 0 {
                        api.prevent_close();
                        let _ = w.emit("confirm-exit", ());
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    let pid = CRAWLER_PID.load(Ordering::SeqCst);
                    if pid > 0 {
                        api.prevent_exit();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.emit("confirm-exit", ());
                        }
                    }
                }
                _ => {}
            }
        });
}
