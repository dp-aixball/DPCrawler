use egui::Color32;

#[derive(Clone, Copy, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
    Auto,
}

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub glass: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub primary: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub input_bg: Color32,
    pub input_focus: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub resizer: Color32,
}

impl ThemePalette {
    pub fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(15, 23, 42),
            surface: Color32::from_rgb(30, 41, 59),
            surface_alt: Color32::from_rgb(51, 65, 85),
            glass: Color32::from_rgba_premultiplied(100, 116, 139, 128),
            border: Color32::from_rgb(71, 85, 105),
            text: Color32::from_rgb(226, 232, 240),
            muted: Color32::from_rgb(148, 163, 184),
            primary: Color32::from_rgb(59, 130, 246),
            success: Color32::from_rgb(34, 197, 94),
            warning: Color32::from_rgb(251, 146, 60),
            danger: Color32::from_rgb(239, 68, 68),
            input_bg: Color32::from_rgb(30, 41, 59),
            input_focus: Color32::from_rgb(51, 65, 85),
            hover: Color32::from_rgb(71, 85, 105),
            selected: Color32::from_rgba_premultiplied(59, 130, 246, 77),
            resizer: Color32::from_rgb(100, 116, 139),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color32::from_rgb(248, 250, 252),
            surface: Color32::from_rgb(241, 245, 249),
            surface_alt: Color32::from_rgb(226, 232, 240),
            glass: Color32::from_rgba_premultiplied(203, 213, 225, 128),
            border: Color32::from_rgb(203, 213, 225),
            text: Color32::from_rgb(15, 23, 42),
            muted: Color32::from_rgb(100, 116, 139),
            primary: Color32::from_rgb(37, 99, 235),
            success: Color32::from_rgb(22, 163, 74),
            warning: Color32::from_rgb(234, 88, 12),
            danger: Color32::from_rgb(220, 38, 38),
            input_bg: Color32::from_rgb(241, 245, 249),
            input_focus: Color32::from_rgb(226, 232, 240),
            hover: Color32::from_rgb(203, 213, 225),
            selected: Color32::from_rgba_premultiplied(37, 99, 235, 77),
            resizer: Color32::from_rgb(203, 213, 225),
        }
    }

    pub fn get_palette(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
            ThemeMode::Auto => {
                if is_system_dark_mode() {
                    Self::dark()
                } else {
                    Self::light()
                }
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn is_system_dark_mode() -> bool {
    std::env::var("GTK_THEME")
        .map(|t| t.contains("dark"))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn is_system_dark_mode() -> bool {
    std::process::Command::new("defaults")
        .args(&["read", "-g", "AppleInterfaceStyle"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.contains("Dark"))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_system_dark_mode() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r#"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"#)
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|v| v == 0)
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn is_system_dark_mode() -> bool {
    false
}
