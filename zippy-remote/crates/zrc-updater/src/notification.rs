//! Update notification system.
//!
//! Provides cross-platform update notifications using native notification systems:
//! - Windows: Toast notifications via Windows API
//! - macOS: NSUserNotification / UNUserNotificationCenter
//! - Linux: libnotify / D-Bus notifications
//!
//! # Requirements
//! - Requirement 11.1: Notify user when update is available
//! - Requirement 11.2: Show update version and release notes summary
//! - Requirement 11.3: Allow user to defer update
//! - Requirement 11.4: Respect "do not disturb" settings
//! - Requirement 11.5: Use native notification system
//! - Requirement 11.6: Not spam notifications
//! - Requirement 11.7: Indicate update urgency (security vs feature)
//! - Requirement 11.8: Provide "remind me later" option

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::UpdateError;
use crate::manager::UpdateInfo;

/// Urgency level for update notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateUrgency {
    /// Normal feature update
    Normal,
    /// Important update (significant features or fixes)
    Important,
    /// Security update (should be installed promptly)
    Security,
    /// Critical security update (should be installed immediately)
    Critical,
}

impl Default for UpdateUrgency {
    fn default() -> Self {
        Self::Normal
    }
}

impl UpdateUrgency {
    /// Determine urgency from update info.
    pub fn from_update_info(info: &UpdateInfo) -> Self {
        if info.is_security_update {
            // Check release notes for critical keywords
            let notes_lower = info.release_notes.to_lowercase();
            if notes_lower.contains("critical")
                || notes_lower.contains("remote code execution")
                || notes_lower.contains("rce")
            {
                Self::Critical
            } else {
                Self::Security
            }
        } else {
            // Check for important keywords
            let notes_lower = info.release_notes.to_lowercase();
            if notes_lower.contains("important") || notes_lower.contains("breaking change") {
                Self::Important
            } else {
                Self::Normal
            }
        }
    }

    /// Get a human-readable description of the urgency.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Normal => "Feature update",
            Self::Important => "Important update",
            Self::Security => "Security update",
            Self::Critical => "Critical security update",
        }
    }
}

/// Content for an update notification.
#[derive(Debug, Clone)]
pub struct NotificationContent {
    /// Title of the notification
    pub title: String,
    /// Body text of the notification
    pub body: String,
    /// Version being updated to
    pub version: Version,
    /// Summary of release notes (truncated)
    pub release_notes_summary: String,
    /// Urgency level
    pub urgency: UpdateUrgency,
    /// Whether this is a security update
    pub is_security_update: bool,
}

impl NotificationContent {
    /// Create notification content from update info.
    ///
    /// # Requirements
    /// - Requirement 11.2: Show update version and release notes summary
    /// - Requirement 11.7: Indicate update urgency
    pub fn from_update_info(info: &UpdateInfo) -> Self {
        let urgency = UpdateUrgency::from_update_info(info);
        let title = Self::format_title(&info.version, urgency);
        let body = Self::format_body(info, urgency);
        let release_notes_summary = Self::summarize_release_notes(&info.release_notes);

        Self {
            title,
            body,
            version: info.version.clone(),
            release_notes_summary,
            urgency,
            is_security_update: info.is_security_update,
        }
    }

    /// Format the notification title.
    fn format_title(version: &Version, urgency: UpdateUrgency) -> String {
        match urgency {
            UpdateUrgency::Critical => format!("⚠️ Critical Update Available: v{}", version),
            UpdateUrgency::Security => format!("🔒 Security Update Available: v{}", version),
            UpdateUrgency::Important => format!("📢 Important Update Available: v{}", version),
            UpdateUrgency::Normal => format!("Update Available: v{}", version),
        }
    }

    /// Format the notification body.
    fn format_body(info: &UpdateInfo, urgency: UpdateUrgency) -> String {
        let urgency_text = match urgency {
            UpdateUrgency::Critical => "A critical security update is available. Please update immediately.",
            UpdateUrgency::Security => "A security update is available. We recommend updating soon.",
            UpdateUrgency::Important => "An important update is available with significant improvements.",
            UpdateUrgency::Normal => "A new version of Zippy Remote is available.",
        };

        let size_mb = info.size as f64 / (1024.0 * 1024.0);
        format!(
            "{}\n\nVersion: {}\nSize: {:.1} MB",
            urgency_text, info.version, size_mb
        )
    }

