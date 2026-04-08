# DPCrawler 从零开始：部署、开发与打包全指南

本手册旨在指导任何开发者从拉取代码的第一步开始，在 Windows、macOS 或 Linux 上完美运行并打包 DPCrawler。

---

## 第 0 步：获取代码 (Git Clone)

首先，将项目克隆到您的本地机器：
```bash
git clone <项目仓库地址>
cd DPCrawler
```

---

## 第 1 步：安装全局系统环境 (System Setup)

在开始项目配置前，请确保您的操作系统具备以下基础：

1.  **Node.js (v18+)**: [安装包下载](https://nodejs.org/)
2.  **Rust**: [官方一键安装脚本](https://rustup.rs/) (安装后需重启终端)
3.  **Python (3.10+)**: [安装包下载](https://www.python.org/) (Windows 安装时请勾选 "Add to PATH")
4.  **平台工具链**:
    *   **macOS**: 终端执行 `xcode-select --install`。
    *   **Windows**: 安装 [Visual Studio 生成工具](https://visualstudio.microsoft.com/visual-cpp-build-tools/) (勾选 "C++ build tools")。
    *   **Linux**: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`。

---

## 第 2 步：项目初始化 (Initialization)

进入项目根目录后，依次执行以下命令：

### 2.1 安装前端依赖
```bash
npm install
```

### 2.2 创建 Python 虚拟环境 (关键)
项目使用 Sidecar 模式，**必须**在根目录下存在名为 `.venv` 的虚拟环境。

**macOS / Linux:**
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r python/requirements.txt
```

**Windows (PowerShell):**
```powershell
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r python\requirements.txt
```

---

## 第 3 步：进入开发模式 (Development)

在正式打包前，建议先通过开发模式验证环境是否配置成功：
```bash
npm run dev
```
*   **结果**：如果能看到应用窗口弹出，且左上角状态显示“就绪”，说明环境、Sidecar 和前端通信全部正常。

---

## 第 4 步：打包成安装包 (Bundling)

当需要分发软件时，执行打包命令：
```bash
npm run build
```

### 4.1 自动执行的任务
执行该命令后，系统会自动完成以下工作：
1.  自动调用 `scripts/build-sidecar.js`。
2.  自动根据当前系统架构编译 Python 爬虫二进制文件。
3.  自动将 Sidecar 嵌入到安装包中。
4.  自动生成适合当前平台的安装包（`.dmg`, `.msi`, `.deb`）。

### 4.2 安装包位置
*   **macOS**: `src-tauri/target/release/bundle/dmg/DPCrawler.dmg`
*   **Windows**: `src-tauri/target/release/bundle/msi/DPCrawler.msi`
*   **Linux**: `src-tauri/target/release/bundle/deb/dpcrawler.deb`

---

## 5. 打包前必读 (Important Tips)

*   **不要重命名二进制文件**：`src-tauri/binaries/` 下生成的文件包含复杂的“三元组”后缀（如 `-x86_64-apple-darwin`），这是 Tauri 识别侧边栏的唯一标识，请勿修改。
*   **关于图标**：如果想更新图标，只需准备一张 1024x1024 的 PNG，放到根目录命名为 `app-icon.png`，然后运行 `npx tauri icon ./app-icon.png` 即可自动更新全平台图标。
*   **环境变更**：如果您修改了 Python 代码，直接运行 `npm run build` 即可，系统会自动重新编译最新的侧边栏。

---

祝您打包愉快！如有任何环境报错，请检查第 1 步的系统工具链是否安装完整。
