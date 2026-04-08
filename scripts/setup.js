const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const isWindows = process.platform === 'win32';
const isMac = process.platform === 'darwin';

const colors = {
    reset: "\x1b[0m",
    red: "\x1b[31m",
    green: "\x1b[32m",
    yellow: "\x1b[33m",
    blue: "\x1b[34m",
    cyan: "\x1b[36m",
    bold: "\x1b[1m"
};

function log(msg, color = colors.reset) {
    console.log(`${color}${msg}${colors.reset}`);
}

function checkCommand(cmd, args = ['--version']) {
    try {
        const result = spawnSync(cmd, args, { encoding: 'utf8' });
        return result.status === 0 ? result.stdout.split('\n')[0].trim() : null;
    } catch (e) {
        return null;
    }
}

async function runSetup() {
    log("\n=== DPCrawler 环境自检与自动初始化 ===\n", colors.bold + colors.cyan);

    let allOk = true;

    // 1. Node.js check
    const nodeVer = checkCommand('node');
    if (nodeVer) {
        log(`[OK] Node.js: ${nodeVer}`, colors.green);
    } else {
        log(`[错误] 未检测到 Node.js，请前往 https://nodejs.org/ 安装`, colors.red);
        allOk = false;
    }

    // 2. Rust check
    const rustVer = checkCommand('cargo');
    if (rustVer) {
        log(`[OK] Rust/Cargo: ${rustVer}`, colors.green);
    } else {
        log(`[注意] 未检测到 Rust 环境！`, colors.yellow);
        log(`      请执行: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, colors.yellow);
        allOk = false;
    }

    // 3. Platform Tools check
    if (isMac) {
        const xcode = checkCommand('xcode-select', ['-p']);
        if (xcode) {
            log(`[OK] macOS Build Tools (Xcode) 已就绪`, colors.green);
        } else {
            log(`[注意] 未检测到苹果开发工具！请执行: xcode-select --install`, colors.yellow);
            allOk = false;
        }
    } else if (isWindows) {
        // Simple check for cl.exe in path
        const cl = checkCommand('cl');
        if (cl) {
            log(`[OK] Windows MSVC 编译器已并在 PATH 中`, colors.green);
        } else {
            log(`[注意] 未检测到 Windows C++ Build Tools！`, colors.yellow);
            log(`      打包需要安装 Visual Studio Build Tools (C++)`, colors.yellow);
            // Don't mark allOk=false yet, as sometimes it's not in PATH but tauri finds it
        }
    }

    // 4. NPM Install
    if (!fs.existsSync(path.join(__dirname, '..', 'node_modules'))) {
        log("\n正在安装前端依赖 (npm install)...", colors.blue);
        try {
            execSync('npm install', { stdio: 'inherit', cwd: path.join(__dirname, '..') });
            log("[OK] 前端依赖安装完成", colors.green);
        } catch (e) {
            log("[错误] npm install 失败", colors.red);
            allOk = false;
        }
    } else {
        log("[OK] node_modules 已存在", colors.green);
    }

    // 5. Python & .venv Setup
    const pythonCmd = isWindows ? 'python' : 'python3';
    const pyVer = checkCommand(pythonCmd);
    if (pyVer) {
        log(`[OK] Python: ${pyVer}`, colors.green);
        const venvPath = path.join(__dirname, '..', '.venv');
        if (!fs.existsSync(venvPath)) {
            log("\n正在创建 Python 虚拟环境 (.venv)...", colors.blue);
            try {
                execSync(`${pythonCmd} -m venv .venv`, { stdio: 'inherit', cwd: path.join(__dirname, '..') });
                log("[OK] 虚拟环境创建成功", colors.green);
            } catch (e) {
                log("[错误] 创建虚拟环境失败", colors.red);
                allOk = false;
            }
        } else {
            log("[OK] .venv 虚拟环境已存在", colors.green);
        }

        // Install requirements
        log("\n正在同步 Python 依赖库 (pip install)...", colors.blue);
        const pipCmd = isWindows 
            ? path.join(venvPath, 'Scripts', 'pip.exe')
            : path.join(venvPath, 'bin', 'pip');
        
        try {
            execSync(`"${pipCmd}" install -r python/requirements.txt`, { stdio: 'inherit', cwd: path.join(__dirname, '..') });
            log("[OK] Python 依赖同步完成", colors.green);
        } catch (e) {
            log("[错误] pip install 失败", colors.red);
            allOk = false;
        }
    } else {
        log(`[错误] 未检测到 Python，请前往 https://www.python.org/ 安装`, colors.red);
        allOk = false;
    }

    log("\n-------------------------------------------", colors.cyan);
    if (allOk) {
        log("恭喜！环境配置已完成。您可以运行以下命令启动项目：", colors.bold + colors.green);
        log("npm run dev", colors.yellow);
    } else {
        log("环境配置尚不完整，请根据上方的 [注意] 或 [错误] 提示进行修复。", colors.bold + colors.yellow);
    }
    log("-------------------------------------------\n", colors.cyan);
}

runSetup();
