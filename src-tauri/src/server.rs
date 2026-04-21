use axum::{
    extract::{Path, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

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
    let output_dir = payload
        .output_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "output".to_string());
    let threshold = payload.threshold.unwrap_or(0.0);

    // Call the same logic that Tauri command uses
    match crate::search::perform_api_search(
        output_dir.clone(),
        payload.site_name.clone(),
        payload.query.clone(),
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
                result.inject_urls(state.port, &payload.site_name, &output_dir, &payload.query);
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

    // 安全剥离任何由双斜杠导致的前缀绝对路径符，避免 Path::join 时发生根节点覆盖
    let safe_path = path.trim_start_matches('/');

    // 增加对 dpc_search_server.html 测试探针文件的白名单放行（从根目录加载而不是 output_dir）
    let full_path = if safe_path == "dpc_search_server.html" {
        crate::fs_utils::resolve_path(".").join(safe_path)
    } else {
        base.join(safe_path)
    };

    if let Ok(bytes) = tokio::fs::read(&full_path).await {
        let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
        let mut body_bytes = bytes;

        if path.ends_with(".html") || path.ends_with(".htm") {
            if let Some(highlight_block) = query.get("highlight_block") {
                if let Ok(html_str) = String::from_utf8(body_bytes.clone()) {
                    use base64::Engine;
                    let b64_term =
                        base64::engine::general_purpose::STANDARD.encode(highlight_block);
                    let script = format!(
                        r#"
<script>
document.addEventListener("DOMContentLoaded", function() {{
    try {{
        var term = decodeURIComponent(escape(atob("{}")));
        if (!term) return;
        var cleanMd = term.replace(/[^\u4e00-\u9fa5a-zA-Z0-9]/g, '');
        if (cleanMd.length < 5) return;
        var prefix = cleanMd.substring(0, Math.min(20, cleanMd.length));
        var suffix = cleanMd.substring(Math.max(0, cleanMd.length - 20));

        var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
            acceptNode: function(node) {
                var p = node.parentNode;
                if (!p) return NodeFilter.FILTER_ACCEPT;
                var tag = p.tagName.toLowerCase();
                if (tag === 'script' || tag === 'style' || tag === 'noscript') return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            }
        }, false);
        var nodes = [];
        var fullText = "";
        var n;
        while(n = walker.nextNode()) {{
            if (n.nodeValue.trim().length === 0) continue;
            var val = n.nodeValue.replace(/[^\u4e00-\u9fa5a-zA-Z0-9]/g, '');
            if (val.length === 0) continue;
            nodes.push({{ node: n, start: fullText.length, end: fullText.length + val.length }});
            fullText += val;
        }}

        var startIndex = fullText.indexOf(prefix);
        var endIndex = fullText.indexOf(suffix, startIndex);
        endIndex = endIndex !== -1 ? endIndex + suffix.length : -1;

        if (startIndex !== -1 && endIndex !== -1 && endIndex >= startIndex) {{
            var startAnchor = null;
            var endAnchor = null;
            for (var i = 0; i < nodes.length; i++) {{
                if (!startAnchor && nodes[i].end > startIndex) startAnchor = nodes[i].node;
                if (nodes[i].start < endIndex) endAnchor = nodes[i].node;
            }}
            if (startAnchor && endAnchor) {{
                var range = document.createRange();
                range.setStartBefore(startAnchor);
                range.setEndAfter(endAnchor);
                var hw = document.createTreeWalker(range.commonAncestorContainer, NodeFilter.SHOW_TEXT, {{
                    acceptNode: function(node) {
                        if (node.nodeValue.trim().length === 0) return NodeFilter.FILTER_REJECT;
                        var p = node.parentNode;
                        if (p) {
                            var tag = p.tagName.toLowerCase();
                            if (tag === 'script' || tag === 'style' || tag === 'noscript') return NodeFilter.FILTER_REJECT;
                        }
                        if (range.intersectsNode(node)) return NodeFilter.FILTER_ACCEPT;
                        return NodeFilter.FILTER_REJECT;
                    }
                }}, false);
                var nodesToWrap = [];
                var hn;
                while(hn = hw.nextNode()) nodesToWrap.push(hn);
                nodesToWrap.forEach(function(node) {{
                    var span = document.createElement('span');
                    span.className = 'api-sandbox-highlight';
                    span.style.backgroundColor = 'rgba(250, 204, 21, 0.4)';
                    span.style.color = '#000';
                    node.parentNode.insertBefore(span, node);
                    span.appendChild(node);
                }});
                setTimeout(() => {{
                    if (startAnchor && startAnchor.parentElement) {{
                        startAnchor.parentElement.scrollIntoView({{behavior: 'smooth', block: 'center'}});
                    }}
                }}, 100);
            }}
        }}
    }} catch(e) {{ console.error("Highlight block injection failed", e); }}
}});
</script></body>"#,
                        b64_term
                    );
                    let injected = html_str.replace("</body>", &script);
                    body_bytes = injected.into_bytes();
                }
            } else if let Some(highlight_term) = query.get("highlight") {
                if let Ok(html_str) = String::from_utf8(body_bytes.clone()) {
                    use base64::Engine;
                    let b64_term = base64::engine::general_purpose::STANDARD.encode(highlight_term);
                    let script = format!(
                        r#"
<script>
document.addEventListener("DOMContentLoaded", function() {{
    try {{
        var term = decodeURIComponent(escape(atob("{}")));
        if (!term) return;
        var words = term.split(/\s+/).filter(w => w.trim());
        var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {
            acceptNode: function(node) {
                var p = node.parentNode;
                if (!p) return NodeFilter.FILTER_ACCEPT;
                var tag = p.tagName.toLowerCase();
                if (tag === 'script' || tag === 'style' || tag === 'noscript') return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            }
        }, false);
        var nodes = [];
        var n;
        while(n = walker.nextNode()) nodes.push(n);
        nodes.forEach(node => {{
            var text = node.nodeValue;
            var hasMatch = false;
            words.forEach(w => {{
                if (w.length >= 2 || /[\u4e00-\u9fa5]/.test(w)) {{
                    if (text.toLowerCase().includes(w.toLowerCase())) hasMatch = true;
                }}
            }});
            if (hasMatch) {{
                var span = document.createElement('span');
                var highlighted = text;
                words.forEach(w => {{
                    if (w.length >= 2 || /[\u4e00-\u9fa5]/.test(w)) {{
                        var regex = new RegExp("(" + w + ")", "gi");
                        highlighted = highlighted.replace(regex, "<mark style='background-color: rgba(250, 204, 21, 0.4); color: #000; border-radius: 2px; padding: 0 2px;'>$1</mark>");
                    }}
                }});
                span.innerHTML = highlighted;
                node.parentNode.replaceChild(span, node);
            }}
        }});
        setTimeout(() => {{
            var firstMark = document.querySelector('mark');
            if (firstMark) firstMark.scrollIntoView({{behavior: 'smooth', block: 'center'}});
        }}, 100);
    }} catch(e) {{ console.error("Highlight injection failed", e); }}
}});
</script></body>"#,
                        b64_term
                    );
                    let injected = html_str.replace("</body>", &script);
                    body_bytes = injected.into_bytes();
                }
            }
        }

        let content_type_str = mime.as_ref();
        let final_content_type = if content_type_str.starts_with("text/") {
            format!("{}; charset=utf-8", content_type_str)
        } else {
            content_type_str.to_string()
        };

        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", final_content_type)
            .body(axum::body::Body::from(body_bytes))
            .unwrap()
    } else {
        (
            StatusCode::NOT_FOUND,
            format!("File not found: {}", full_path.display()),
        )
            .into_response()
    }
}

async fn search_page_handler() -> Response {
    let full_path = crate::fs_utils::resolve_path(".").join("dpc_search_server.html");
    if let Ok(bytes) = tokio::fs::read(&full_path).await {
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
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
        .route("/search", get(search_page_handler))
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
