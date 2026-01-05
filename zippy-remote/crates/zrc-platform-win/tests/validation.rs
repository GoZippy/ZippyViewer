#![cfg(windows)]

use zrc_core::platform::HostPlatform;

/// Validation tests for zrc-platform-win components
/// These tests validate that each component can be instantiated and basic operations work

#[test]
fn test_gdi_capturer_creation() {
    let capturer = zrc_platform_win::capture_gdi::GdiCapturer::new();
    assert!(capturer.is_ok(), "GDI capturer should be creatable");
}

#[test]
fn test_gdi_capture_frame() {
    let mut capturer = zrc_platform_win::capture_gdi::GdiCapturer::new().unwrap();
    let frame = capturer.capture_frame();
    assert!(frame.is_ok(), "GDI capture_frame should work");
    
    let frame = frame.unwrap();
    assert!(frame.width > 0, "Frame should have width > 0");
    assert!(frame.height > 0, "Frame should have height > 0");
    assert!(!frame.bgra.is_empty(), "Frame should have pixel data");
}

#[test]
fn test_dxgi_availability() {
    let available = zrc_platform_win::capture_dxgi::DxgiCapturer::is_available();
    // DXGI may or may not be available depending on system
    println!("DXGI available: {}", available);
}

#[test]
fn test_win_capturer_creation() {
    let capturer = zrc_platform_win::capturer::WinCapturer::new();
    assert!(capturer.is_ok(), "WinCapturer should be creatable");
}

#[test]
fn test_win_capturer_capture() {
    let mut capturer = zrc_platform_win::capturer::WinCapturer::new().unwrap();
    let frame = capturer.capture_frame();
    assert!(frame.is_ok(), "WinCapturer capture_frame should work");
}

#[test]
fn test_win_injector_creation() {
    let injector = zrc_platform_win::injector::WinInjector::new();
    // Should always succeed
    assert!(injector.is_elevated() || !injector.is_elevated()); // Just check it doesn't panic
}

#[test]
fn test_monitor_manager_creation() {
    let manager = zrc_platform_win::monitor::MonitorManager::new();
    assert!(manager.is_ok(), "MonitorManager should be creatable");
}

#[test]
fn test_monitor_enumeration() {
    let manager = zrc_platform_win::monitor::MonitorManager::new().unwrap();
    let monitors = manager.monitors();
    assert!(!monitors.is_empty(), "Should have at least one monitor");
    
    let primary = manager.primary_monitor();
    assert!(primary.is_some(), "Should have a primary monitor");
}

#[test]
fn test_system_info_collection() {
    let info = zrc_platform_win::system_info::SystemInfo::collect();
    assert!(!info.windows_version.is_empty(), "Should have Windows version");
    assert!(!info.computer_name.is_empty(), "Should have computer name");
    assert!(!info.user_name.is_empty(), "Should have user name");
}

#[test]
fn test_display_config() {
    let info = zrc_platform_win::system_info::SystemInfo::collect();
    let config = info.display_config();
    assert!(config.monitor_count > 0, "Should have at least one monitor");
}

#[test]
fn test_network_adapters() {
    let info = zrc_platform_win::system_info::SystemInfo::collect();
    let adapters = info.network_adapters();
    // May be empty, but should not panic
    println!("Found {} network adapters", adapters.len());
}

#[test]
fn test_dpapi_keystore() {
    let keystore = zrc_platform_win::keystore::DpapiKeyStore::new(
        zrc_platform_win::keystore::DpapiScope::CurrentUser
    );
    
    // Test store and load
    let test_key = b"test_key_data";
    let result = keystore.store_key("test_key", test_key);
    assert!(result.is_ok(), "Should be able to store key");
    
    let loaded = keystore.load_key("test_key");
    assert!(loaded.is_ok(), "Should be able to load key");
    
    let loaded_key = loaded.unwrap();
    assert_eq!(loaded_key.as_bytes(), test_key, "Loaded key should match stored key");
    
    // Cleanup
    let _ = keystore.delete_key("test_key");
}

#[test]
fn test_clipboard() {
    let clipboard = zrc_platform_win::clipboard::WinClipboard::new();
    assert!(clipboard.is_ok(), "WinClipboard should be creatable");
    
    let clipboard = clipboard.unwrap();
    let seq = clipboard.sequence_number();
    println!("Clipboard sequence number: {}", seq);
}

#[test]
fn test_uac_handler() {
    let handler = zrc_platform_win::uac::UacHandler::new();
    let is_secure = handler.is_secure_desktop();
    println!("Is secure desktop: {}", is_secure);
    
    let desktop_name = handler.current_desktop_name();
    println!("Current desktop: {}", desktop_name);
}

#[test]
fn test_win_platform_creation() {
    let platform = zrc_platform_win::WinPlatform::new();
    assert!(platform.is_ok(), "WinPlatform should be creatable");
}

#[test]
fn test_special_key_handler() {
    let injector = std::sync::Arc::new(zrc_platform_win::injector::WinInjector::new());
    let handler = zrc_platform_win::special_keys::SpecialKeyHandler::new(injector);
    
    // Test Alt+Tab (should work without elevation)
    let result = handler.send_alt_tab();
    // May fail if not in right context, but should not panic
    println!("Alt+Tab result: {:?}", result);
}

