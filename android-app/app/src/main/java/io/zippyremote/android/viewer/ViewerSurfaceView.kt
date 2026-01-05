package io.zippyremote.android.viewer

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Matrix
import android.graphics.Paint
import android.graphics.PointF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.GestureDetector
import io.zippyremote.android.viewer.FrameReceiver

/**
 * SurfaceView for rendering remote frames
 * Supports zoom and pan gestures
 */
class ViewerSurfaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : SurfaceView(context, attrs, defStyleAttr), SurfaceHolder.Callback {
    
    private var renderThread: RenderThread? = null
    private var frameReceiver: FrameReceiver? = null
    
    private var currentZoom = 1.0f
    private var panOffset = PointF(0f, 0f)
    private var minZoom = 0.5f
    private var maxZoom = 5.0f
    
    private val scaleGestureDetector = ScaleGestureDetector(context, ScaleListener())
    private val gestureDetector = GestureDetector(context, GestureListener())
    
    private val matrix = Matrix()
    private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
    
    init {
        holder.addCallback(this)
        holder.setType(SurfaceHolder.SURFACE_TYPE_PUSH_BUFFERS)
    }
    
    fun setFrameReceiver(receiver: FrameReceiver) {
        frameReceiver = receiver
    }
    
    override fun surfaceCreated(holder: SurfaceHolder) {
        renderThread = RenderThread(holder).also { it.start() }
    }
    
    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        // Handle surface size changes
    }
    
    override fun surfaceDestroyed(holder: SurfaceHolder) {
        renderThread?.quit()
        renderThread = null
    }
    
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (scaleGestureDetector.onTouchEvent(event)) {
            return true
        }
        return gestureDetector.onTouchEvent(event)
    }
    
    private inner class ScaleListener : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScale(detector: ScaleGestureDetector): Boolean {
            val scaleFactor = detector.scaleFactor
            currentZoom *= scaleFactor
            currentZoom = currentZoom.coerceIn(minZoom, maxZoom)
            return true
        }
    }
    
    private inner class GestureListener : GestureDetector.SimpleOnGestureListener() {
        private var lastPanX = 0f
        private var lastPanY = 0f
        
        override fun onDown(e: MotionEvent): Boolean {
            lastPanX = e.x
            lastPanY = e.y
            return true
        }
        
        override fun onScroll(
            e1: MotionEvent?,
            e2: MotionEvent,
            distanceX: Float,
            distanceY: Float
        ): Boolean {
            if (e2.pointerCount == 1) {
                // Single finger pan
                panOffset.x -= distanceX / currentZoom
                panOffset.y -= distanceY / currentZoom
                return true
            }
            return false
        }
    }
    
    private inner class RenderThread(private val holder: SurfaceHolder) : Thread() {
        @Volatile
        private var running = true
        
        override fun run() {
            while (running) {
                val frame = frameReceiver?.pollFrame() ?: run {
                    Thread.sleep(16) // ~60fps
                    continue
                }
                
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
            
            // Apply zoom and pan
            matrix.reset()
            matrix.postScale(currentZoom, currentZoom)
            matrix.postTranslate(panOffset.x, panOffset.y)
            canvas.concat(matrix)
            
            // Decode frame based on format
            val bitmap = when (frame.format) {
                1 -> { // RawBGRA or RawRGBA
                    BitmapFactory.decodeByteArray(frame.data, 0, frame.data.size)
                }
                2 -> { // JPEG
                    BitmapFactory.decodeByteArray(frame.data, 0, frame.data.size)
                }
                3 -> { // PNG
                    BitmapFactory.decodeByteArray(frame.data, 0, frame.data.size)
                }
                else -> {
                    // Try to decode as image
                    BitmapFactory.decodeByteArray(frame.data, 0, frame.data.size)
                }
            }
            
            if (bitmap != null) {
                canvas.drawBitmap(bitmap, 0f, 0f, paint)
                bitmap.recycle()
            }
            
            canvas.restore()
        }
        
        fun quit() {
            running = false
        }
    }
    
    /**
     * Reset zoom and pan to default
     */
    fun resetView() {
        currentZoom = 1.0f
        panOffset = PointF(0f, 0f)
    }
    
    /**
     * Set zoom level
     */
    fun setZoom(zoom: Float) {
        currentZoom = zoom.coerceIn(minZoom, maxZoom)
    }
    
    /**
     * Get current zoom level
     */
    fun getZoom(): Float = currentZoom
}

/**
 * Frame data structure
 */
data class Frame(
    val data: ByteArray,
    val width: Int,
    val height: Int,
    val format: Int,
    val timestamp: Long
)

/**
 * Frame receiver interface
 */
interface FrameReceiver {
    fun pollFrame(): Frame?
}
