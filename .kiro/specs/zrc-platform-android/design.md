# Design Document: zrc-platform-android

## Overview

The zrc-platform-android crate implements Android-specific functionality for the Zippy Remote Control (ZRC) system. This crate provides a controller application for viewing and controlling remote devices from Android devices, with optional host capabilities via MediaProjection and AccessibilityService.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        zrc-platform-android                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Kotlin/Android Layer                             │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Main      │  │   Viewer    │  │  Settings   │                  │   │
│  │  │  Activity   │  │  Activity   │  │  Activity   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Device     │  │   Pairing   │  │  Foreground │                  │   │
│  │  │  List       │  │   Flow      │  │   Service   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│                                    ▼ JNI                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Rust Core (via JNI)                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Session    │  │  Transport  │  │   Crypto    │                  │   │
│  │  │  Manager    │  │   Client    │  │   (zrc-    │                  │   │
│  │  │             │  │             │  │   crypto)   │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Android Platform APIs                            │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │  Keystore   │  │MediaProject │  │Accessibility│                  │   │
│  │  │   (Keys)    │  │   (Host)    │  │  (Host)     │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```


## Components and Interfaces

### JNI Bridge

```rust
// Rust side - lib.rs
use jni::JNIEnv;
use jni::objects::{JClass, JString, JByteArray};
use jni::sys::{jlong, jint, jboolean};

/// Initialize the Rust runtime
#[no_mangle]
pub extern "C" fn Java_io_zippyremote_core_ZrcCore_init(
    env: JNIEnv,
    _class: JClass,
    config_json: JString,
) -> jlong {
    let config: String = env.get_string(config_json).unwrap().into();
    let core = Box::new(ZrcCore::new(&config).unwrap());
    Box::into_raw(core) as jlong
}

/// Start a session
#[no_mangle]
pub extern "C" fn Java_io_zippyremote_core_ZrcCore_startSession(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    device_id: JByteArray,
) -> jlong {
    let core = unsafe { &mut *(handle as *mut ZrcCore) };
    let device_id_bytes = env.convert_byte_array(device_id).unwrap();
    
    match core.start_session(&device_id_bytes) {
        Ok(session_id) => session_id as jlong,
        Err(_) => -1,
    }
}

/// Receive frame callback
#[no_mangle]
pub extern "C" fn Java_io_zippyremote_core_ZrcCore_pollFrame(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    session_id: jlong,
) -> JByteArray {
    let core = unsafe { &*(handle as *const ZrcCore) };
    
    match core.poll_frame(session_id as u64) {
        Some(frame) => env.byte_array_from_slice(&frame.data).unwrap(),
        None => JByteArray::default(),
    }
}
```

```kotlin
// Kotlin side - ZrcCore.kt
package io.zippyremote.core

class ZrcCore private constructor(private val handle: Long) {
    
    companion object {
        init {
            System.loadLibrary("zrc_android")
        }
        
        fun create(config: String): ZrcCore {
            val handle = init(config)
            if (handle == 0L) throw RuntimeException("Failed to initialize ZRC core")
            return ZrcCore(handle)
        }
        
        @JvmStatic private external fun init(configJson: String): Long
    }
    
    external fun startSession(deviceId: ByteArray): Long
    external fun endSession(sessionId: Long)
    external fun pollFrame(sessionId: Long): ByteArray?
    external fun sendInput(sessionId: Long, eventJson: String)
    external fun getConnectionStatus(sessionId: Long): String
    
    fun close() {
        destroy(handle)
    }
    
