package io.zippyremote.android.connection

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import androidx.annotation.RequiresApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Manages network connectivity and connection status
 */
class ConnectionManager(private val context: Context) {
    
    private val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    
    private val _connectionStatus = MutableStateFlow<ConnectionStatus>(ConnectionStatus.Disconnected)
    val connectionStatus: StateFlow<ConnectionStatus> = _connectionStatus.asStateFlow()
    
    private val _networkType = MutableStateFlow<NetworkType>(NetworkType.Unknown)
    val networkType: StateFlow<NetworkType> = _networkType.asStateFlow()
    
    private var networkCallback: ConnectivityManager.NetworkCallback? = null
    private var networkReceiver: BroadcastReceiver? = null
    
    init {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            registerNetworkCallback()
        } else {
            registerNetworkReceiver()
        }
        updateConnectionStatus()
    }
    
    @RequiresApi(Build.VERSION_CODES.N)
    private fun registerNetworkCallback() {
        val request = NetworkRequest.Builder()
            .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        
        networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                updateConnectionStatus()
            }
            
            override fun onLost(network: Network) {
                updateConnectionStatus()
            }
            
            override fun onCapabilitiesChanged(
                network: Network,
                networkCapabilities: NetworkCapabilities
            ) {
                updateNetworkType(networkCapabilities)
            }
        }
        
        connectivityManager.registerNetworkCallback(request, networkCallback!!)
    }
    
    private fun registerNetworkReceiver() {
        networkReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                updateConnectionStatus()
            }
        }
        
        val filter = IntentFilter(ConnectivityManager.CONNECTIVITY_ACTION)
        context.registerReceiver(networkReceiver, filter)
    }
    
    private fun updateConnectionStatus() {
        val activeNetwork = connectivityManager.activeNetwork
        val capabilities = activeNetwork?.let {
            connectivityManager.getNetworkCapabilities(it)
        }
        
        val isConnected = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) == true
        
        _connectionStatus.value = if (isConnected) {
            ConnectionStatus.Connected
        } else {
            ConnectionStatus.Disconnected
        }
        
        if (capabilities != null) {
            updateNetworkType(capabilities)
        }
    }
    
    private fun updateNetworkType(capabilities: NetworkCapabilities) {
        _networkType.value = when {
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> NetworkType.WiFi
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> NetworkType.Cellular
            capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> NetworkType.Ethernet
            else -> NetworkType.Unknown
        }
    }
    
    fun cleanup() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            networkCallback?.let {
                connectivityManager.unregisterNetworkCallback(it)
            }
        } else {
            networkReceiver?.let {
                context.unregisterReceiver(it)
            }
        }
    }
}

enum class ConnectionStatus {
    Connected,
    Connecting,
    Disconnected,
    Error
}

enum class NetworkType {
    WiFi,
    Cellular,
    Ethernet,
    Unknown
}
