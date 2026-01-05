package io.zippyremote.android.keystore

import android.content.Context
import android.content.SharedPreferences
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import javax.crypto.Cipher

/**
 * Android Keystore integration for secure key storage
 */
class AndroidKeyStore(private val context: Context) {
    
    private val keyStore = KeyStore.getInstance("AndroidKeyStore").apply {
        load(null)
    }
    
    /**
     * Generate a key pair in Android Keystore
     */
    fun generateKeyPair(alias: String): KeyPair {
        val keyPairGenerator = KeyPairGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_EC,
            "AndroidKeyStore"
        )
        
        val parameterSpec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
        ).apply {
            setDigests(KeyProperties.DIGEST_SHA256)
            setUserAuthenticationRequired(false)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                setIsStrongBoxBacked(true)
            }
        }.build()
        
        keyPairGenerator.initialize(parameterSpec)
        return keyPairGenerator.generateKeyPair()
    }
    
    /**
     * Store a secret using EncryptedSharedPreferences
     */
    fun storeSecret(alias: String, data: ByteArray) {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        
        val encryptedPrefs = EncryptedSharedPreferences.create(
            context,
            "zrc_secrets",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
        
        encryptedPrefs.edit()
            .putString(alias, android.util.Base64.encodeToString(data, android.util.Base64.NO_WRAP))
            .apply()
    }
    
    /**
     * Load a secret from EncryptedSharedPreferences
     */
    fun loadSecret(alias: String): ByteArray? {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        
        val encryptedPrefs = EncryptedSharedPreferences.create(
            context,
            "zrc_secrets",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
        
        val encoded = encryptedPrefs.getString(alias, null) ?: return null
        return android.util.Base64.decode(encoded, android.util.Base64.NO_WRAP)
    }
    
    /**
     * Delete a secret
     */
    fun deleteSecret(alias: String) {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        
        val encryptedPrefs = EncryptedSharedPreferences.create(
            context,
            "zrc_secrets",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
        
        encryptedPrefs.edit()
            .remove(alias)
            .apply()
    }
    
    /**
     * Zeroize a secret by overwriting and deleting
     */
    fun zeroizeSecret(alias: String) {
        val data = loadSecret(alias)
        if (data != null) {
            // Overwrite with zeros
            data.fill(0)
            deleteSecret(alias)
        }
    }
}
