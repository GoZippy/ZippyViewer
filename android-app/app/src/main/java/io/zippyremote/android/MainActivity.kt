package io.zippyremote.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.zippyremote.android.ui.theme.ZippyRemoteTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            ZippyRemoteTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    MainScreen(
                        onStartSession = { deviceId ->
                            val intent = Intent(this, ViewerActivity::class.java).apply {
                                putExtra("device_id", deviceId) // deviceId is already a hex string
                            }
                            startActivity(intent)
                        },
                        onPairDevice = {
                            val intent = Intent(this, PairingActivity::class.java)
                            startActivity(intent)
                        }
                    )
                }
            }
        }
    }
}

@Composable
fun MainScreen(
    onStartSession: (String) -> Unit,
    onPairDevice: () -> Unit
) {
    var deviceList by remember { mutableStateOf<List<String>>(emptyList()) }
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "ZippyRemote",
            style = MaterialTheme.typography.headlineLarge
        )
        
        Spacer(modifier = Modifier.height(32.dp))
        
        Button(
            onClick = onPairDevice,
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Pair Device")
        }
        
        if (deviceList.isEmpty()) {
            Text(
                text = "No paired devices",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        } else {
            deviceList.forEach { deviceId ->
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    onClick = { onStartSession(deviceId) }
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp)
                    ) {
                        Text(
                            text = "Device: ${deviceId.take(8)}...",
                            style = MaterialTheme.typography.titleMedium
                        )
                        Text(
                            text = "Tap to connect",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
        }
    }
}