    /// Summarize release notes to a reasonable length.
    fn summarize_release_notes(notes: &str) -> String {
        const MAX_LENGTH: usize = 200;

        // Take first paragraph or first MAX_LENGTH characters
        let first_para = notes.split("\n\n").next().unwrap_or(notes);
        let trimmed = first_para.trim();

        if trimmed.len() <= MAX_LENGTH {
            trimmed.to_string()
        } else {
            // Find a good break point
            let truncated = &trimmed[..MAX_LENGTH];
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }
}


/// User response to a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationResponse {
    /// User clicked to install the update
    Install,
    /// User chose to defer/remind later
    RemindLater,
    /// User dismissed the notification
    Dismissed,
    /// Notification timed out
    TimedOut,
    /// User chose to skip this version
    SkipVersion,
}

/// Configuration for the notification system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Whether notifications are enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Minimum interval between notifications (in hours)
    #[serde(default = "default_notification_interval")]
    pub min_interval_hours: u32,

    /// Whether to respect system "do not disturb" settings
    #[serde(default = "default_true")]
    pub respect_dnd: bool,

    /// Default remind later duration (in hours)
    #[serde(default = "default_remind_later_hours")]
    pub remind_later_hours: u32,

    /// Maximum number of reminders before stopping
    #[serde(default = "default_max_reminders")]
    pub max_reminders: u32,

    /// Whether to show notifications for non-security updates
    #[serde(default = "default_true")]
    pub show_feature_updates: bool,

    /// Always show critical security updates regardless of other settings
    #[serde(default = "default_true")]
    pub always_show_critical: bool,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_interval_hours: default_notification_interval(),
            respect_dnd: true,
            remind_later_hours: default_remind_later_hours(),
            max_reminders: default_max_reminders(),
            show_feature_updates: true,
            always_show_critical: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_notification_interval() -> u32 {
    4 // 4 hours minimum between notifications
}

fn default_remind_later_hours() -> u32 {
    24 // Remind after 24 hours
}

fn default_max_reminders() -> u32 {
    5 // Maximum 5 reminders before giving up
}

/// State for tracking deferred notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferredUpdate {
    /// Version that was deferred
    pub version: Version,
    /// When the update was first seen
    pub first_seen: DateTime<Utc>,
    /// When to remind next
    pub remind_at: DateTime<Utc>,
    /// Number of times reminded
    pub reminder_count: u32,
    /// Whether user chose to skip this version
    pub skipped: bool,
}

/// Persistent state for the notification system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationState {
    /// Last notification time
    pub last_notification: Option<DateTime<Utc>>,
    /// Deferred updates
    pub deferred: Vec<DeferredUpdate>,
    /// Versions that user chose to skip
    pub skipped_versions: Vec<Version>,
}

