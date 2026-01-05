package io.zippyremote.android.input

import android.content.Context
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.unit.dp

/**
 * Keyboard input handler for remote control
 */
class KeyboardInputHandler(
    private val context: Context,
    private val inputSender: InputSender
) {
    
    private val imm = context.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
    
    /**
     * Show soft keyboard
     */
    fun showKeyboard(view: View) {
        imm.showSoftInput(view, InputMethodManager.SHOW_IMPLICIT)
    }
    
    /**
     * Hide soft keyboard
     */
    fun hideKeyboard(view: View) {
        imm.hideSoftInputFromWindow(view.windowToken, 0)
    }
    
    /**
     * Handle hardware keyboard key event
     */
    fun handleKeyEvent(event: KeyEvent): Boolean {
        if (event.action == KeyEvent.ACTION_DOWN) {
            inputSender.sendKey(event.keyCode, true)
            return true
        } else if (event.action == KeyEvent.ACTION_UP) {
            inputSender.sendKey(event.keyCode, false)
            return true
        }
        return false
    }
    
    /**
     * Send special key combination (e.g., Ctrl+Alt+Del)
     */
    fun sendSpecialKeyCombination(combination: SpecialKeyCombination) {
        when (combination) {
            SpecialKeyCombination.CTRL_ALT_DEL -> {
                // Send Ctrl+Alt+Del sequence
                inputSender.sendKey(KeyEvent.KEYCODE_CTRL_LEFT, true)
                inputSender.sendKey(KeyEvent.KEYCODE_ALT_LEFT, true)
                inputSender.sendKey(KeyEvent.KEYCODE_DEL, true)
                inputSender.sendKey(KeyEvent.KEYCODE_DEL, false)
                inputSender.sendKey(KeyEvent.KEYCODE_ALT_LEFT, false)
                inputSender.sendKey(KeyEvent.KEYCODE_CTRL_LEFT, false)
            }
        }
    }
}

enum class SpecialKeyCombination {
    CTRL_ALT_DEL
}

/**
 * Special keys toolbar composable
 */
@Composable
fun SpecialKeysToolbar(
    onKeyPress: (Int, Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        // Ctrl key
        Button(
            onClick = { onKeyPress(KeyEvent.KEYCODE_CTRL_LEFT, true) },
            modifier = Modifier.weight(1f)
        ) {
            Text("Ctrl")
        }
        
        // Alt key
        Button(
            onClick = { onKeyPress(KeyEvent.KEYCODE_ALT_LEFT, true) },
            modifier = Modifier.weight(1f)
        ) {
            Text("Alt")
        }
        
        // Win/Meta key
        Button(
            onClick = { onKeyPress(KeyEvent.KEYCODE_META_LEFT, true) },
            modifier = Modifier.weight(1f)
        ) {
            Text("Win")
        }
        
        // Function keys dropdown
        var showFunctionKeys by remember { mutableStateOf(false) }
        Button(
            onClick = { showFunctionKeys = true },
            modifier = Modifier.weight(1f)
        ) {
            Text("F1-F12")
        }
        
        DropdownMenu(
            expanded = showFunctionKeys,
            onDismissRequest = { showFunctionKeys = false }
        ) {
            for (i in 1..12) {
                DropdownMenuItem(
                    text = { Text("F$i") },
                    onClick = {
                        onKeyPress(KeyEvent.KEYCODE_F1 + i - 1, true)
                        showFunctionKeys = false
                    }
                )
            }
        }
        
        // Ctrl+Alt+Del
        Button(
            onClick = {
                onKeyPress(KeyEvent.KEYCODE_CTRL_LEFT, true)
                onKeyPress(KeyEvent.KEYCODE_ALT_LEFT, true)
                onKeyPress(KeyEvent.KEYCODE_DEL, true)
            },
            modifier = Modifier.weight(1f)
        ) {
            Text("CAD")
        }
    }
}
