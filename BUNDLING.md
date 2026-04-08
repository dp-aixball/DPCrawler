# DPCrawler 一键部署与打包指南

本手册旨在帮助您以最简单的方式完成 DPCrawler 的部署与打包。

---

## 🚀 快速开始 (极致简化版)

如果您刚刚拉取了代码，只需执行以下 **两步** 即可：

### 第一步：安装全局基础环境
请确保您的电脑上已经安装了：
1.  **Node.js**: [下载地址](https://nodejs.org/)
2.  **Rust**: [安装脚本](https://rustup.rs/)
3.  **Python 3.10+**: [下载地址](https://www.python.org/)

### 第二步：一键初始化环境
在项目根目录运行：
```bash
npm run setup
```
> [!TIP]
> 这个脚本会自动帮您：安装前端依赖、创建 Python 虚拟环境、同步 Python 库。如果缺少 Rust 或 Xcode/VS 工具，它会给出明确的提示和安装建议。

---

## 🛠 开发与打包流程

### 1. 开发预览
```bash
npm run dev
```

### 2. 正式打包 (生成安装包)
```bash
npm run build
```

---

## 📦 安装包在哪？

打包完成后，您可以在以下目录找到成品：
*   **macOS**: `src-tauri/target/release/bundle/dmg/DPCrawler.dmg`
*   **Windows**: `src-tauri/target/release/bundle/msi/DPCrawler.msi`
*   **Linux**: `src-tauri/target/release/bundle/deb/dpcrawler.deb`

---

## 💡 常见问题与小贴士

*   **Windows 打包报错？**：请确保安装了 [Visual Studio Build Tools (C++)](https://visualstudio.microsoft.com/visual-cpp-build-tools/)。如果您运行过 `npm run setup`，它会检测此项。
*   **图标更新**：双击根目录下的 `app-icon.png` 换成您的图，然后运行 `npx tauri icon ./app-icon.png`。
*   **更新代码**：修改 Python 代码后，直接运行 `npm run build` 即可，系统会自动重新编译。

---

祝您打包顺利！
