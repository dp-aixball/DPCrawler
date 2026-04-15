# DPCrawler GUI - 模块化代码结构

## 📁 文件组织

```
src/
├── main.rs              (92 行) - 应用入口、窗口配置、字体加载
├── lib.rs               (5 行) - 公共库导出接口
├── app.rs               (291 行) - 核心数据结构和业务逻辑
│   ├── DPCrawlerDemo - 应用状态结构体
│   ├── RunMode - 运行模式枚举
│   ├── eframe::App impl - 主更新循环
│   └── 公共方法：begin_crawl、stop_current_task、clear_results 等
├── components.rs        (905 行) - UI 渲染组件
│   └── DPCrawlerDemo impl - 所有 render_* 和 select_popup_* 函数
└── theme.rs             (115 行) - 主题系统
    ├── ThemeMode - 主题模式(Auto/Dark/Light)
    ├── ThemePalette - 调色板
    └── get_palette() - 主题切换逻辑
```

## 🎯 模块职责

### main.rs - 应用入口
- 窗口初始化和配置
- 字体加载（中文字体支持）
- 应用启动点

### app.rs - 核心应用
- **数据结构**：UI状态、爬虫参数、进度、日志、文件列表等
- **业务逻辑**：crawl/pre-crawl/stop/clear 等核心操作
- **App trait**：实现 eframe::App 的 update() 方法
- **Helper 方法**：visible_files()、apply_active_site_to_form() 等

### components.rs - UI 渲染
- **侧边栏**：render_sidebar() - 目标URL、扩展名、参数设置等
- **顶部栏**：render_topbar() - 控制按钮（开始/停止/清空等）
- **进度条**：render_progress() - 进度显示
- **标签页**：render_tabs()、render_logs()、render_files()、render_results()
- **表单组件**：select_popup_string()、select_popup_theme()
- **文件预览**：render_markdown_preview() - Markdown 渲染

### theme.rs - 主题系统
- **颜色定义**：Dark/Light 两套完整调色板
- **自动检测**：系统主题自动切换
- **颜色属性**：背景、表面、边框、文字、状态色等 16 个颜色变量

## 🔄 数据流

```
main.rs (启动)
    ↓
app.rs::update()
    ├─ 获取当前主题 → theme.rs::get_palette()
    ├─ 侧边栏 → components.rs::render_sidebar()
    ├─ 中心内容
    │  ├─ 进度条 → components.rs::render_progress()
    │  ├─ 标签页 → components.rs::render_tabs()
    │  └─ 标签内容
    │     ├─ 日志 → render_logs()
    │     ├─ 文件 → render_files()
    │     └─ 结果 → render_results()
    └─ 业务操作（app.rs 中的 begin_crawl、stop_current_task 等）
```

## 📝 编译和运行

```bash
# 编译检查
cargo check

# 编译和运行
cargo run

# 发布版本
cargo build --release
```

## ✨ 模块化优势

| 方面 | 优势 |
|------|------|
| **可维护性** | 每个文件职责明确，易于定位和修改功能 |
| **可扩展性** | 添加新功能时可独立扩展相应模块 |
| **可测试性** | UI 逻辑与业务逻辑分离，便于单元测试 |
| **代码复用** | 模块可独立导入使用，支持lib模式 |
| **编译效率** | 文件变化影响范围更小，增量编译更快 |

## 🔧 添加新功能指南

### 1. 新增 UI 控件
- 在 `components.rs` 中添加新的 render_* 方法

### 2. 新增业务逻辑
- 在 `app.rs` 中添加新方法（如 new_function()）
- 在 `components.rs` 中调用

### 3. 新增状态字段
- 在 `app.rs` 的 `DPCrawlerDemo` 结构体中添加
- 在 `Default` impl 中初始化

### 4. 新增主题颜色
- 在 `theme.rs` 的 `ThemePalette` 中添加
- 在 `dark()` 和 `light()` 中定义具体值

## 📦 依赖

- `eframe` - 应用框架
- `egui` - UI 库
- 其他：由 eframe 间接依赖（winit、glow 等）

## 🚀 下一步

- [ ] 集成实际的爬虫 backend
- [ ] 实现异步任务处理（tokio）
- [ ] 添加配置文件持久化
- [ ] 实现网络请求 mock
- [ ] 编写单元测试
