package io.zippyremote.core

/**
 * ZRC Core wrapper for Android
 * Provides JNI bindings to the Rust zrc-platform-android library
 */
class ZrcCore private constructor(private val handle: Long) {
    
    companion object {
        init {
            System.loadLibrary("zrc_android")
        }
        
        /**
         * Create a new ZRC core instance
         * @param config JSON configuration string
         * @return ZrcCore instance
         * @throws RuntimeException if initialization fails
         */
        fun create(config: String): ZrcCore {
            val handle = init(config)
            if (handle == 0L) {
                throw RuntimeException("Failed to initialize ZRC core")
            }
            return ZrcCore(handle)
        }
        
        @JvmStatic
        private external fun init(configJson: String): Long
    }
    
    /**
     * Start a session with a remote device
     * @param deviceId Device ID as byte array (32 bytes)
     * @return Session ID, or -1 on error
     */
    external fun startSession(deviceId: ByteArray): Long
    
    /**
     * End a session
     * @param sessionId Session ID to end
     */
    external fun endSession(sessionId: Long)
    
    /**
     * Poll for a frame from the remote device
     * @param sessionId Session ID
     * @return Frame data as ByteArray, or null if no frame available
     */
    external fun pollFrame(sessionId: Long): ByteArray?
    
    /**
     * Send input event to remote device
     * @param sessionId Session ID
     * @param eventJson Input event as JSON string
     */
    external fun sendInput(sessionId: Long, eventJson: String)
    
    /**
     * Get connection status
     * @param sessionId Session ID
     * @return Connection status string (e.g., "connected", "disconnected")
     */
    external fun getConnectionStatus(sessionId: Long): String
    
    /**
     * Close and destroy the ZRC core instance
     */
    fun close() {
        destroy(handle)
    }
    
    private external fun destroy(handle: Long)
}