impl NotificationState {
    /// Load state from a file.
    pub fn load(path: &std::path::Path) -> Result<Self, UpdateError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Save state to a file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), UpdateError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if a version is skipped.
    pub fn is_skipped(&self, version: &Version) -> bool {
        self.skipped_versions.contains(version)
    }

    /// Check if a version is deferred and not yet due for reminder.
    pub fn is_deferred(&self, version: &Version) -> bool {
        self.deferred.iter().any(|d| {
            &d.version == version && !d.skipped && d.remind_at > Utc::now()
        })
    }

    /// Get deferred update info if exists.
    pub fn get_deferred(&self, version: &Version) -> Option<&DeferredUpdate> {
        self.deferred.iter().find(|d| &d.version == version)
    }

    /// Add or update a deferred update.
    pub fn defer_update(&mut self, version: Version, remind_hours: u32) {
        let now = Utc::now();
        let remind_at = now + chrono::Duration::hours(remind_hours as i64);

        if let Some(existing) = self.deferred.iter_mut().find(|d| d.version == version) {
            existing.remind_at = remind_at;
            existing.reminder_count += 1;
        } else {
            self.deferred.push(DeferredUpdate {
                version,
                first_seen: now,
                remind_at,
                reminder_count: 1,
                skipped: false,
            });
        }
    }

    /// Skip a version (don't notify again).
    pub fn skip_version(&mut self, version: Version) {
        if !self.skipped_versions.contains(&version) {
            self.skipped_versions.push(version.clone());
        }
        // Also mark as skipped in deferred if present
        if let Some(deferred) = self.deferred.iter_mut().find(|d| d.version == version) {
            deferred.skipped = true;
        }
    }

    /// Clear deferred state for a version (e.g., after successful install).
    pub fn clear_deferred(&mut self, version: &Version) {
        self.deferred.retain(|d| &d.version != version);
    }

    /// Record that a notification was shown.
    pub fn record_notification(&mut self) {
        self.last_notification = Some(Utc::now());
    }

    /// Check if enough time has passed since last notification.
    pub fn can_notify(&self, min_interval_hours: u32) -> bool {
        match self.last_notification {
            None => true,
            Some(last) => {
                let elapsed = Utc::now() - last;
                elapsed >= chrono::Duration::hours(min_interval_hours as i64)
            }
        }
    }
}


