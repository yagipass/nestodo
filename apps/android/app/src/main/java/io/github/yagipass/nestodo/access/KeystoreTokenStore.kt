package io.github.yagipass.nestodo.access

import android.app.KeyguardManager
import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.SharedPreferencesMigration
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import java.security.GeneralSecurityException
import java.security.KeyStore
import java.security.ProviderException
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlinx.coroutines.flow.first

class DeviceLockedException :
    IOException("the saved sign-in cannot be used until the phone is unlocked")

internal interface TokenCipher {
    fun seal(plain: String): String

    fun open(sealed: String): String
}

private val Context.tokenStore by
    preferencesDataStore(
        name = "access_tokens",
        corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
        produceMigrations = { listOf(SharedPreferencesMigration(it, "access_tokens")) },
    )

class KeystoreTokenStore
internal constructor(
    private val store: DataStore<Preferences>,
    private val cipher: TokenCipher,
    private val isDeviceLocked: () -> Boolean,
) : TokenStore {
    constructor(
        context: Context
    ) : this(
        context.tokenStore,
        KeystoreCipher,
        { context.getSystemService(KeyguardManager::class.java)?.isDeviceLocked == true },
    )

    override suspend fun load(host: String, kind: TokenKind): String? {
        val stored = store.data.first()[key(host, kind)] ?: return null
        return try {
            cipher.open(stored)
        } catch (e: Exception) {
            if (!e.isKeystoreFailure()) throw e
            if (isDeviceLocked()) throw DeviceLockedException()
            delete(host, kind)
            null
        }
    }

    override suspend fun save(host: String, kind: TokenKind, token: String) {
        val sealed =
            try {
                cipher.seal(token)
            } catch (e: Exception) {
                if (e.isKeystoreFailure() && isDeviceLocked()) throw DeviceLockedException()
                throw e
            }
        store.edit { it[key(host, kind)] = sealed }
    }

    override suspend fun delete(host: String, kind: TokenKind) {
        store.edit { it.remove(key(host, kind)) }
    }

    private fun key(host: String, kind: TokenKind) = stringPreferencesKey("$host/${kind.name}")
}

private fun Exception.isKeystoreFailure() =
    this is GeneralSecurityException ||
        this is ProviderException ||
        this is IllegalArgumentException

private object KeystoreCipher : TokenCipher {
    private const val KEYSTORE = "AndroidKeyStore"
    private const val ALIAS = "access-tokens-unlocked"
    private const val OLD_ALIAS = "access-tokens"
    private const val TRANSFORMATION = "AES/GCM/NoPadding"
    private const val TAG_BITS = 128

    override fun seal(plain: String): String {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, secretKey())
        val sealed = cipher.doFinal(plain.toByteArray())
        val encoder = Base64.getEncoder()
        return "${encoder.encodeToString(cipher.iv)}.${encoder.encodeToString(sealed)}"
    }

    override fun open(sealed: String): String {
        val parts = sealed.split('.')
        require(parts.size == 2) { "a stored token has ${parts.size} parts" }
        val (iv, data) = parts.map(Base64.getDecoder()::decode)
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(TAG_BITS, iv))
        return cipher.doFinal(data).decodeToString()
    }

    private fun secretKey(): SecretKey {
        val keyStore = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (keyStore.getKey(ALIAS, null) as? SecretKey)?.let {
            return it
        }
        if (keyStore.containsAlias(OLD_ALIAS)) keyStore.deleteEntry(OLD_ALIAS)
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                    ALIAS,
                    KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
                )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setUnlockedDeviceRequired(true)
                .build()
        )
        return generator.generateKey()
    }
}
