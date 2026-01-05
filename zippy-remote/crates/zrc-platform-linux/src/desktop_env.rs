#![cfg(target_os = "linux")]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    GNOME,
    KDE,
    XFCE,
    LXDE,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    XWayland,
    Headless,
}

/// Desktop environment detection
pub struct DesktopEnvironmentInfo {
    pub de: DesktopEnvironment,
    pub session_type: SessionType,
}

impl DesktopEnvironmentInfo {
    /// Detect desktop environment
    pub fn detect() -> Self {
        let de = std::env::var("XDG_CURRENT_DESKTOP")
            .or_else(|_| std::env::var("DESKTOP_SESSION"))
            .ok()
            .and_then(|s| {
                let s_lower = s.to_lowercase();
                if s_lower.contains("gnome") {
                    Some(DesktopEnvironment::GNOME)
                } else if s_lower.contains("kde") {
                    Some(DesktopEnvironment::KDE)
                } else if s_lower.contains("xfce") {
                    Some(DesktopEnvironment::XFCE)
                } else if s_lower.contains("lxde") {
                    Some(DesktopEnvironment::LXDE)
                } else {
                    None
                }
            })
            .unwrap_or(DesktopEnvironment::Unknown);

        let session_type = if std::env::var("WAYLAND_DISPLAY").is_ok() {
            if std::env::var("DISPLAY").is_ok() {
                SessionType::XWayland
            } else {
                SessionType::Wayland
            }
        } else if std::env::var("DISPLAY").is_ok() {
            SessionType::X11
        } else {
            SessionType::Headless
        };

        Self {
            de,
            session_type,
        }
    }

    /// Get capabilities based on DE and session
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            can_capture: matches!(self.session_type, SessionType::X11 | SessionType::Wayland | SessionType::XWayland),
            can_inject_input: matches!(self.session_type, SessionType::X11 | SessionType::XWayland),
            requires_portal: matches!(self.session_type, SessionType::Wayland),
            supports_pipewire: matches!(self.de, DesktopEnvironment::GNOME | DesktopEnvironment::KDE),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    pub can_capture: bool,
    pub can_inject_input: bool,
    pub requires_portal: bool,
    pub supports_pipewire: bool,
}
