package io.zippyremote.android.host

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.os.Bundle
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

/**
 * Accessibility service for input injection (host mode)
 */
class ZrcAccessibilityService : AccessibilityService() {
    
    companion object {
        @Volatile
        var instance: ZrcAccessibilityService? = null
            private set
    }
    
    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        
        val info = AccessibilityServiceInfo().apply {
            eventTypes = AccessibilityEvent.TYPES_ALL_MASK
            feedbackType = AccessibilityServiceInfo.FEEDBACK_GENERIC
            flags = AccessibilityServiceInfo.FLAG_REQUEST_TOUCH_EXPLORATION_MODE
            notificationTimeout = 100
        }
        setServiceInfo(info)
    }
    
    override fun onDestroy() {
        super.onDestroy()
        instance = null
    }
    
    /**
     * Inject a tap gesture
     */
    fun injectTap(x: Float, y: Float) {
        val path = Path().apply {
            moveTo(x, y)
        }
        
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, 100))
            .build()
        
        dispatchGesture(gesture, null, null)
    }
    
    /**
     * Inject a swipe gesture
     */
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
    
    /**
     * Inject text input
     */
    fun injectText(text: String) {
        val node = rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        node?.let {
            val arguments = Bundle().apply {
                putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text)
            }
            it.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments)
        }
    }
    
    /**
     * Inject scroll gesture
     */
    fun injectScroll(direction: ScrollDirection) {
        val node = rootInActiveWindow?.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        node?.let {
            val action = when (direction) {
                ScrollDirection.UP -> AccessibilityNodeInfo.ACTION_SCROLL_UP
                ScrollDirection.DOWN -> AccessibilityNodeInfo.ACTION_SCROLL_DOWN
                ScrollDirection.LEFT -> AccessibilityNodeInfo.ACTION_SCROLL_LEFT
                ScrollDirection.RIGHT -> AccessibilityNodeInfo.ACTION_SCROLL_RIGHT
            }
            it.performAction(action)
        }
    }
    
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // Handle accessibility events if needed
    }
    
    override fun onInterrupt() {
        // Handle interruption
    }
    
    enum class ScrollDirection {
        UP, DOWN, LEFT, RIGHT
    }
}
