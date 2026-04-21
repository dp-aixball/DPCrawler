# DPCrawler RAG 检索引擎 API 接口文档 v3

> **版本说明**：此处的 v3 仅为文档层面的更新（补充了内置测试页面、MD 下载机制以及高亮控制等接入细节），Server 端的实际 API 接口及底层行为逻辑与 v2 版本完全一致，已有业务可以直接沿用，无需任何代码改动。

DPCrawler 底层检索引擎已经完成重构。目前实现了前端 HTML 渲染与后端文本索引的架构解耦。
本服务器（Server）的核心设计旨在解决大语言模型（LLM）应用中的两个主要诉求：
1. **知识检索**：根据用户的自然语言问题或关键词，从本地知识库中快速检索并提取出高度相关的知识片段（作为 RAG 上下文）。
2. **知识溯源**：为大模型的生成结果提供可靠的参考出处，允许终端用户通过专属链接直接访问、查阅带有动态高亮标记的原始完整文档。

为了支持跨服务器、分布式部署，本接口采用 **HTTP RESTful API + 内置静态资源服务** 的方式，向外部提供检索片段及原文附件的访问能力。

---

## 1. 内置可视化测试页面 (Demo UI)：`GET /search`

为了方便开发测试与直观体验，检索引擎内置了一个开箱即用的前端交互页面。
直接在浏览器中访问服务器的 `/search` 路径（例如 `http://127.0.0.1:18088/search`），即可打开独立的检索测试面板。
该页面集成了全部 API 的调用逻辑，支持可视化切换站点、发起自然语言检索，并能在悬浮层中直接预览带高亮效果的 HTML 文档。
> 💡 **提示**：该测试页面的前端 HTML/JS 源代码是完全公开且未经过任何混淆或隐藏的。前端开发人员可以直接在浏览器中“查看网页源代码（View Source）”，作为接入 API 的参考示例。

---

## 2. 站点查询接口：`GET /api/v1/sites`

查询当前服务器（或默认 `output` 根目录）下已挂载的所有独立爬虫站点数据集名称。第三方 UI 可借助该接口生成下拉菜单等交互组件。
* **返回格式**：`["SiteA", "SiteB", "Dp_Gov"]`

---

## 3. 核心检索接口：`POST /api/v1/search`

分布式知识库检索入口，允许外部 Agent 跨网调用。

### 3.1 输入参数 (Request Payload)

Content-Type: `application/json`

| 字段名       | 类型     | 必填 | 描述                                                                                                                                                 |
| :----------- | :------- | :--- | :--------------------------------------------------------------------------------------------------------------------------------------------------- |
| `site_name`  | `String` | 是   | 具体需要检索的站点工程名（用于限定知识库范围）。                                                                                           |
| `output_dir` | `String` | 否   | 后台数据的根存储路径，不传时默认在当前工作目录的 `output` 文件夹中寻找。                                            |
| `query`      | `String` | 是   | 用户的自然语言查询内容或长提示词（系统具有自动泛用分词和降噪能力）。                                                                         |
| `top_k`      | `usize`  | 是   | 期望返回的相关文档数量上限（如 `5`）。                                                                                                     |
| `threshold`  | `f64`    | 否   | 相似度或密度的最低阈值（默认为 `0.0` 即不过滤），用于屏蔽低相关性匹配。                                                                            |

<br>

### 3.2 输出结果 (Response Array)

返回包含 `SearchResult` 对象的 JSON 数组。响应数据不再返回服务器的物理绝对路径，而是提供用于直接下载或访问的 **相对 URL 路径**。

