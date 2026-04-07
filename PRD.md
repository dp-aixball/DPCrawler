# DPCrawler - RAG知识问答爬虫

## 1. 项目概述

**项目目标**：为RAG（检索增强生成）知识问答系统提供**跨平台桌面客户端**工具

**核心价值**：
- 从目标网站抓取文档内容
- 以LLM友好的格式存储（Markdown）
- 支持增量爬取，快速识别更新内容
- 图形界面操作，跨平台运行（Windows/Mac/Linux）

---

## 2. 功能需求

### 2.1 配置文件管理

通过 `config.yaml` 统一配置所有爬虫参数：

| 配置项 | 类型 | 说明 |
|--------|------|------|
| `urls` | list | 目标网站URL列表 |
| `file_extensions` | list | 要爬取的文件扩展名（.pdf, .doc, .md等） |
| `content_format` | string | 内容存储格式：markdown / json / txt |
| `meta_format` | string | 元数据格式（固定为json） |
| `enable_meta` | bool | 是否为每个文件生成元数据 |
| `index_file` | string | 主索引文件名 |
| `output_dir` | string | 输出目录 |
| `delay` | int | 请求间隔（秒） |
| `max_workers` | int | 最大并发数 |
| `recursive` | bool | 是否递归爬取 |
| `max_depth` | int | 最大递归深度 |

### 2.2 内容存储

**主文件**：`{filename}.md`（或配置的格式）

- 存储网页/文档的实际内容
- 采用Markdown格式，便于LLM理解和检索

### 2.3 元数据管理

**元数据文件**：`{filename}.json`（每个内容文件对应一个）

```json
{
  "md5": "a1b2c3d4...",
  "fetch_date": "2026-04-07T10:30:00",
  "source_url": "https://example.com/docs/guide",
  "title": "用户指南",
  "file_size": 12345,
  "content_type": "text/html"
}
```

### 2.4 主索引文件

**文件**：`index.json`（在output目录下）

```json
{
  "last_updated": "2026-04-07T10:30:00",
  "total_files": 150,
  "updated_files": ["file1.md", "file2.md"],
  "new_files": ["file3.md"],
  "deleted_files": [],
  "file_tree": [
    {
      "name": "file1.md",
      "md5": "a1b2c3d4...",
      "source_url": "https://example.com/...",
      "last_modified": "2026-04-07"
    }
  ]
}
```

### 2.5 增量爬取

**核心逻辑**：
1. 爬取前读取 `index.json` 中的文件MD5列表
2. 爬取时计算新文件的MD5
3. 对比判断文件状态：
   - **新增**：MD5不在索引中
   - **修改**：MD5已变化
   - **未变**：MD5相同，跳过存储
4. 爬取后更新 `index.json`

**输出报告**：
- 控制台显示：新增/修改/删除文件数量
- 详细列表输出

---

## 3. 技术架构

### 3.1 技术选型

| 层级 | 技术 | 说明 |
|------|------|------|
| GUI框架 | **Tauri 2.x** | Rust绑定，支持Python后端，体积小(~10MB) |
| 前端 | 原生 HTML/CSS/JS | 轻量级界面 |
| 后端爬虫 | Python | requests, BeautifulSoup, html2text |
| 通信 | Tauri Commands | Rust主进程与前端通信 |
| 打包 | Tauri bundler | 一键打包Windows/Mac/Linux |

### 3.2 项目结构

```
DPCrawler/
├── src/                    # 前端界面
│   ├── index.html
│   ├── styles.css
│   └── app.js
├── src-tauri/              # Tauri配置
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── lib.rs
│       └── main.rs
├── python/                 # Python爬虫模块
│   ├── crawler.py
│   ├── storage.py
│   └── config.py
├── config.yaml             # 爬虫配置
├── pyproject.toml          # Python依赖
└── package.json
```

### 3.3 核心模块

| 模块 | 职责 |
|------|------|
| `src-tauri/src/lib.rs` | Tauri命令、窗口管理、Python进程调用 |
| `crawler.py` | HTTP请求、HTML解析、文件下载、扩展名过滤 |
| `storage.py` | 元数据生成、文件存储、增量检测、索引更新 |

### 3.3 依赖库

**Python端**（`python/requirements.txt`）：
```
requests
beautifulsoup4
pyyaml
html2text
aiohttp (异步并发)
```

**前端**（`package.json`）：
```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.0.0"
  }
}
```

### 3.4 数据流

```
前端界面(index.html) ←→ Tauri Commands ←→ Rust主进程
                                              ↓
                                        Python后端
                                              ↓
                                      文件系统
                                              ↓
                                output/{content, meta, index.json}
```

---

## 4. GUI功能需求

### 4.1 界面布局

```
┌─────────────────────────────────────────────┐
│  Header: Logo + 状态指示器                    │
├───────────────┬─────────────────────────────┤
│  Sidebar      │  Main Content               │
│  - 快速操作    │  - 配置/日志/文件/结果标签页  │
├───────────────┴─────────────────────────────┤
│  Footer: 版本信息 + 操作按钮                   │
└─────────────────────────────────────────────┘
```

### 4.2 配置页面

| 配置项 | 说明 |
|--------|------|
| 目标URLs | 多行输入，每行一个URL |
| 文件扩展名 | 复选框：.html/.md/.txt/.pdf等 |
| 输出目录 | 内容存储路径 |
| 内容格式 | Markdown / JSON / TXT |
| 元数据 | 启用/禁用 |
| 请求间隔 | 秒 |
| 最大并发 | 1-10 |
| 最大深度 | 递归深度 |
| 递归爬取 | 开关 |

### 4.3 核心功能

| 功能 | 说明 |
|------|------|
| 配置编辑 | 可视化编辑所有爬虫参数 |
| 配置保存 | 保存到config.yaml |
| 爬取控制 | 开始/停止爬取 |
| 实时日志 | 显示爬取过程日志 |
| 结果展示 | 新增/修改/删除文件列表 |
| 索引查看 | 显示已爬取文件总数 |

---

## 5. 验收标准

### 5.1 爬虫核心
1. ✅ 配置文件可指定URL列表和文件扩展名过滤
2. ✅ 内容以Markdown格式存储（LLM友好）
3. ✅ 每个文件有对应元数据（MD5、日期、来源URL）
4. ✅ 主索引文件记录所有文件tree信息
5. ✅ 增量爬取正确识别新增/修改/删除文件
6. ✅ 运行结果清晰展示更新状态

### 5.2 跨平台客户端
7. ✅ 使用Tauri打包，可生成Windows/Mac/Linux原生应用
8. ✅ 可视化配置页面，操作便捷
9. ✅ 实时显示爬取日志
10. ✅ 安装包体积控制在15MB以内
