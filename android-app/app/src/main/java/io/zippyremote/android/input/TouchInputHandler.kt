package io.zippyremote.android.input

import android.content.Context
import android.graphics.PointF
import android.view.GestureDetector
import android.view.HapticFeedbackConstants
import android.view.MotionEvent
import android.view.View
import io.zippyremote.android.viewer.ViewerSurfaceView

/**
 * Handles touch input and maps it to remote mouse/input events
 */
class TouchInputHandler(
    private val viewerView: ViewerSurfaceView,
    private val inputSender: InputSender
) {
    
    private var lastTouchPoint: PointF? = null
    private var touchMode = TouchMode.CONTROL
    
    private var remoteWidth = 1920
    private var remoteHeight = 1080
    
    enum class TouchMode {
        CONTROL,  // Send input to remote
        ZOOM      // Local zoom/pan only
    }
    
    private val gestureDetector = GestureDetector(viewerView.context, object : GestureDetector.SimpleOnGestureListener() {
        
        override fun onSingleTapConfirmed(e: MotionEvent): Boolean {
            if (touchMode == TouchMode.CONTROL) {
                val remotePoint = mapToRemote(e.x, e.y)
                inputSender.sendClick(remotePoint.x.toInt(), remotePoint.y.toInt(), MouseButton.LEFT)
                return true
            }
            return false
        }
        
        override fun onLongPress(e: MotionEvent) {
            if (touchMode == TouchMode.CONTROL) {
                val remotePoint = mapToRemote(e.x, e.y)
                inputSender.sendClick(remotePoint.x.toInt(), remotePoint.y.toInt(), MouseButton.RIGHT)
                viewerView.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
            }
        }
        
        override fun onScroll(
            e1: MotionEvent?,
            e2: MotionEvent,
            distanceX: Float,
            distanceY: Float
        ): Boolean {
            if (touchMode == TouchMode.CONTROL) {
                if (e2.pointerCount == 2) {
                    // Two-finger scroll
                    inputSender.sendScroll(distanceY.toInt())
                    return true
                } else if (e2.pointerCount == 1) {
                    // Single finger drag = mouse move
                    val remotePoint = mapToRemote(e2.x, e2.y)
                    inputSender.sendMouseMove(remotePoint.x.toInt(), remotePoint.y.toInt())
                    return true
                }
            }
            return false
        }
    })
    
    fun onTouchEvent(event: MotionEvent): Boolean {
        return gestureDetector.onTouchEvent(event)
    }
    
    /**
     * Map local screen coordinates to remote display coordinates
     */
    private fun mapToRemote(localX: Float, localY: Float): PointF {
        val viewWidth = viewerView.width.toFloat()
        val viewHeight = viewerView.height.toFloat()
        
        // Account for zoom and pan
        val zoom = viewerView.getZoom()
        val scaleX = (remoteWidth.toFloat() / viewWidth) / zoom
        val scaleY = (remoteHeight.toFloat() / viewHeight) / zoom
        
        // Map coordinates
        val remoteX = localX * scaleX
        val remoteY = localY * scaleY
        
        return PointF(remoteX, remoteY)
    }
    
    /**
     * Set remote display dimensions
     */
    fun setRemoteDimensions(width: Int, height: Int) {
        remoteWidth = width
        remoteHeight = height
    }
    
    /**
     * Set touch mode
     */
    fun setTouchMode(mode: TouchMode) {
        touchMode = mode
    }
}

/**
 * Mouse button types
 */
enum class MouseButton {
    LEFT,
    RIGHT,
    MIDDLE
}

/**
 * Input sender interface for sending events to remote device
 */
interface InputSender {
    fun sendClick(x: Int, y: Int, button: MouseButton)
    fun sendMouseMove(x: Int, y: Int)
    fun sendScroll(delta: Int)
    fun sendKey(keyCode: Int, down: Boolean)
    fun sendText(text: String)
}