```json
[
  {
    "filename": "docs/2026年报名简章.md", // 相对于站点根目录的文件路径
    "title": "2026年同等学力人员申请硕士学位报考指南", 
    "score": 45, 
    "snippet": "2026 年同等学力人员申请硕士学位外国语水平...报名流程见附件。", 
    "url": "https://tdxl.neea.edu.cn/notice/1", // 网页原文的在线出处
    "start_line": 24, 
    "end_line": 28, 
    "matched_block": "（二）考试报名\n符合报考资格的考生须于 3 月 16 日...", // 检索提取的连续上下文切片，主要作为 RAG 知识送入远程 LLM。
    "local_path": "/absolute/path/to/output/...md", // 物理机上的原始绝对位置
    
    // 通过 DPCrawler 内置静态服务器暴露的链接
    "md_download_url": "/files/SiteA/docs/2026年报名简章.md?output_dir=...", // Markdown 全文下载链接
    "html_view_url": "/files/SiteA/html_views/2026年报名简章.html?output_dir=...&highlight=查询词", // 默认带有检索词高亮参数的 HTML 预览链接
    "html_block_view_url": "/files/SiteA/html_views/2026年报名简章.html?output_dir=...&highlight_block=核心块内容" // 带有匹配段落高亮参数的 HTML 预览链接
  }
]
```

> 💡 **前端接入说明：URL 的使用、高亮与文件获取**
> 
> 后端静态服务内置了高亮脚本。前端在 `<iframe src="...">` 中直接使用上述 URL 时，页面会自动定位并高亮匹配内容。
> 
> 1. **文本块高亮**：使用 `html_block_view_url`，页面会自动滚动到目标段落块并高亮背景。
> 2. **关键词高亮**：使用 `html_view_url`，页面会自动滚动并高亮命中的离散检索词。
> 3. **获取无高亮的原文**：`html_view_url` 默认带有 `&highlight=...` 参数。如需获取无任何高亮、且不自动滚动的原始 HTML 页面，只需在请求时移除该参数即可。
>    ```javascript
>    // 剥离 highlight 参数的示例代码：
>    let urlObj = new URL(result.html_view_url, window.location.origin);
>    urlObj.searchParams.delete('highlight');
>    let cleanOriginalUrl = urlObj.pathname + urlObj.search; 
>    // 使用 cleanOriginalUrl 即可获得原始页面
>    ```
> 4. **获取包含元数据的 Markdown 原文**：通过 `md_download_url` 可直接获取或下载 Markdown 格式的完整原文。**重点：** 该 Markdown 文件的头部固定包含标准 YAML 格式的元数据块（Front Matter），记录了文章真实的 `title`、`source_url`（原始在线出处）以及抓取时间等核心溯源元数据，非常适合提供给 LLM 进行深度结构化分析或归档提取。

---

## 4. 跨服务前端渲染规范

当外部业务接收到检索结果的 JSON 后，可通过返回的静态服务链接获取原文并渲染。
为了实现文档的自动高亮和定位，后端的处理逻辑如下，供前端了解：

1. **页面挂载**：前端将响应体中的 `html_view_url` 赋给 `<iframe src="...">` 组件。
2. **高亮脚本注入**：后端在返回 HTML 文件时，会读取 URL 中的 `highlight` 或 `highlight_block` 参数，动态在结尾处注入一段 JavaScript 脚本。
3. **DOM 文本定位**：注入的脚本在 `DOMContentLoaded` 后执行，通过 `TreeWalker` 遍历 HTML 文本节点，寻找与参数匹配的文本内容。
4. **视觉反馈**：利用 Web `Range` API 对匹配的文本节点进行背景色标记，并调用 `scrollIntoView()` 滚动至可视区域内。

通过这种 **接口提供链接 + 静态文件下发** 的方式：
*   **带宽优化**：后端接口直接下发轻量的文档片段，避免了将冗长的 HTML 代码混入 JSON 响应体内，减小了 API 解析与传输开销。
*   **RAG 适用性**：大语言模型（LLM）可以仅获取精简的文本块（`matched_block`）和文件链接，提高处理效率，节省 Token 消耗。
*   **内容溯源**：应用层的最终用户依然能通过 URL 访问保持原始格式与排版的完整文档，实现可靠的信息溯源。
