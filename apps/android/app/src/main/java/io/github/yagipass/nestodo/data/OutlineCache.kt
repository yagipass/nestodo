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
import io.github.yagipass.nestodo.domain.ImageIcon
import io.github.yagipass.nestodo.domain.MyIcons
import io.github.yagipass.nestodo.domain.Outline
import java.io.IOException
import java.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

interface OutlineCache {
    suspend fun load(server: String): Outline?

    suspend fun save(server: String, outline: Outline): Boolean

    suspend fun loadIcons(server: String): MyIcons?

    suspend fun saveIcons(server: String, icons: MyIcons): Boolean

    suspend fun clear()
}

private val Context.outlineStore by
    preferencesDataStore(
        name = "outline",
        corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
    )

private val Context.iconStore by
    preferencesDataStore(
        name = "icons",
        corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
    )

class PreferenceOutlineCache
internal constructor(
    private val store: DataStore<Preferences>,
    private val iconStore: DataStore<Preferences>,
    private val cipher: StorageCipher,
) : OutlineCache {
    constructor(context: Context) : this(context.outlineStore, context.iconStore, KeystoreCipher)

    @Serializable
    private data class Shelf(val icons: List<ImageIcon>, val images: Map<String, String>)

    override suspend fun load(server: String): Outline? =
        read(store, server, OUTLINE) { json.decodeFromString<Outline>(it) }

    override suspend fun save(server: String, outline: Outline): Boolean =
        write(store, server, OUTLINE) { json.encodeToString(outline) }

    override suspend fun loadIcons(server: String): MyIcons? =
        read(iconStore, server, ICONS) { plain ->
            val shelf = json.decodeFromString<Shelf>(plain)
            MyIcons(shelf.icons, shelf.images.mapValues { Base64.getDecoder().decode(it.value) })
        }

    override suspend fun saveIcons(server: String, icons: MyIcons): Boolean =
        write(iconStore, server, ICONS) {
            val images = icons.images.mapValues { Base64.getEncoder().encodeToString(it.value) }
            json.encodeToString(Shelf(icons.list, images))
        }

    override suspend fun clear() {
        edit(store) { it.clear() }
        edit(iconStore) { it.clear() }
    }

    private suspend fun <T> read(
        store: DataStore<Preferences>,
        server: String,
        key: Preferences.Key<String>,
        decode: (String) -> T,
    ): T? {
        val saved =
            try {
                store.data.first()
            } catch (_: IOException) {
                return null
            }
        if (saved[SERVER] != server) return null
        val sealed = saved[key] ?: return null
        return withContext(Dispatchers.IO) {
            try {
                decode(cipher.open(sealed))
            } catch (e: Exception) {
                if (!e.isKeystoreFailure()) throw e
                null
            }
        }
    }

    private suspend fun write(
        store: DataStore<Preferences>,
        server: String,
        key: Preferences.Key<String>,
        encode: () -> String,
    ): Boolean {
        val sealed =
            withContext(Dispatchers.IO) {
                try {
                    cipher.seal(encode())
                } catch (e: Exception) {
                    if (!e.isKeystoreFailure()) throw e
                    null
                }
            } ?: return false
        return edit(store) {
            it[SERVER] = server
            it[key] = sealed
        }
    }

    private suspend fun edit(
        store: DataStore<Preferences>,
        change: (MutablePreferences) -> Unit,
    ): Boolean =
        try {
            store.edit { change(it) }
            true
        } catch (_: IOException) {
            false
        }

    private companion object {
        val SERVER = stringPreferencesKey("server")
        val OUTLINE = stringPreferencesKey("outline")
        val ICONS = stringPreferencesKey("icons")
        val json = Json { ignoreUnknownKeys = true }
    }
}
