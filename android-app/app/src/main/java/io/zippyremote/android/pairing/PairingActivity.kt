package io.zippyremote.android.pairing

import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.camera.core.*
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import io.zippyremote.android.ui.theme.ZippyRemoteTheme
import io.zippyremote.core.ZrcCore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.concurrent.Executors

class PairingActivity : ComponentActivity() {
    private var zrcCore: ZrcCore? = null
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize ZRC core
        val config = "{}" // TODO: Load from preferences
        zrcCore = ZrcCore.create(config)
        
        setContent {
            ZippyRemoteTheme {
                PairingScreen(
                    zrcCore = zrcCore!!,
                    onQRScanned = { inviteData ->
                        // Process QR code invite
                        processInvite(inviteData)
                    },
                    onPasteInvite = {
                        val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        val clip = clipboard.primaryClip
                        if (clip != null && clip.itemCount > 0) {
                            val text = clip.getItemAt(0).text.toString()
                            processInvite(text)
                        }
                    }
                )
            }
        }
    }
    
    private fun processInvite(inviteData: String) {
        // TODO: Import invite through ZrcCore
        // For now, just finish the activity
        // In a full implementation, this would:
        // 1. Decode base64 invite
        // 2. Import through pairing controller
        // 3. Complete pairing flow
        finish()
    }
}

@Composable
fun PairingScreen(
    zrcCore: ZrcCore,
    onQRScanned: (String) -> Unit,
    onPasteInvite: () -> Unit
) {
    var showQRScanner by remember { mutableStateOf(false) }
    
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text(
            text = "Pair Device",
            style = MaterialTheme.typography.headlineMedium
        )
        
        Button(
            onClick = { showQRScanner = true },
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Scan QR Code")
        }
        
        Button(
            onClick = onPasteInvite,
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Paste Invite from Clipboard")
        }
        
        if (showQRScanner) {
            QRScannerView(
                onQRScanned = { data ->
                    onQRScanned(data)
                    showQRScanner = false
                },
                onDismiss = { showQRScanner = false }
            )
        }
    }
}

@Composable
fun QRScannerView(
    onQRScanned: (String) -> Unit,
    onDismiss: () -> Unit
) {
    val lifecycleOwner = LocalLifecycleOwner.current
    val context = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    var cameraProvider by remember { mutableStateOf<ProcessCameraProvider?>(null) }
    
    // Initialize camera provider
    LaunchedEffect(Unit) {
        val provider = withContext(Dispatchers.Main) {
            ProcessCameraProvider.getInstance(context).get()
        }
        cameraProvider = provider
    }
    
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .height(400.dp)
    ) {
        Column(
            modifier = Modifier.fillMaxSize()
        ) {
            // Close button
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End
            ) {
                IconButton(onClick = onDismiss) {
                    Text("Close")
                }
            }
            
            // Camera preview
            if (cameraProvider != null) {
                AndroidView(
                    factory = { ctx ->
                        val previewView = PreviewView(ctx)
                        val preview = Preview.Builder().build().also {
                            it.setSurfaceProvider(previewView.surfaceProvider)
                        }
                        
                        val imageAnalysis = ImageAnalysis.Builder()
                            .setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST)
                            .build()
                            .also {
                                it.setAnalyzer(
                                    Executors.newSingleThreadExecutor(),
                                    QRBarcodeAnalyzer { barcode ->
                                        scope.launch {
                                            onQRScanned(barcode.rawValue ?: "")
                                        }
                                    }
                                )
                            }
                        
                        val cameraSelector = CameraSelector.DEFAULT_BACK_CAMERA
                        
                        try {
                            cameraProvider?.unbindAll()
                            cameraProvider?.bindToLifecycle(
                                lifecycleOwner,
                                cameraSelector,
                                preview,
                                imageAnalysis
                            )
                        } catch (e: Exception) {
                            // Handle camera error
                        }
                        
                        previewView
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                )
            } else {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = androidx.compose.ui.Alignment.Center
                ) {
                    CircularProgressIndicator()
                }
            }
            
            Text(
                text = "Point camera at QR code",
                modifier = Modifier.padding(16.dp),
                style = MaterialTheme.typography.bodyMedium
            )
        }
    }
}

/**
 * Barcode analyzer for QR code scanning
 */
class QRBarcodeAnalyzer(
    private val onBarcodeDetected: (Barcode) -> Unit
) : ImageAnalysis.Analyzer {
    
    private val scanner = BarcodeScanning.getClient()
    
    override fun analyze(imageProxy: ImageProxy) {
        val mediaImage = imageProxy.image
        if (mediaImage != null) {
            val image = InputImage.fromMediaImage(
                mediaImage,
                imageProxy.imageInfo.rotationDegrees
            )
            
            scanner.process(image)
                .addOnSuccessListener { barcodes ->
                    for (barcode in barcodes) {
                        if (barcode.format == Barcode.FORMAT_QR_CODE) {
                            onBarcodeDetected(barcode)
                            break
                        }
                    }
                }
                .addOnFailureListener {
                    // Handle error
                }
                .addOnCompleteListener {
                    imageProxy.close()
                }
        } else {
            imageProxy.close()
        }
    }
}
