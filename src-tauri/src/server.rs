use axum::{
    extract::{Path, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::fs_utils;

#[derive(Deserialize)]
pub struct SearchRequest {
    pub output_dir: Option<String>,
    pub site_name: String,
    pub query: String,
    pub top_k: usize,
    pub threshold: Option<f64>,
}

#[derive(Clone)]
struct AppState {
    port: u16,
}

// RESTful API 处理器
async fn api_search_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SearchRequest>,
) -> Result<Json<Vec<crate::search::SearchResult>>, StatusCode> {
    let output_dir = payload.output_dir.unwrap_or_else(|| "./output".to_string());
    let threshold = payload.threshold.unwrap_or(0.0);

    // Call the same logic that Tauri command uses
    match crate::commands::api_search(
        output_dir.clone(),
        payload.site_name.clone(),
        payload.query,
        payload.top_k,
        threshold,
    )
    .await
    {
        Ok(mut results) => {
            // Post-process to inject the HTTP server URL paths
            for result in &mut results {
                // filename format from search is like "SiteName/docs/xxxx.md"
                // But the SearchResult in Rust contains raw paths.
                // We'll update SearchResult directly during search generation.
                // However, wait, search generator needs to know the port.
                // Actually, let's inject it here.
                let _base_url = payload.site_name.clone();
                // Because `filename` in result usually looks like "SiteName/Filename" from file_tree.
                // Wait, search.rs has access to `filename`. Let's just fix the URLs in search.rs.
                result.inject_urls(state.port, &payload.site_name, &output_dir);
            }
            Ok(Json(results))
        }
        Err(e) => {
            eprintln!("Search API error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn file_handler(
    Path(path): Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let output_dir = query
        .get("output_dir")
        .cloned()
        .unwrap_or_else(|| "./output".to_string());
    let base = crate::fs_utils::resolve_path(&output_dir);
    // Path might contain urlencoded characters, handled automatically by axum Path wrapper
    let full_path = base.join(&path);

    if let Ok(bytes) = tokio::fs::read(&full_path).await {
        let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime.as_ref())
            .body(axum::body::Body::from(bytes))
            .unwrap()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("File not found: {}", full_path.display()),
        )
            .into_response()
    }
}

pub async fn run_server(port: u16) {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let shared_state = Arc::new(AppState { port });

    let app = Router::new()
        // API 路由
        .route("/api/v1/search", post(api_search_handler))
        // 动态读取静态路由，规避 ServeDir 固定绑定 data_dir 导致二级域名或跨绝对路径的 404 断层
        .route("/files/*path", get(file_handler))
        .layer(cors)
        .with_state(shared_state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    println!("HTTP Server started at http://0.0.0.0:{}", port);

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("Server error: {}", e);
    }
}
