package io.zippyremote.android.connection

import kotlinx.coroutines.*
import kotlin.math.pow

/**
 * Manages automatic reconnection with exponential backoff
 */
class AutoReconnectManager(
    private val onReconnect: suspend () -> Boolean
) {
    private var reconnectJob: Job? = null
    private var attemptCount = 0
    private val maxAttempts = 10
    private val baseDelayMs = 1000L // 1 second
    
    /**
     * Start reconnection attempts
     */
    fun start() {
        if (reconnectJob?.isActive == true) {
            return // Already reconnecting
        }
        
        reconnectJob = CoroutineScope(Dispatchers.IO).launch {
            attemptReconnect()
        }
    }
    
    /**
     * Stop reconnection attempts
     */
    fun stop() {
        reconnectJob?.cancel()
        reconnectJob = null
        attemptCount = 0
    }
    
    /**
     * Reset attempt count (call after successful connection)
     */
    fun reset() {
        attemptCount = 0
    }
    
    private suspend fun attemptReconnect() {
        while (attemptCount < maxAttempts) {
            attemptCount++
            
            // Calculate exponential backoff delay
            val delayMs = (baseDelayMs * 2.0.pow(attemptCount - 1)).toLong().coerceAtMost(60000) // Max 60 seconds
            
            delay(delayMs)
            
            try {
                if (onReconnect()) {
                    // Success - reset counter
                    attemptCount = 0
                    return
                }
            } catch (e: Exception) {
                // Continue retrying
            }
        }
        
        // Max attempts reached
        attemptCount = 0
    }
}
