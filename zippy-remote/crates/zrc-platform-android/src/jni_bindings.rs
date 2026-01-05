//! JNI bindings for Android

use jni::JNIEnv;
use jni::objects::{JClass, JString, JByteArray};
use jni::sys::{jlong, jbyteArray, jstring};

use crate::core::ZrcCore;
use crate::error::ZrcError;
use crate::session::Session;

/// Initialize the Rust runtime
/// Returns a handle to the ZrcCore instance
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_init(
    mut env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let result = || -> Result<jlong, ZrcError> {
        let config_str: String = env.get_string(&config_json)?.into();
        let core = ZrcCore::new(&config_str)?;
        let handle = Box::into_raw(Box::new(core)) as jlong;
        Ok(handle)
    }();

    match result {
        Ok(handle) => handle,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
            0
        }
    }
}

/// Destroy the ZrcCore instance
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            let _ = Box::from_raw(handle as *mut ZrcCore);
        }
    }
}

/// Start a session
/// Note: This blocks on async operations - should be called from background thread
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_startSession(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id: JByteArray,
) -> jlong {
    let result = || -> Result<jlong, ZrcError> {
        if handle == 0 {
            return Err(ZrcError::InvalidParameter("Invalid handle".to_string()));
        }

        let core = unsafe { &*(handle as *const ZrcCore) };
        let device_id_bytes = env.convert_byte_array(&device_id)?;

        // Use blocking runtime for async operations
        // This should ideally be called from a background thread in Kotlin
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|| {
                // Create a minimal runtime if none exists
                // Note: This is not ideal - the app should provide a runtime
                tokio::runtime::Runtime::new()
                    .ok()
                    .map(|rt| rt.handle().clone())
            })
            .ok_or_else(|| ZrcError::Core("No tokio runtime available".to_string()))?;
        
        let session_id = rt.block_on(core.start_session(device_id_bytes))?;
        Ok(session_id as jlong)
    }();

    match result {
        Ok(session_id) => session_id,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
            -1
        }
    }
}

/// End a session
/// Note: This blocks on async operations - should be called from background thread
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_endSession(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session_id: jlong,
) {
    let result = || -> Result<(), ZrcError> {
        if handle == 0 {
            return Err(ZrcError::InvalidParameter("Invalid handle".to_string()));
        }

        let core = unsafe { &*(handle as *const ZrcCore) };
        let session_id = session_id as u64;

        // Use blocking runtime for async operations
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|| {
                tokio::runtime::Runtime::new()
                    .ok()
                    .map(|rt| rt.handle().clone())
            })
            .ok_or_else(|| ZrcError::Core("No tokio runtime available".to_string()))?;
        
        rt.block_on(core.end_session(session_id))?;
        Ok(())
    }();

    if let Err(e) = result {
        let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
    }
}

/// Poll for a frame
/// Returns a ByteArray with frame data, or null if no frame available
/// Note: This blocks on async operations - should be called from background thread
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_pollFrame(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session_id: jlong,
) -> jbyteArray {
    let result = || -> Result<Option<jbyteArray>, ZrcError> {
        if handle == 0 {
            return Err(ZrcError::InvalidParameter("Invalid handle".to_string()));
        }

        let core = unsafe { &*(handle as *const ZrcCore) };
        let session_id = session_id as u64;

        // Use blocking runtime for async operations
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|| {
                tokio::runtime::Runtime::new()
                    .ok()
                    .map(|rt| rt.handle().clone())
            })
            .ok_or_else(|| ZrcError::Core("No tokio runtime available".to_string()))?;
        
        // Poll for frame
        if let Some(frame_data) = rt.block_on(core.poll_frame(session_id)) {
            // Encode frame with metadata for transfer to Java
            let encoded = frame_data.encode_for_jni();
            let byte_array = env.new_byte_array(encoded.len() as i32)?;
            env.set_byte_array_region(&byte_array, 0, &encoded)?;
            Ok(Some(byte_array.into_raw()))
        } else {
            Ok(None)
        }
    }();

    match result {
        Ok(Some(byte_array)) => byte_array,
        Ok(None) => std::ptr::null_mut(),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Send input event
/// Note: This blocks on async operations - should be called from background thread
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_sendInput(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session_id: jlong,
    event_json: JString,
) {
    let result = || -> Result<(), ZrcError> {
        if handle == 0 {
            return Err(ZrcError::InvalidParameter("Invalid handle".to_string()));
        }

        let core = unsafe { &*(handle as *const ZrcCore) };
        let session_id = session_id as u64;
        let event_str: String = env.get_string(&event_json)?.into();

        // Parse JSON to InputEvent
        let event = crate::input::InputEvent::from_json(&event_str)
            .map_err(|e| ZrcError::Input(format!("Failed to parse input event: {}", e)))?;

        // Use blocking runtime for async operations
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|| {
                tokio::runtime::Runtime::new()
                    .ok()
                    .map(|rt| rt.handle().clone())
            })
            .ok_or_else(|| ZrcError::Core("No tokio runtime available".to_string()))?;
        
        rt.block_on(core.send_input(session_id, event))?;
        Ok(())
    }();

    if let Err(e) = result {
        let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
    }
}

/// Get connection status
/// Note: This blocks on async operations - should be called from background thread
#[no_mangle]
pub extern "system" fn Java_io_zippyremote_core_ZrcCore_getConnectionStatus(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session_id: jlong,
) -> jstring {
    let result = || -> Result<String, ZrcError> {
        if handle == 0 {
            return Err(ZrcError::InvalidParameter("Invalid handle".to_string()));
        }

        let core = unsafe { &*(handle as *const ZrcCore) };
        let session_id = session_id as u64;

        // Use blocking runtime for async operations
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|| {
                tokio::runtime::Runtime::new()
                    .ok()
                    .map(|rt| rt.handle().clone())
            })
            .ok_or_else(|| ZrcError::Core("No tokio runtime available".to_string()))?;
        
        let status = rt.block_on(core.get_connection_status(session_id));
        Ok(status)
    }();

    match result {
        Ok(status) => {
            match env.new_string(&status) {
                Ok(jstring) => jstring.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", &e.to_string());
            std::ptr::null_mut()
        }
    }
}
