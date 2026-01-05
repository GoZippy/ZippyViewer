package io.zippyremote.android.viewer

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Decodes frame data from JNI format
 * Format: [metadata_len: u32][metadata_bytes][frame_data]
 */
object FrameDecoder {
    
    /**
     * Decode frame from JNI byte array
     * @param data Encoded frame data from native code
     * @return Decoded Frame object, or null if decoding fails
     */
    fun decodeFrame(data: ByteArray): Frame? {
        if (data.size < 4) {
            return null
        }
        
        // Read metadata length (u32, little-endian)
        val buffer = ByteBuffer.wrap(data).order(ByteOrder.LITTLE_ENDIAN)
        val metadataLen = buffer.int.toInt() and 0xFFFFFFFF
        
        if (data.size < 4 + metadataLen) {
            return null
        }
        
        // Skip metadata for now (can parse protobuf if needed)
        val frameDataStart = 4 + metadataLen
        val frameData = data.sliceArray(frameDataStart until data.size)
        
        // Try to decode as bitmap to get dimensions
        val options = BitmapFactory.Options().apply {
            inJustDecodeBounds = true
        }
        BitmapFactory.decodeByteArray(frameData, 0, frameData.size, options)
        
        val width = options.outWidth.takeIf { it > 0 } ?: 1920
        val height = options.outHeight.takeIf { it > 0 } ?: 1080
        val format = 2 // Assume JPEG for now
        
        return Frame(
            data = frameData,
            width = width,
            height = height,
            format = format,
            timestamp = System.currentTimeMillis()
        )
    }
}