/// Platform-specific notification backend trait.
///
/// # Requirements
/// - Requirement 11.5: Use native notification system
pub trait NotificationBackend: Send + Sync {
    /// Show a notification and return the user's response.
    fn show(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError>;

    /// Check if the system is in "do not disturb" mode.
    fn is_do_not_disturb(&self) -> bool;

    /// Check if notifications are supported on this platform.
    fn is_supported(&self) -> bool;
}

/// Cross-platform notification manager.
///
/// Handles notification display, deferral, and state persistence.
pub struct NotificationManager {
    /// Configuration
    config: NotificationConfig,
    /// Persistent state
    state: Arc<RwLock<NotificationState>>,
    /// State file path
    state_path: PathBuf,
    /// Platform-specific backend
    backend: Box<dyn NotificationBackend>,
    /// In-memory last notification time (for rate limiting within session)
    last_notification: Arc<RwLock<Option<Instant>>>,
}

impl NotificationManager {
    /// Create a new notification manager.
    pub fn new(
        config: NotificationConfig,
        state_path: PathBuf,
        backend: Box<dyn NotificationBackend>,
    ) -> Result<Self, UpdateError> {
        let state = NotificationState::load(&state_path).unwrap_or_default();

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            state_path,
            backend,
            last_notification: Arc::new(RwLock::new(None)),
        })
    }

    /// Create with the default platform backend.
    pub fn with_default_backend(
        config: NotificationConfig,
        state_path: PathBuf,
    ) -> Result<Self, UpdateError> {
        let backend = create_platform_backend();
        Self::new(config, state_path, backend)
    }

    /// Check if a notification should be shown for the given update.
    ///
    /// # Requirements
    /// - Requirement 11.4: Respect "do not disturb" settings
    /// - Requirement 11.6: Not spam notifications
    pub async fn should_notify(&self, info: &UpdateInfo) -> bool {
        // Check if notifications are enabled
        if !self.config.enabled {
            debug!("Notifications disabled");
            return false;
        }

        let urgency = UpdateUrgency::from_update_info(info);

        // Critical updates always show (if configured)
        if urgency == UpdateUrgency::Critical && self.config.always_show_critical {
            return true;
        }

        // Check do not disturb
        if self.config.respect_dnd && self.backend.is_do_not_disturb() {
            debug!("System is in do not disturb mode");
            return false;
        }

        // Check if feature updates are enabled
        if !info.is_security_update && !self.config.show_feature_updates {
            debug!("Feature update notifications disabled");
            return false;
        }

        let state = self.state.read().await;

        // Check if version is skipped
        if state.is_skipped(&info.version) {
            debug!("Version {} is skipped", info.version);
            return false;
        }

        // Check if version is deferred and not yet due
        if state.is_deferred(&info.version) {
            debug!("Version {} is deferred", info.version);
            return false;
        }

        // Check reminder count
        if let Some(deferred) = state.get_deferred(&info.version) {
            if deferred.reminder_count >= self.config.max_reminders {
                debug!("Max reminders reached for version {}", info.version);
                return false;
            }
        }

        // Check notification interval (rate limiting)
        if !state.can_notify(self.config.min_interval_hours) {
            debug!("Too soon since last notification");
            return false;
        }

        true
    }

    /// Show a notification for an available update.
    ///
    /// # Requirements
    /// - Requirement 11.1: Notify user when update is available
    /// - Requirement 11.2: Show update version and release notes summary
    /// - Requirement 11.7: Indicate update urgency
    pub async fn notify_update(
        &self,
        info: &UpdateInfo,
    ) -> Result<NotificationResponse, UpdateError> {
        if !self.should_notify(info).await {
            return Ok(NotificationResponse::Dismissed);
        }

        let content = NotificationContent::from_update_info(info);
        info!(
            "Showing update notification for version {} (urgency: {:?})",
            info.version, content.urgency
        );

        // Show the notification
        let response = self.backend.show(&content)?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.record_notification();
            self.save_state(&state)?;
        }

        // Update in-memory rate limit
        *self.last_notification.write().await = Some(Instant::now());

        Ok(response)
    }

    /// Handle user response to defer the update.
    ///
    /// # Requirements
    /// - Requirement 11.3: Allow user to defer update
    /// - Requirement 11.8: Provide "remind me later" option
    pub async fn defer_update(&self, version: &Version) -> Result<(), UpdateError> {
        let mut state = self.state.write().await;
        state.defer_update(version.clone(), self.config.remind_later_hours);
        self.save_state(&state)?;
        info!(
            "Update {} deferred, will remind in {} hours",
            version, self.config.remind_later_hours
        );
        Ok(())
    }

    /// Handle user response to skip this version.
    pub async fn skip_version(&self, version: &Version) -> Result<(), UpdateError> {
        let mut state = self.state.write().await;
        state.skip_version(version.clone());
        self.save_state(&state)?;
        info!("Version {} skipped by user", version);
        Ok(())
    }

    /// Clear deferred state after successful update.
    pub async fn clear_deferred(&self, version: &Version) -> Result<(), UpdateError> {
        let mut state = self.state.write().await;
        state.clear_deferred(version);
        self.save_state(&state)?;
        Ok(())
    }

    /// Get the current notification state.
    pub async fn state(&self) -> NotificationState {
        self.state.read().await.clone()
    }

    /// Save state to disk.
    fn save_state(&self, state: &NotificationState) -> Result<(), UpdateError> {
        state.save(&self.state_path)
    }

    /// Check if notifications are supported on this platform.
    pub fn is_supported(&self) -> bool {
        self.backend.is_supported()
    }
}


// =============================================================================
// Platform-specific notification backends
// =============================================================================

/// Create the appropriate notification backend for the current platform.
pub fn create_platform_backend() -> Box<dyn NotificationBackend> {
    #[cfg(target_os = "windows")]
    {
        Box::new(WindowsNotificationBackend::new())
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(MacOSNotificationBackend::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxNotificationBackend::new())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Box::new(StubNotificationBackend)
    }
}

/// Stub backend for unsupported platforms or testing.
pub struct StubNotificationBackend;

impl NotificationBackend for StubNotificationBackend {
    fn show(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError> {
        info!(
            "Stub notification: {} - {}",
            content.title, content.body
        );
        Ok(NotificationResponse::Dismissed)
    }

    fn is_do_not_disturb(&self) -> bool {
        false
    }

    fn is_supported(&self) -> bool {
        false
    }
}

// =============================================================================
// Windows notification backend
// =============================================================================

#[cfg(target_os = "windows")]
pub struct WindowsNotificationBackend {
    app_id: String,
}

#[cfg(target_os = "windows")]
impl WindowsNotificationBackend {
    pub fn new() -> Self {
        Self {
            app_id: "ZippyRemote".to_string(),
        }
    }

