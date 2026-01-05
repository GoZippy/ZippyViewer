package io.zippyremote.android.clipboard

import android.content.ClipData
import android.content.ClipboardManager as AndroidClipboardManager
import android.content.Context
import android.os.Build
import androidx.annotation.RequiresApi

/**
 * Clipboard synchronization manager
 */
class ClipboardManager(private val context: Context) {
    
    private val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as AndroidClipboardManager
    private var syncEnabled = false
    private var lastClipboardContent: String? = null
    
    /**
     * Read clipboard content
     */
    fun readClipboard(): String? {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            // Android 10+ requires special permission
            return readClipboardAndroid10()
        }
        
        val clip = clipboard.primaryClip ?: return null
        if (clip.itemCount > 0) {
            return clip.getItemAt(0).text?.toString()
        }
        return null
    }
    
    @RequiresApi(Build.VERSION_CODES.Q)
    private fun readClipboardAndroid10(): String? {
        // On Android 10+, clipboard access requires special handling
        // This is a placeholder - actual implementation would need proper permission handling
        val clip = clipboard.primaryClip ?: return null
        if (clip.itemCount > 0) {
            return clip.getItemAt(0).text?.toString()
        }
        return null
    }
    
    /**
     * Write to clipboard
     */
    fun writeClipboard(text: String) {
        val clip = ClipData.newPlainText("ZRC Clipboard", text)
        clipboard.setPrimaryClip(clip)
        lastClipboardContent = text
    }
    
    /**
     * Check if clipboard content has changed
     */
    fun hasClipboardChanged(): Boolean {
        val current = readClipboard()
        return current != null && current != lastClipboardContent
    }
    
    /**
     * Enable clipboard synchronization
     */
    fun enableSync() {
        syncEnabled = true
    }
    
    /**
     * Disable clipboard synchronization
     */
    fun disableSync() {
        syncEnabled = false
    }
    
    /**
     * Check if sync is enabled
     */
    fun isSyncEnabled(): Boolean = syncEnabled
}
