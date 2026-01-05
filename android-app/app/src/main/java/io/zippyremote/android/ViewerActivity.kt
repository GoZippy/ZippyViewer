package io.zippyremote.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import io.zippyremote.android.ui.theme.ZippyRemoteTheme
import io.zippyremote.android.viewer.ViewerSurfaceView
import io.zippyremote.android.input.TouchInputHandler
import io.zippyremote.android.input.InputSender
import io.zippyremote.android.input.MouseButton
import io.zippyremote.core.ZrcCore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class ViewerActivity : ComponentActivity() {
    private var zrcCore: ZrcCore? = null
    private var sessionId: Long = -1
    private var touchInputHandler: TouchInputHandler? = null
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        val deviceId = intent.getByteArrayExtra("device_id") ?: return finish()
        
        // Initialize ZRC core
        val config = "{}" // TODO: Load from preferences
        zrcCore = ZrcCore.create(config)
        
        // Start session
        sessionId = zrcCore?.startSession(deviceId) ?: -1
        if (sessionId == -1L) {
            finish()
            return
        }
        
        setContent {
            ZippyRemoteTheme {
                ViewerScreen(
                    zrcCore = zrcCore!!,
                    sessionId = sessionId,
                    onInputSenderCreated = { sender ->
                        touchInputHandler = TouchInputHandler(
                            viewerView = it,
                            inputSender = sender
                        )
                    }
                )
            }
        }
    }
    
    override fun onDestroy() {
        super.onDestroy()
        if (sessionId != -1L) {
            zrcCore?.endSession(sessionId)
        }
        zrcCore?.close()
    }
}

@Composable
fun ViewerScreen(
    zrcCore: ZrcCore,
    sessionId: Long,
    onInputSenderCreated: (ViewerSurfaceView, InputSender) -> Unit
) {
    val scope = rememberCoroutineScope()
    var connectionStatus by remember { mutableStateOf("disconnected") }
    
    // Create input sender
    val inputSender = remember {
        object : InputSender {
            override fun sendClick(x: Int, y: Int, button: MouseButton) {
                val eventJson = """
                    {
                        "event_type": "MouseDown",
                        "mouse_x": $x,
                        "mouse_y": $y,
                        "button": ${button.ordinal + 1}
                    }
                """.trimIndent()
                zrcCore.sendInput(sessionId, eventJson)
            }
            
            override fun sendMouseMove(x: Int, y: Int) {
                val eventJson = """
                    {
                        "event_type": "MouseMove",
                        "mouse_x": $x,
                        "mouse_y": $y
                    }
                """.trimIndent()
                zrcCore.sendInput(sessionId, eventJson)
            }
            
            override fun sendScroll(delta: Int) {
                val eventJson = """
                    {
                        "event_type": "Scroll",
                        "scroll_delta_y": $delta
                    }
                """.trimIndent()
                zrcCore.sendInput(sessionId, eventJson)
            }
            
            override fun sendKey(keyCode: Int, down: Boolean) {
                // TODO: Implement
            }
            
            override fun sendText(text: String) {
                val eventJson = """
                    {
                        "event_type": "KeyChar",
                        "text": "$text"
                    }
                """.trimIndent()
                zrcCore.sendInput(sessionId, eventJson)
            }
        }
    }
    
    // Poll for frames and update connection status
    LaunchedEffect(sessionId) {
        while (true) {
            withContext(Dispatchers.IO) {
                // Poll frame (blocking call from background thread)
                val frame = zrcCore.pollFrame(sessionId)
                connectionStatus = zrcCore.getConnectionStatus(sessionId)
                // Frame will be processed by ViewerSurfaceView's FrameReceiver
            }
            kotlinx.coroutines.delay(16) // ~60fps
        }
    }
    
    Column(
        modifier = Modifier.fillMaxSize()
    ) {
        // Connection status bar
        Surface(
            color = MaterialTheme.colorScheme.surfaceVariant
        ) {
            Text(
                text = "Status: $connectionStatus",
                modifier = Modifier.padding(8.dp),
                style = MaterialTheme.typography.bodySmall
            )
        }
        
        // Viewer surface
        AndroidView(
            factory = { context ->
                ViewerSurfaceView(context).apply {
                    setFrameReceiver(object : io.zippyremote.android.viewer.FrameReceiver {
                        override fun pollFrame(): io.zippyremote.android.viewer.Frame? {
                            val frameData = zrcCore.pollFrame(sessionId) ?: return null
                            // Decode frame from JNI format
                            return io.zippyremote.android.viewer.FrameDecoder.decodeFrame(frameData)
                        }
                    })
                    onInputSenderCreated(this, inputSender)
                }
            },
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
        )
    }
}