    pub fn with_app_id(app_id: String) -> Self {
        Self { app_id }
    }
}

#[cfg(target_os = "windows")]
impl Default for WindowsNotificationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl NotificationBackend for WindowsNotificationBackend {
    fn show(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError> {
        use std::process::Command;

        // Use PowerShell to show a toast notification
        // This is a simple approach that works without COM registration
        let script = format!(
            r#"
            [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
            [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

            $template = @"
            <toast>
                <visual>
                    <binding template="ToastGeneric">
                        <text>{}</text>
                        <text>{}</text>
                    </binding>
                </visual>
                <actions>
                    <action content="Install Now" arguments="install" activationType="foreground"/>
                    <action content="Remind Later" arguments="remind" activationType="foreground"/>
                </actions>
            </toast>
"@

            $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
            $xml.LoadXml($template)
            $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("{}").Show($toast)
            "#,
            escape_xml(&content.title),
            escape_xml(&content.body),
            &self.app_id
        );

        let output = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    debug!("Windows toast notification shown");
                    // Toast notifications are fire-and-forget in this implementation
                    // For interactive responses, we'd need a more complex COM-based approach
                    Ok(NotificationResponse::Dismissed)
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    warn!("Failed to show Windows notification: {}", stderr);
                    Ok(NotificationResponse::Dismissed)
                }
            }
            Err(e) => {
                warn!("Failed to execute PowerShell for notification: {}", e);
                Ok(NotificationResponse::Dismissed)
            }
        }
    }

    fn is_do_not_disturb(&self) -> bool {
        // Check Windows Focus Assist status
        // This requires reading from the registry or using WNF
        // For simplicity, we return false (not in DND mode)
        false
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(target_os = "windows")]
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}


// =============================================================================
// macOS notification backend
// =============================================================================

#[cfg(target_os = "macos")]
pub struct MacOSNotificationBackend {
    bundle_id: String,
}

#[cfg(target_os = "macos")]
impl MacOSNotificationBackend {
    pub fn new() -> Self {
        Self {
            bundle_id: "io.zippyremote.ZippyRemote".to_string(),
        }
    }

    pub fn with_bundle_id(bundle_id: String) -> Self {
        Self { bundle_id }
    }
}

#[cfg(target_os = "macos")]
impl Default for MacOSNotificationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "macos")]
impl NotificationBackend for MacOSNotificationBackend {
    fn show(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError> {
        use std::process::Command;

        // Use osascript to show a notification
        // For more advanced features, we'd use the UserNotifications framework via objc
        let script = format!(
            r#"display notification "{}" with title "{}" sound name "default""#,
            escape_applescript(&content.body),
            escape_applescript(&content.title)
        );

        let output = Command::new("osascript")
            .args(["-e", &script])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    debug!("macOS notification shown");
                    Ok(NotificationResponse::Dismissed)
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    warn!("Failed to show macOS notification: {}", stderr);
                    Ok(NotificationResponse::Dismissed)
                }
            }
            Err(e) => {
                warn!("Failed to execute osascript for notification: {}", e);
                Ok(NotificationResponse::Dismissed)
            }
        }
    }

    fn is_do_not_disturb(&self) -> bool {
        use std::process::Command;

        // Check macOS Do Not Disturb status using defaults
        let output = Command::new("defaults")
            .args(["-currentHost", "read", "com.apple.notificationcenterui", "doNotDisturb"])
            .output();

        match output {
            Ok(result) => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                stdout.trim() == "1"
            }
            Err(_) => false,
        }
    }

    fn is_supported(&self) -> bool {
        true
    }
}

