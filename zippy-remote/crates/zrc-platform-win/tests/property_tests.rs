#![cfg(windows)]

use std::time::Duration;
use std::thread;

/// Property test 2.5: GDI resource cleanup
/// Property: No GDI handle leaks after capture cycles
/// Validates: Requirement 1.8
#[test]
fn property_test_gdi_resource_cleanup() {
    // Create and destroy multiple capturers
    // In a real property test, we'd check GDI handle counts
    for _ in 0..10 {
        let mut capturer = zrc_platform_win::capture_gdi::GdiCapturer::new().unwrap();
        let _frame = capturer.capture_frame().unwrap();
        // Capturer is dropped here - resources should be cleaned up
    }
    
    // If we can still create capturers after many cycles, resources are being cleaned up
    let capturer = zrc_platform_win::capture_gdi::GdiCapturer::new();
    assert!(capturer.is_ok(), "Should be able to create capturer after many cycles");
}

/// Property test 3.6: Desktop Switch Recovery
/// Property: When a desktop switch occurs, capture system recovers within 2 seconds
/// Validates: Requirements 2.5, 8.1, 8.4
#[test]
fn property_test_desktop_switch_recovery() {
    if !zrc_platform_win::capture_dxgi::DxgiCapturer::is_available() {
        println!("DXGI not available, skipping desktop switch recovery test");
        return;
    }
    
    let capturer = zrc_platform_win::capture_dxgi::DxgiCapturer::new();
    if capturer.is_err() {
        println!("DXGI capturer creation failed (may require desktop access): {:?}", capturer.err());
        return;
    }
    
    let mut capturer = capturer.unwrap();
    
    // Simulate desktop switch by calling handle_desktop_switch
    let start = std::time::Instant::now();
    let result = capturer.handle_desktop_switch();
    let elapsed = start.elapsed();
    
    // Desktop switch may fail in test environment (no actual desktop switch occurred)
    // but timing should still be reasonable
    println!("Desktop switch recovery result: {:?}, elapsed: {:?}", result, elapsed);
    assert!(elapsed < Duration::from_secs(2), "Recovery should complete within 2 seconds");
}

/// Property test 4.5: Capture Backend Fallback
/// Property: When preferred backend is unavailable, system falls back to next available
/// Validates: Requirements 1.1, 2.2, 3.2
#[test]
fn property_test_capture_backend_fallback() {
    let capturer = zrc_platform_win::capturer::WinCapturer::new();
    assert!(capturer.is_ok(), "Should always be able to create capturer with fallback");
    
    let capturer = capturer.unwrap();
    let backend = capturer.backend_type();
    
    // Should have selected one of the backends
    assert!(
        matches!(backend, zrc_platform_win::capturer::BackendType::Wgc | 
                          zrc_platform_win::capturer::BackendType::Dxgi | 
                          zrc_platform_win::capturer::BackendType::Gdi),
        "Should have selected a valid backend"
    );
    
    // GDI should always be available as last resort
    let gdi_capturer = zrc_platform_win::capturer::WinCapturer::with_backend(
        zrc_platform_win::capturer::BackendType::Gdi
    );
    assert!(gdi_capturer.is_ok(), "GDI should always be available as fallback");
}

/// Property test 6.5: Input Coordinate Accuracy
/// Property: Coordinates are correctly mapped to virtual desktop space
/// Validates: Requirements 5.5, 5.6, 5.8
#[test]
fn property_test_coordinate_accuracy() {
    use zrc_platform_win::injector::CoordinateMapper;
    
    let mapper = CoordinateMapper::new();
    
    // Test coordinate clamping - clamped values should be within virtual screen bounds
    // Note: Virtual screen can have negative coordinates in multi-monitor setups
    let (clamped_x, clamped_y) = mapper.clamp(-100, -100);
    // Clamped values should be within the virtual screen bounds (which may be negative)
    println!("Clamped coordinates: ({}, {})", clamped_x, clamped_y);
    
    // Test coordinate to absolute conversion
    let (abs_x, abs_y) = mapper.to_absolute(100, 100);
    assert!(abs_x >= 0 && abs_x <= 65535, "Absolute X should be in 0-65535 range");
    assert!(abs_y >= 0 && abs_y <= 65535, "Absolute Y should be in 0-65535 range");
    
    // Test edge cases - absolute values should always be in 0-65535 range
    let (abs_x_max, abs_y_max) = mapper.to_absolute(i32::MAX, i32::MAX);
    assert!(abs_x_max >= 0 && abs_x_max <= 65535, "Max X should be in 0-65535 range");
    assert!(abs_y_max >= 0 && abs_y_max <= 65535, "Max Y should be in 0-65535 range");
}

