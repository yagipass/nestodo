package io.github.yagipass.nestodo.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import io.github.yagipass.nestodo.access.KeystoreCipher
import io.github.yagipass.nestodo.access.StorageCipher
import io.github.yagipass.nestodo.access.isKeystoreFailure
import io.github.yagipass.nestodo.domain.Outline
import java.io.IOException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json

interface OutlineCache {
    suspend fun load(server: String): Outline?

    suspend fun save(server: String, outline: Outline): Boolean

    suspend fun clear()
}

private val Context.outlineStore by
    preferencesDataStore(
        name = "outline",
        corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
    )

class PreferenceOutlineCache
internal constructor(private val store: DataStore<Preferences>, private val cipher: StorageCipher) :
    OutlineCache {
    constructor(context: Context) : this(context.outlineStore, KeystoreCipher)

    override suspend fun load(server: String): Outline? {
        val saved =
            try {
                store.data.first()
            } catch (_: IOException) {
                return null
            }
        if (saved[SERVER] != server) return null
        val sealed = saved[OUTLINE] ?: return null
        return withContext(Dispatchers.IO) {
            try {
                json.decodeFromString<Outline>(cipher.open(sealed))
            } catch (e: Exception) {
                if (!e.isKeystoreFailure()) throw e
                null
            }
        }
    }

    override suspend fun save(server: String, outline: Outline): Boolean {
        val sealed =
            withContext(Dispatchers.IO) {
                try {
                    cipher.seal(json.encodeToString(outline))
                } catch (e: Exception) {
                    if (!e.isKeystoreFailure()) throw e
                    null
                }
            } ?: return false
        return write {
            it[SERVER] = server
            it[OUTLINE] = sealed
        }
    }

    override suspend fun clear() {
        write { it.clear() }
    }

    private suspend fun write(change: (MutablePreferences) -> Unit): Boolean =
        try {
            store.edit { change(it) }
            true
        } catch (_: IOException) {
            false
        }

    private companion object {
        val SERVER = stringPreferencesKey("server")
        val OUTLINE = stringPreferencesKey("outline")
        val json = Json { ignoreUnknownKeys = true }
    }
}