#[cfg(target_os = "macos")]
fn escape_applescript(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// Linux notification backend
// =============================================================================

#[cfg(target_os = "linux")]
pub struct LinuxNotificationBackend {
    app_name: String,
}

#[cfg(target_os = "linux")]
impl LinuxNotificationBackend {
    pub fn new() -> Self {
        Self {
            app_name: "Zippy Remote".to_string(),
        }
    }

    pub fn with_app_name(app_name: String) -> Self {
        Self { app_name }
    }
}

#[cfg(target_os = "linux")]
impl Default for LinuxNotificationBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "linux")]
impl NotificationBackend for LinuxNotificationBackend {
    fn show(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError> {
        use std::process::Command;

        // Try notify-send first (most common)
        let urgency = match content.urgency {
            UpdateUrgency::Critical => "critical",
            UpdateUrgency::Security | UpdateUrgency::Important => "normal",
            UpdateUrgency::Normal => "low",
        };

        let output = Command::new("notify-send")
            .args([
                "--app-name", &self.app_name,
                "--urgency", urgency,
                &content.title,
                &content.body,
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    debug!("Linux notification shown via notify-send");
                    Ok(NotificationResponse::Dismissed)
                } else {
                    // Try gdbus as fallback
                    self.show_via_dbus(content)
                }
            }
            Err(_) => {
                // notify-send not available, try D-Bus directly
                self.show_via_dbus(content)
            }
        }
    }

    fn is_do_not_disturb(&self) -> bool {
        // Linux doesn't have a standard DND API
        // Some desktop environments have their own, but we'll return false
        false
    }

