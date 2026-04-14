use std::process::Command;

fn main() {
    // 获取 git commit hash (短版本)
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|hash| hash.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 获取 git commit 时间
    let git_date = Command::new("git")
        .args(["log", "-1", "--format=%ci"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|date| date.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // 通过环境变量注入到前端
    println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_hash);
    println!("cargo:rustc-env=GIT_COMMIT_DATE={}", git_date);
    
    tauri_build::build()
}