    private external fun destroy(handle: Long)
}
```

### Frame Renderer

```kotlin
// ViewerSurfaceView.kt
class ViewerSurfaceView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    
    private var renderThread: RenderThread? = null
    private var frameReceiver: FrameReceiver? = null
    private var currentZoom = 1.0f
    private var panOffset = PointF(0f, 0f)
    
    private val scaleGestureDetector = ScaleGestureDetector(context, ScaleListener())
    private val gestureDetector = GestureDetector(context, GestureListener())
    
    init {
        holder.addCallback(this)
    }
    
    override fun surfaceCreated(holder: SurfaceHolder) {
        renderThread = RenderThread(holder).also { it.start() }
    }
    
    override fun surfaceDestroyed(holder: SurfaceHolder) {
        renderThread?.quit()
        renderThread = null
    }
    
    fun setFrameReceiver(receiver: FrameReceiver) {
        frameReceiver = receiver
    }
    
    private inner class RenderThread(private val holder: SurfaceHolder) : Thread() {
        @Volatile private var running = true
        
        override fun run() {
            while (running) {
                val frame = frameReceiver?.pollFrame() ?: continue
                
                val canvas = holder.lockCanvas() ?: continue
                try {
                    renderFrame(canvas, frame)
                } finally {
                    holder.unlockCanvasAndPost(canvas)
                }
            }
        }
        
        private fun renderFrame(canvas: Canvas, frame: Frame) {
            canvas.save()
            canvas.scale(currentZoom, currentZoom)
            canvas.translate(panOffset.x, panOffset.y)
            
            // Decode and draw frame
            val bitmap = BitmapFactory.decodeByteArray(frame.data, 0, frame.data.size)
            canvas.drawBitmap(bitmap, 0f, 0f, null)
            bitmap.recycle()
            
            canvas.restore()
        }
        
        fun quit() {
            running = false
        }
    }
}
```

### Touch Input Handler

```kotlin
// TouchInputHandler.kt
class TouchInputHandler(
    private val viewerView: ViewerSurfaceView,
    private val inputSender: InputSender
) {
    
    private var lastTouchPoint: PointF? = null
    private var touchMode = TouchMode.CONTROL
    
    enum class TouchMode {
        CONTROL,  // Send input to remote
        ZOOM      // Local zoom/pan
    }
    
    private val gestureDetector = GestureDetector(viewerView.context, object : GestureDetector.SimpleOnGestureListener() {
        
        override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
            val remotePoint = mapToRemote(e.x, e.y)
            inputSender.sendClick(remotePoint.x, remotePoint.y, MouseButton.LEFT)
            return true
        }
        
        override fun onLongPress(e: MotionEvent) {
            val remotePoint = mapToRemote(e.x, e.y)
            inputSender.sendClick(remotePoint.x, remotePoint.y, MouseButton.RIGHT)
            viewerView.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
        }
        
        override fun onScroll(e1: MotionEvent?, e2: MotionEvent, dx: Float, dy: Float): Boolean {
            if (e2.pointerCount == 2) {
                // Two-finger scroll
                inputSender.sendScroll(dy.toInt())
            } else {
                // Single finger drag = mouse move
                val remotePoint = mapToRemote(e2.x, e2.y)
                inputSender.sendMouseMove(remotePoint.x, remotePoint.y)
            }
            return true
        }
    })
    
    fun onTouchEvent(event: MotionEvent): Boolean {
        return gestureDetector.onTouchEvent(event)
    }
    
    private fun mapToRemote(localX: Float, localY: Float): PointF {
        // Map local coordinates to remote display coordinates
        val scaleX = remoteWidth / viewerView.width.toFloat()
        val scaleY = remoteHeight / viewerView.height.toFloat()
        return PointF(localX * scaleX, localY * scaleY)
    }
}
```

### Android Keystore Integration

```kotlin
// AndroidKeyStore.kt
class AndroidKeyStore : KeyStore {
    
    private val keyStore = java.security.KeyStore.getInstance("AndroidKeyStore").apply {
        load(null)
    }
    
    fun generateKeyPair(alias: String): KeyPair {
        val keyPairGenerator = KeyPairGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_EC,
            "AndroidKeyStore"
        )
        
        val parameterSpec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
        ).apply {
            setDigests(KeyProperties.DIGEST_SHA256)
            setUserAuthenticationRequired(false)
            // Use StrongBox if available
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                setIsStrongBoxBacked(true)
            }
        }.build()
        
        keyPairGenerator.initialize(parameterSpec)
        return keyPairGenerator.generateKeyPair()
    }
    
    fun storeSecret(alias: String, data: ByteArray) {
        // Use encrypted shared preferences or keystore-backed encryption
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        
        val encryptedPrefs = EncryptedSharedPreferences.create(
            context,
            "zrc_secrets",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
        
        encryptedPrefs.edit()
            .putString(alias, Base64.encodeToString(data, Base64.NO_WRAP))
            .apply()
    }
    
    fun loadSecret(alias: String): ByteArray? {
        // Load from encrypted shared preferences
    }
}
```

### Host Mode - MediaProjection

```kotlin
// ScreenCaptureService.kt
class ScreenCaptureService : Service() {
    
    private var mediaProjection: MediaProjection? = null
    private var virtualDisplay: VirtualDisplay? = null
    private var imageReader: ImageReader? = null
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val resultCode = intent?.getIntExtra("resultCode", Activity.RESULT_CANCELED) ?: return START_NOT_STICKY
        val data = intent.getParcelableExtra<Intent>("data") ?: return START_NOT_STICKY
        
        startForeground(NOTIFICATION_ID, createNotification())
        
        val projectionManager = getSystemService(MEDIA_PROJECTION_SERVICE) as MediaProjectionManager
        mediaProjection = projectionManager.getMediaProjection(resultCode, data)
        
        setupVirtualDisplay()
        