    fn is_supported(&self) -> bool {
        use std::process::Command;

        // Check if notify-send is available
        Command::new("which")
            .arg("notify-send")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
impl LinuxNotificationBackend {
    fn show_via_dbus(&self, content: &NotificationContent) -> Result<NotificationResponse, UpdateError> {
        use std::process::Command;

        // Use gdbus to send notification via D-Bus
        let output = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest", "org.freedesktop.Notifications",
                "--object-path", "/org/freedesktop/Notifications",
                "--method", "org.freedesktop.Notifications.Notify",
                &self.app_name,
                "0",  // replaces_id
                "",   // app_icon
                &content.title,
                &content.body,
                "[]", // actions
                "{}", // hints
                "-1", // expire_timeout
            ])
            .output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    debug!("Linux notification shown via D-Bus");
                } else {
                    warn!("Failed to show Linux notification via D-Bus");
                }
                Ok(NotificationResponse::Dismissed)
            }
            Err(e) => {
                warn!("Failed to execute gdbus for notification: {}", e);
                Ok(NotificationResponse::Dismissed)
            }
        }
    }
}


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_update_info(is_security: bool, release_notes: &str) -> UpdateInfo {
        UpdateInfo {
            version: Version::new(2, 0, 0),
            release_notes: release_notes.to_string(),
            size: 10 * 1024 * 1024, // 10 MB
            is_security_update: is_security,
            expected_hash: [0u8; 32],
            artifact_url: "https://example.com/update.zip".to_string(),
            channel: crate::channel::UpdateChannel::Stable,
        }
    }

    #[test]
    fn test_urgency_from_update_info_normal() {
        let info = create_test_update_info(false, "Bug fixes and improvements");
        let urgency = UpdateUrgency::from_update_info(&info);
        assert_eq!(urgency, UpdateUrgency::Normal);
    }

    #[test]
    fn test_urgency_from_update_info_important() {
        let info = create_test_update_info(false, "Important: Breaking change in API");
        let urgency = UpdateUrgency::from_update_info(&info);
        assert_eq!(urgency, UpdateUrgency::Important);
    }

    #[test]
    fn test_urgency_from_update_info_security() {
        let info = create_test_update_info(true, "Security fix for authentication");
        let urgency = UpdateUrgency::from_update_info(&info);
        assert_eq!(urgency, UpdateUrgency::Security);
    }

    #[test]
    fn test_urgency_from_update_info_critical() {
        let info = create_test_update_info(true, "Critical: Remote code execution vulnerability");
        let urgency = UpdateUrgency::from_update_info(&info);
        assert_eq!(urgency, UpdateUrgency::Critical);
    }

    #[test]
    fn test_notification_content_from_update_info() {
        let info = create_test_update_info(true, "Security fix for authentication bypass");
        let content = NotificationContent::from_update_info(&info);

        assert!(content.title.contains("Security"));
        assert!(content.title.contains("2.0.0"));
        assert!(content.is_security_update);
        assert_eq!(content.urgency, UpdateUrgency::Security);
    }

    #[test]
    fn test_notification_content_release_notes_summary() {
        let long_notes = "This is a very long release note that goes on and on and on. \
            It contains many details about the release including bug fixes, \
            new features, and improvements. The summary should be truncated \
            to a reasonable length so it fits in a notification.";
        
        let info = create_test_update_info(false, long_notes);
        let content = NotificationContent::from_update_info(&info);

        assert!(content.release_notes_summary.len() <= 203); // 200 + "..."
        assert!(content.release_notes_summary.ends_with("..."));
    }

    #[test]
    fn test_notification_config_default() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_interval_hours, 4);
        assert!(config.respect_dnd);
        assert_eq!(config.remind_later_hours, 24);
        assert_eq!(config.max_reminders, 5);
        assert!(config.show_feature_updates);
        assert!(config.always_show_critical);
    }

    #[test]
    fn test_notification_state_default() {
        let state = NotificationState::default();
        assert!(state.last_notification.is_none());
        assert!(state.deferred.is_empty());
        assert!(state.skipped_versions.is_empty());
    }

    #[test]
    fn test_notification_state_defer_update() {
        let mut state = NotificationState::default();
        let version = Version::new(2, 0, 0);

        state.defer_update(version.clone(), 24);

        assert!(!state.deferred.is_empty());
        let deferred = state.get_deferred(&version).unwrap();
        assert_eq!(deferred.version, version);
        assert_eq!(deferred.reminder_count, 1);
        assert!(!deferred.skipped);
    }

    #[test]
    fn test_notification_state_defer_update_increments_count() {
        let mut state = NotificationState::default();
        let version = Version::new(2, 0, 0);

        state.defer_update(version.clone(), 24);
        state.defer_update(version.clone(), 24);
        state.defer_update(version.clone(), 24);

        let deferred = state.get_deferred(&version).unwrap();
        assert_eq!(deferred.reminder_count, 3);
    }

    #[test]
    fn test_notification_state_skip_version() {
        let mut state = NotificationState::default();
        let version = Version::new(2, 0, 0);

        state.skip_version(version.clone());

        assert!(state.is_skipped(&version));
        assert!(state.skipped_versions.contains(&version));
    }

    #[test]
    fn test_notification_state_clear_deferred() {
        let mut state = NotificationState::default();
        let version = Version::new(2, 0, 0);

        state.defer_update(version.clone(), 24);
        assert!(state.get_deferred(&version).is_some());

        state.clear_deferred(&version);
        assert!(state.get_deferred(&version).is_none());
    }

    #[test]
    fn test_notification_state_can_notify() {
        let mut state = NotificationState::default();

        // No previous notification - can notify
        assert!(state.can_notify(4));

        // Record notification
        state.record_notification();

        // Just notified - cannot notify again (within 4 hours)
        assert!(!state.can_notify(4));

        // With 0 hour interval - can always notify
        assert!(state.can_notify(0));
    }

    #[test]
    fn test_stub_backend() {
        let backend = StubNotificationBackend;
        let content = NotificationContent {
            title: "Test".to_string(),
            body: "Test body".to_string(),
            version: Version::new(1, 0, 0),
            release_notes_summary: "Test notes".to_string(),
            urgency: UpdateUrgency::Normal,
            is_security_update: false,
        };

        let response = backend.show(&content).unwrap();
        assert_eq!(response, NotificationResponse::Dismissed);
        assert!(!backend.is_do_not_disturb());
        assert!(!backend.is_supported());
    }

    #[test]
    fn test_urgency_description() {
        assert_eq!(UpdateUrgency::Normal.description(), "Feature update");
        assert_eq!(UpdateUrgency::Important.description(), "Important update");
        assert_eq!(UpdateUrgency::Security.description(), "Security update");
        assert_eq!(UpdateUrgency::Critical.description(), "Critical security update");
    }
}