#[test]
fn test_win_capturer_monitor_selection() {
    let mut capturer = zrc_platform_win::capturer::WinCapturer::new().unwrap();
    let monitors = capturer.list_monitors();
    
    if monitors.len() > 1 {
        // Test selecting a non-primary monitor
        let result = capturer.select_monitor(1);
        assert!(result.is_ok(), "Should be able to select monitor by index");
    }
}

#[test]
fn test_win_capturer_display_change() {
    let mut capturer = zrc_platform_win::capturer::WinCapturer::new().unwrap();
    let result = capturer.handle_display_change();
    // Should not panic, may succeed or fail depending on actual display state
    println!("Display change handling result: {:?}", result);
}

#[test]
fn test_injector_coordinate_mapping() {
    use zrc_platform_win::injector::CoordinateMapper;
    
    let mapper = CoordinateMapper::new();
    
    // Test various coordinate mappings
    let test_cases = vec![
        (0, 0),
        (100, 100),
        (1920, 1080),
        (i32::MAX, i32::MAX),
        (-100, -100),
    ];
    
    for (x, y) in test_cases {
        let (abs_x, abs_y) = mapper.to_absolute(x, y);
        assert!(abs_x >= 0 && abs_x <= 65535, "Absolute X should be in valid range");
        assert!(abs_y >= 0 && abs_y <= 65535, "Absolute Y should be in valid range");
    }
}

#[test]
fn test_injector_key_tracking() {
    let mut injector = zrc_platform_win::injector::WinInjector::new();
    
    // Inject some keys and release them
    let _ = injector.inject_key(0x41, true); // 'A'
    let _ = injector.inject_key(0x41, false); // Release 'A'
    
    // Test release all keys
    let result = injector.release_all_keys();
    assert!(result.is_ok(), "Release all keys should succeed");
}

#[test]
fn test_monitor_manager_refresh() {
    let mut manager = zrc_platform_win::monitor::MonitorManager::new().unwrap();
    let initial_count = manager.monitors().len();
    
    // Refresh should not fail
    let result = manager.refresh();
    assert!(result.is_ok(), "Refresh should succeed");
    
    // Should still have monitors after refresh
    assert_eq!(manager.monitors().len(), initial_count, "Monitor count should be stable");
}

#[test]
fn test_monitor_manager_get_by_handle() {
    let manager = zrc_platform_win::monitor::MonitorManager::new().unwrap();
    let monitors = manager.monitors();
    
    if !monitors.is_empty() {
        let monitor = &monitors[0];
        let found = manager.get_monitor_by_handle(monitor.handle);
        assert!(found.is_some(), "Should find monitor by handle");
        assert_eq!(found.unwrap().handle, monitor.handle, "Should return same monitor");
    }
}

#[test]
fn test_dpapi_keystore_entropy() {
    use zrc_platform_win::keystore::{DpapiKeyStore, DpapiScope};
    
    let keystore = DpapiKeyStore::new(DpapiScope::CurrentUser)
        .with_entropy(b"test_entropy");
    
    let test_key = b"key_with_entropy";
    let result = keystore.store_key("entropy_test", test_key);
    assert!(result.is_ok(), "Should be able to store key with entropy");
    
    // Should be able to load with same entropy
    let loaded = keystore.load_key("entropy_test");
    assert!(loaded.is_ok(), "Should be able to load key with entropy");
    assert_eq!(loaded.unwrap().as_bytes(), test_key, "Loaded key should match");
    
    // Cleanup
    let _ = keystore.delete_key("entropy_test");
}

#[test]
fn test_clipboard_text_roundtrip() {
    let clipboard = zrc_platform_win::clipboard::WinClipboard::new().unwrap();
    
    let test_text = "ZRC Clipboard Test";
    let result = clipboard.write_text(test_text);
    
    if result.is_ok() {
        // Small delay to ensure clipboard is updated
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let read = clipboard.read_text();
        assert!(read.is_ok(), "Should be able to read clipboard");
        
        if let Ok(Some(text)) = read {
            assert_eq!(text, test_text, "Read text should match written text");
        }
    }
}

#[test]
fn test_system_info_uptime() {
    let info = zrc_platform_win::system_info::SystemInfo::collect();
    assert!(info.uptime_seconds > 0, "Uptime should be positive");
    println!("System uptime: {} seconds", info.uptime_seconds);
}

#[test]
fn test_system_info_vm_detection() {
    let info = zrc_platform_win::system_info::SystemInfo::collect();
    println!("Is VM: {}", info.is_vm);
    println!("Is RDP: {}", info.is_rdp_session);
}

#[tokio::test]
async fn test_win_platform_integration() {
    let platform = zrc_platform_win::WinPlatform::new().unwrap();
    
    // Test that we can at least try to capture (may fail in test environment)
    let _ = platform.capture_frame().await;
    
    // Test input (won't actually inject in test, but should not panic)
    use zrc_core::platform::InputEvent;
    let _ = platform.apply_input(InputEvent::MouseMove { x: 100, y: 100 }).await;
}