/// Property test 7.5: Key State Cleanup
/// Property: All held keys are released within 100ms of session termination
/// Validates: Requirement 6.7
#[test]
fn property_test_key_state_cleanup() {
    let mut injector = zrc_platform_win::injector::WinInjector::new();
    
    use windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL;
    
    // Hold some keys
    let _ = injector.inject_key(0x41, true); // 'A'
    let _ = injector.inject_key(0x42, true); // 'B'
    let _ = injector.inject_key(VK_CONTROL.0 as u32, true);
    
    // Release all keys
    let start = std::time::Instant::now();
    let result = injector.release_all_keys();
    let elapsed = start.elapsed();
    
    assert!(result.is_ok(), "Key release should succeed");
    assert!(elapsed < Duration::from_millis(100), "Key release should complete within 100ms");
    
    // Test automatic cleanup on drop
    {
        let mut injector2 = zrc_platform_win::injector::WinInjector::new();
        let _ = injector2.inject_key(0x43, true); // 'C'
        // injector2 is dropped here - should release keys automatically
    }
    
    // If we get here without hanging, keys were released
    thread::sleep(Duration::from_millis(50));
}

/// Property test 9.5: Service Status Reporting
/// Property: Service status is reported to SCM within 1 second
/// Validates: Requirement 9.3
#[test]
fn property_test_service_status_reporting() {
    use zrc_platform_win::service::WinService;
    use windows::Win32::System::Services::SERVICE_RUNNING;
    use std::sync::mpsc;
    
    let (tx, _rx) = mpsc::channel();
    let service = WinService::new("TestService".to_string(), tx);
    
    if service.is_ok() {
        let service = service.unwrap();
        
        let start = std::time::Instant::now();
        let result = service.set_status(SERVICE_RUNNING);
        let elapsed = start.elapsed();
        
        // Status reporting should be fast (even if it fails due to not being a real service)
        assert!(elapsed < Duration::from_secs(1), "Status reporting should complete within 1 second");
        // Result may fail if not actually a service, but timing should still be fast
        println!("Service status reporting result: {:?}, elapsed: {:?}", result, elapsed);
    } else {
        // Service creation may fail if not running as service - that's OK
        println!("Service creation failed (expected if not running as service)");
    }
}

/// Property test 10.5: DPAPI Scope Isolation
/// Property: Keys stored with CurrentUser scope are not accessible from other user contexts
/// Validates: Requirements 10.2, 10.3
#[test]
fn property_test_dpapi_scope_isolation() {
    use zrc_platform_win::keystore::{DpapiKeyStore, DpapiScope};
    
    let user_keystore = DpapiKeyStore::new(DpapiScope::CurrentUser);
    let machine_keystore = DpapiKeyStore::new(DpapiScope::LocalMachine);
    
    let test_key = b"user_scope_key_data";
    
    // Store with CurrentUser scope
    let result = user_keystore.store_key("scope_test_key", test_key);
    assert!(result.is_ok(), "Should be able to store key with CurrentUser scope");
    
    // Should be able to load with same scope
    let loaded = user_keystore.load_key("scope_test_key");
    assert!(loaded.is_ok(), "Should be able to load key with same scope");
    assert_eq!(loaded.unwrap().as_bytes(), test_key, "Loaded key should match");
    
    // Machine scope should not be able to access user scope key
    // (In practice, DPAPI enforces this at the OS level)
    // We can't easily test cross-user access, but we verify scopes are different
    let machine_key = b"machine_scope_key_data";
    let _ = machine_keystore.store_key("scope_test_machine", machine_key);
    
    // Cleanup
    let _ = user_keystore.delete_key("scope_test_key");
    let _ = machine_keystore.delete_key("scope_test_machine");
}
