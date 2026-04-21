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

#[derive(Deserialize)]
pub struct ListSitesQuery {
    pub output_dir: Option<String>,
}

async fn list_sites_handler(
    axum::extract::Query(payload): axum::extract::Query<ListSitesQuery>,
) -> impl IntoResponse {
    let output_dir = payload
        .output_dir
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "output".to_string());

    let base = crate::fs_utils::resolve_path(&output_dir);
    let mut sites = vec![];

    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    if let Ok(name) = entry.file_name().into_string() {
                        sites.push(name);
                    }
                }
            }
        }
    }

    Json(sites)
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
        
        // --- CRISIS FIX ---
        // Markdown 格式文本中包含了 [Title](url) 的链接形式
        var dmd = term.replace(/!\[[^\]]*\]\([^\)]*\)/g, '');
        dmd = dmd.replace(/\[([^\]]+)\]\([^\)]+\)/g, '$1');
        dmd = dmd.replace(/<(https?:\/\/[^>]+)>/g, '');
        
        // 针对表格等复杂结构，按空白符和表格分隔符进行词汇级切分
        var rawTokens = dmd.split(/[\s\|\n]+/);
        var tokens = [];
        rawTokens.forEach(function(t) {{
            var ct = t.replace(/[^\u4e00-\u9fa5a-zA-Z0-9]/g, '');
            // 核心修复：PDF 的文本流常常会在长句中间被页眉、隐形换行等杂音硬生生切断！
            // 如果不拆分，几十个字的中文段落（无空格）在 indexOf 时只要碰到一个杂音就会全军覆没得 0 分。
            // 因此必须将过长的 Token 切块（N-gram），保证容错率。
            if (ct.length > 8) {{
                for (var i = 0; i < ct.length; i += 5) {{
                    var chunk = ct.substring(i, i + 5);
                    if (chunk.length >= 2) tokens.push(chunk);
                }}
            }} else if (ct.length >= 2) {{
                tokens.push(ct);
            }}
        }});
        
        if (tokens.length === 0) {{
            rawTokens.forEach(function(t) {{
                var ct = t.replace(/[^\u4e00-\u9fa5a-zA-Z0-9]/g, '');
                if (ct.length > 0) tokens.push(ct);
            }});
        }}
        if (tokens.length === 0) return;

        var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {{
            acceptNode: function(node) {{
                var p = node.parentNode;
                if (!p) return NodeFilter.FILTER_ACCEPT;
                var tag = p.tagName.toLowerCase();
                if (tag === 'script' || tag === 'style' || tag === 'noscript') return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            }}
        }}, false);
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

        // --- 核心修复：无序聚类算法 ---
        var tokenPositions = [];
        var uniqueTokens = [];
        tokens.forEach(function(t) {{
            if (uniqueTokens.indexOf(t) === -1) uniqueTokens.push(t);
        }});

        for(var i = 0; i < uniqueTokens.length; i++) {{
            var t = uniqueTokens[i];
            var searchPos = 0;
            while(true) {{
                var pos = fullText.indexOf(t, searchPos);
                if(pos === -1) break;
                tokenPositions.push({{ tIdx: i, start: pos, end: pos + t.length }});
                searchPos = pos + 1;
            }}
        }}

        if (tokenPositions.length === 0) return;

        tokenPositions.sort(function(a, b) {{ return a.start - b.start; }});

        var W = Math.max(1500, uniqueTokens.join('').length * 4);
        var maxScore = -1;
        var bestWindowStart = -1;
        var bestWindowEnd = -1;

        for (var i = 0; i < tokenPositions.length; i++) {{
            var windowStart = tokenPositions[i].start;
            var windowEnd = windowStart + W;
            
            var seen = {{}};
            var score = 0;
            var actualEnd = windowStart;

            for (var j = i; j < tokenPositions.length; j++) {{
                if (tokenPositions[j].start > windowEnd) break;
                
                var tIdx = tokenPositions[j].tIdx;
                if (!seen[tIdx]) {{
                    seen[tIdx] = true;
                    // 使用词汇长度作为权重，越长的特征词越能决定真实位置！
                    score += uniqueTokens[tIdx].length;
                    // 核心修复：只在“首次发现新的特征词”时扩展高亮边界。
                    // 绝不让几百字外的重复废词把高亮框强行拉长好几倍！
                    actualEnd = Math.max(actualEnd, tokenPositions[j].end);
                }}
            }}

            if (score > maxScore) {{
                maxScore = score;
                bestWindowStart = windowStart;
                bestWindowEnd = actualEnd;
            }} else if (score === maxScore && maxScore > 0) {{
                if ((actualEnd - windowStart) < (bestWindowEnd - bestWindowStart)) {{
                    bestWindowStart = windowStart;
                    bestWindowEnd = actualEnd;
                }}
            }}
        }}

        // 恢复“整块高亮”的视觉体验：将密度聚类找到的真实边界 [bestWindowStart, bestWindowEnd] 之间的所有文本节点全部高亮，避免像斑马线一样断断续续。
        if (bestWindowStart !== -1 && bestWindowEnd !== -1) {{
            var startAnchor = null;
            for (var i = 0; i < nodes.length; i++) {{
                var nObj = nodes[i];
                // 只要该文本节点与最佳窗口有交集，就予以高亮
                if (nObj.start < bestWindowEnd && nObj.end > bestWindowStart) {{
                    if (!startAnchor) startAnchor = nObj.node;
                    if (!nObj.wrapped) {{
                        var span = document.createElement('span');
                        span.className = 'api-sandbox-highlight';
                        span.style.backgroundColor = 'rgba(250, 204, 21, 0.4)';
                        span.style.color = '#000';
                        nObj.node.parentNode.insertBefore(span, nObj.node);
                        span.appendChild(nObj.node);
                        nObj.wrapped = true;
                    }}
                }}
            }}
            setTimeout(() => {{
                if (startAnchor && startAnchor.parentElement) {{
                    startAnchor.parentElement.scrollIntoView({{behavior: 'smooth', block: 'center'}});
                }}
            }}, 100);
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
        var words = [];
        if (typeof Intl !== 'undefined' && Intl.Segmenter) {{
            var segmenter = new Intl.Segmenter('zh-CN', {{ granularity: 'word' }});
            for (var seg of segmenter.segment(term)) {{
                if (seg.isWordLike && seg.segment.length > 1) words.push(seg.segment);
            }}
        }} else {{
            words = term.split(/[\s,，.。?？!！;；、]/).filter(w => w.trim());
        }}
        if (words.length === 0) words = [term.trim()];
        words.sort((a, b) => b.length - a.length);
        var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, {{
            acceptNode: function(node) {{
                var p = node.parentNode;
                if (!p) return NodeFilter.FILTER_ACCEPT;
                var tag = p.tagName.toLowerCase();
                if (tag === 'script' || tag === 'style' || tag === 'noscript') return NodeFilter.FILTER_REJECT;
                return NodeFilter.FILTER_ACCEPT;
            }}
        }}, false);
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
        .route("/api/v1/sites", get(list_sites_handler))
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
