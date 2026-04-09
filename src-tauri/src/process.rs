use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::io::{Read, Write};

pub static CRAWLER_PID: AtomicU32 = AtomicU32::new(0);

/// Helper to kill a process by PID portably
pub fn kill_pid(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .arg("/F")
            .arg("/PID")
            .arg(pid.to_string())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn();
    }
}

/// Helper to check if a process is alive portably
pub fn is_pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let output = Command::new("tasklist")
            .arg("/FI")
            .arg(format!("PID eq {}", pid))
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
        if let Ok(out) = output {
            String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
        } else {
            false
        }
    }
}

/// Disable WebKit cache & persistence to prevent IPC corruption
pub fn disable_webkit_cache() {
    let pid_file = dirs::cache_dir()
        .map(|d| d.join("com.dpcrawler.app").join(".pid"))
        .unwrap_or_else(|| std::env::temp_dir().join("dpcrawler.pid"));
    
    if pid_file.exists() {
        if let Ok(mut f) = std::fs::File::open(&pid_file) {
            let mut buf = String::new();
            if f.read_to_string(&mut buf).is_ok() {
                if let Ok(pid) = buf.trim().parse::<u32>() {
                    if is_pid_alive(pid) {
                        kill_pid(pid);
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            }
        }
    }

    let dirs_to_nuke = [
        dirs::data_local_dir().map(|d| d.join("com.dpcrawler.app")),
        dirs::cache_dir().map(|d| d.join("com.dpcrawler.app")),
    ];
    for path in dirs_to_nuke.into_iter().flatten() {
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }
        let _ = std::fs::create_dir_all(&path);
    }
    
    if let Some(parent) = pid_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::File::create(&pid_file) {
        let _ = f.write_all(std::process::id().to_string().as_bytes());
    }
}