        return START_STICKY
    }
    
    private fun setupVirtualDisplay() {
        val metrics = resources.displayMetrics
        
        imageReader = ImageReader.newInstance(
            metrics.widthPixels,
            metrics.heightPixels,
            PixelFormat.RGBA_8888,
            2
        ).apply {
            setOnImageAvailableListener({ reader ->
                val image = reader.acquireLatestImage() ?: return@setOnImageAvailableListener
                processFrame(image)
                image.close()
            }, handler)
        }
        
        virtualDisplay = mediaProjection?.createVirtualDisplay(
            "ZRC Screen Capture",
            metrics.widthPixels,
            metrics.heightPixels,
            metrics.densityDpi,
            DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR,
            imageReader?.surface,
            null,
            handler
        )
    }
    
    private fun processFrame(image: Image) {
        val plane = image.planes[0]
        val buffer = plane.buffer
        val data = ByteArray(buffer.remaining())
        buffer.get(data)
        
        // Send to connected controller via Rust core
        zrcCore.sendFrame(sessionId, data, image.width, image.height)
    }
}
```

### Host Mode - AccessibilityService

```kotlin
// ZrcAccessibilityService.kt
class ZrcAccessibilityService : AccessibilityService() {
    
    companion object {
        var instance: ZrcAccessibilityService? = null
            private set
    }
    
    override fun onServiceConnected() {
        instance = this
        
        val info = AccessibilityServiceInfo().apply {
            eventTypes = AccessibilityEvent.TYPES_ALL_MASK
            feedbackType = AccessibilityServiceInfo.FEEDBACK_GENERIC
            flags = AccessibilityServiceInfo.FLAG_REQUEST_TOUCH_EXPLORATION_MODE
        }
        serviceInfo = info
    }
    
    override fun onDestroy() {
        instance = null
        super.onDestroy()
    }
    
    fun injectTap(x: Float, y: Float) {
        val path = Path().apply {
            moveTo(x, y)
        }
        
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, 100))
            .build()
        
        dispatchGesture(gesture, null, null)
    }
    
    fun injectSwipe(startX: Float, startY: Float, endX: Float, endY: Float, duration: Long) {
        val path = Path().apply {
            moveTo(startX, startY)
            lineTo(endX, endY)
        }
        
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, duration))
            .build()
        
        dispatchGesture(gesture, null, null)
    }
    
    fun injectText(text: String) {
        val node = rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        node?.let {
            val arguments = Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text)
            }
            it.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments)
        }
    }
    
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {}
    override fun onInterrupt() {}
}
```

## Data Models

### Configuration

```kotlin
data class ZrcConfig(
    val rendezvousUrls: List<String>,
    val relayUrls: List<String>,
    val transportPreference: TransportPreference = TransportPreference.AUTO,
    val captureQuality: CaptureQuality = CaptureQuality.BALANCED,
)

enum class TransportPreference {
    AUTO, MESH, DIRECT, RELAY
}

enum class CaptureQuality {
    LOW, BALANCED, HIGH
}
```

## Correctness Properties

### Property 1: JNI Memory Safety
*For any* JNI call, native memory SHALL be properly managed with explicit cleanup to prevent leaks.
**Validates: Requirements 12.5, 12.6**

### Property 2: Frame Rendering Continuity
*For any* frame sequence, frames SHALL be rendered in order, dropping late frames rather than blocking.
**Validates: Requirements 1.7, 1.8**

### Property 3: Touch Coordinate Accuracy
*For any* touch event, the mapped remote coordinates SHALL be within ±1 pixel of the mathematically correct mapping.
**Validates: Requirements 2.1, 2.4**

### Property 4: Keystore Security
*For any* key stored in Android Keystore, the key SHALL use hardware backing when available.
**Validates: Requirements 9.1, 9.2**

### Property 5: Service Lifecycle
*For any* foreground service, a notification SHALL be displayed and the service SHALL stop when the session ends.
**Validates: Requirements 4.7, 7.4**

## Error Handling

| Error Condition | Response | Recovery |
|-----------------|----------|----------|
| JNI exception | Log, propagate to Kotlin | Show error dialog |
| MediaProjection denied | Show permission dialog | Guide user |
| Accessibility not enabled | Show setup instructions | Guide to settings |
| Network change | Attempt reconnection | Notify user |
| Low memory | Drop frames | Continue operation |

## Testing Strategy

### Unit Tests
- JNI binding correctness
- Coordinate mapping
- Gesture recognition
- Keystore operations

### Integration Tests
- Full session flow
- Frame rendering pipeline
- Input injection (emulator)
- Network transitions

### Device Tests
- Various Android versions (API 26+)
- Different screen sizes
- Hardware keyboard support
- Accessibility service
