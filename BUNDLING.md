# DPCrawler 跨平台打包指南

本文档详细说明了如何在不同操作系统（Windows, macOS, Linux）上开发和打包 DPCrawler。

## 1. 跨平台架构说明

DPCrawler 是一个基于 **Tauri (Rust)** 和 **Python (Sidecar)** 的混合架构应用。为了实现跨平台无缝运行，我们构建了以下自动化机制：

*   **统一构建入口**：通过 `scripts/build-sidecar.js` (Node.js) 自动识别操作系统，选择调用 `build-sidecar.sh` (Unix) 或 `build-sidecar.bat` (Windows)。
*   **便携式 Python 环境**：应用会自动在本地 `.venv` 中寻找 Python 解释器并安装依赖，确保 Sidecar 编译的一致性。
*   **平台适配层 (Rust)**：在 `src-tauri/src/lib.rs` 中使用了条件编译 (`#[cfg(unix/windows)]`)，处理了不同平台的进程管理（信号 vs 任务结束）和文件路径差异。

---

## 2. 环境准备

在所有平台上，您都需要准备以下基础环境：

1.  **Node.js** (v18+)
2.  **Rust & Cargo** (最新稳定版)
3.  **Python 3.10+**
4.  **初始化环境**：
    ```bash
    # 安装前端依赖
    npm install
    
    # 创建并初始化虚拟环境 (Mac/Linux)
    python3 -m venv .venv
    source .venv/bin/activate
    pip install -r python/requirements.txt
    
    # Windows 用户使用:
    # python -m venv .venv
    # .venv\Scripts\activate
    # pip install -r python/requirements.txt
    ```

---

## 3. 打包流程

### macOS (Intel / Apple Silicon)
1.  确保已安装 Xcode Command Line Tools。
2.  运行打包命令：
    ```bash
    npm run build
    ```
3.  **输出结果**：`src-tauri/target/release/bundle/dmg/` 或 `app/`。
4.  **注意**：Sidecar 会被命名为 `crawler-x86_64-apple-darwin` 或 `crawler-aarch64-apple-darwin`。

### Windows (x64)
1.  确保已安装 [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 和 [Visual Studio 生成工具](https://visualstudio.microsoft.com/visual-cpp-build-tools/)。
2.  运行打包命令：
    ```powershell
    npm run build
    ```
3.  **输出结果**：`src-tauri\target\release\bundle\msi\`。
4.  **注意**：Sidecar 会被命名为 `crawler-x86_64-pc-windows-msvc.exe`。

### Linux (Ubuntu/Debian)
1.  安装系统依赖：`sudo apt update && sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`。
2.  运行打包命令：
    ```bash
    npm run build
    ```
3.  **输出结果**：`src-tauri/target/release/bundle/deb/` 或 `appimage/`。
4.  **注意**：Sidecar 会被命名为 `crawler-x86_64-unknown-linux-gnu`。

---

## 4. 常见问题 (Troubleshooting)

### 1. Sidecar 二进制文件丢失
如果报错 `resource path binaries/crawler-... doesn't exist`：
*   手动运行 `node scripts/build-sidecar.js` 强制重新编译。
*   检查 `src-tauri/binaries/` 目录是否生成了对应平台后缀的文件。

### 2. Python 依赖报错
PyInstaller 在打包时可能无法找到某些隐式导入。
*   检查 `python/crawler.spec` 中的 `hiddenimports`。
*   确保在运行打包命令前已激活 `.venv` 并执行了 `pip install`。

### 3. Windows 进程无法停止
如果点击“停止”无效：
*   确保您的终端具有管理员权限（有时处理僵尸进程需要）。
*   检查 `src-tauri/src/lib.rs` 中的 `taskkill` 逻辑是否被杀毒软件拦截。

---

## 5. 跨平台支持状态总结

| 功能特性 | macOS | Windows | Linux | 状态 |
| :--- | :---: | :---: | :---: | :--- |
| **GUI 渲染** | ✅ | ✅ | ✅ | 已适配 |
| **Sidecar 自动编译** | ✅ | ✅ | ✅ | 已适配 |
| **进程管理 (停止功能)** | ✅ | ✅ | ✅ | 已适配 |
| **多核并发爬取** | ✅ | ✅ | ✅ | 已适配 |
| **附件转换 (PDF/Word)** | ✅ | ✅ | ✅ | 已适配 |
| **配置自动保存** | ✅ | ✅ | ✅ | 已适配 |

> [!IMPORTANT]
> 由于 DPCrawler 涉及操作系统底层的进程调度，**请务必在目标平台上进行最终打包**。例如，要在 Windows 上分发软件，必须在 Windows 物理机或虚拟机中运行 `npm run build`。
