const { spawn } = require('child_process');
const os = require('os');
const path = require('path');

const isWindows = os.platform() === 'win32';
const scriptName = isWindows ? 'build-sidecar.bat' : 'build-sidecar.sh';
const scriptPath = path.join(__dirname, '..', scriptName);

console.log(`[Build] Running sidecar build script: ${scriptName}`);

const shell = isWindows ? true : '/bin/bash';
const cmd = isWindows ? scriptPath : scriptPath;

const child = spawn(cmd, [], {
  shell: shell,
  stdio: 'inherit',
  env: process.env
});

child.on('exit', (code) => {
  if (code !== 0) {
    console.error(`[Build] Sidecar build failed with exit code ${code}`);
    process.exit(code);
  }
  console.log('[Build] Sidecar build completed successfully');
});
