package io.github.yagipass.nestodo.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.SharedPreferencesMigration
import androidx.datastore.preferences.core.MutablePreferences
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import java.io.IOException
import kotlinx.coroutines.flow.first

interface Settings {
    suspend fun showCompleted(): Boolean

    suspend fun setShowCompleted(value: Boolean)

    suspend fun shareParent(): Long?

    suspend fun setShareParent(value: Long?)
}

private val Context.settingsStore by
    preferencesDataStore(
        name = "settings",
        corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
        produceMigrations = { listOf(SharedPreferencesMigration(it, "settings")) },
    )

class PreferenceSettings internal constructor(private val store: DataStore<Preferences>) :
    Settings {
    constructor(context: Context) : this(context.settingsStore)

    override suspend fun showCompleted() = read()[SHOW_COMPLETED] ?: true

    override suspend fun setShowCompleted(value: Boolean) = write { it[SHOW_COMPLETED] = value }

    override suspend fun shareParent() = read()[SHARE_PARENT]

    override suspend fun setShareParent(value: Long?) = write {
        if (value == null) it.remove(SHARE_PARENT) else it[SHARE_PARENT] = value
    }

    private suspend fun read() =
        try {
            store.data.first()
        } catch (_: IOException) {
            emptyPreferences()
        }

    private suspend fun write(change: (MutablePreferences) -> Unit) {
        try {
            store.edit { change(it) }
        } catch (_: IOException) {}
    }

    private companion object {
        val SHOW_COMPLETED = booleanPreferencesKey("show_completed")
        val SHARE_PARENT = longPreferencesKey("share_parent")
    }
}
