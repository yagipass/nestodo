package io.github.yagipass.nestodo

import android.content.SharedPreferences
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.SharedPreferencesMigration
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import io.github.yagipass.nestodo.data.Backend
import io.github.yagipass.nestodo.data.Connection
import io.github.yagipass.nestodo.data.LoginFlow
import io.github.yagipass.nestodo.data.Settings
import io.github.yagipass.nestodo.data.TodoRepository
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Todo
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.TestScope
import org.junit.rules.TemporaryFolder

class FakeRepository(var todos: List<Todo>) : TodoRepository {
    val calls = mutableListOf<String>()
    var days = listOf(Day("2026-10-26", added = 2, completed = 1, open = 3))
    var failure: Exception? = null
    var gate: CompletableDeferred<Unit>? = null
    private var nextId = 100L

    private suspend fun answer(call: String) {
        calls += call
        gate?.await()
        failure?.let {
            failure = null
            throw it
        }
    }

    private fun find(id: Long) = todos.firstOrNull { it.id == id } ?: Todo(id, title = "")

    override suspend fun list(): List<Todo> {
        answer("GET")
        return todos
    }

    override suspend fun create(title: String, parentId: Long?, index: Int, note: String): Todo {
        val withNote = if (note.isEmpty()) "" else " with note \"$note\""
        answer("POST \"$title\" under $parentId at $index$withNote")
        return Todo(nextId++, parentId, index, title, note = note)
    }

    override suspend fun rename(id: Long, title: String): Todo {
        answer("PATCH $id title \"$title\"")
        return find(id)
    }

    override suspend fun setNote(id: Long, note: String): Todo {
        answer("PATCH $id note \"$note\"")
        return find(id)
    }

    override suspend fun setCompleted(id: Long, completed: Boolean): Todo {
        answer("PATCH $id completed $completed")
        return find(id)
    }

    override suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo {
        answer("PATCH $id collapsed $collapsed")
        return find(id)
    }

    override suspend fun move(id: Long, parentId: Long?, index: Int): Todo {
        answer("PATCH $id under $parentId at $index")
        return find(id)
    }

    override suspend fun delete(id: Long, recursive: Boolean) {
        answer(if (recursive) "DELETE $id with its subtree" else "DELETE $id")
    }

    override suspend fun stats(timeZone: String): List<Day> {
        answer("GET stats in $timeZone")
        return days
    }
}

class FakeBackend(private val repository: TodoRepository, var signedIn: Boolean = true) : Backend {
    override suspend fun connect() =
        if (signedIn) Connection.Ready(repository, "family@example.com") else Connection.SignedOut

    override suspend fun startLogin(): LoginFlow = error("not used")

    override suspend fun logout() {}
}

class FakeSettings : Settings {
    var showCompleted = true
    var shareParent: Long? = null

    override suspend fun showCompleted() = showCompleted

    override suspend fun setShowCompleted(value: Boolean) {
        showCompleted = value
    }

    override suspend fun shareParent() = shareParent

    override suspend fun setShareParent(value: Long?) {
        shareParent = value
    }
}

fun TestScope.preferenceStore(
    folder: TemporaryFolder,
    legacy: SharedPreferences? = null,
): DataStore<Preferences> =
    PreferenceDataStoreFactory.create(
        migrations = listOfNotNull(legacy?.let { prefs -> SharedPreferencesMigration({ prefs }) }),
        scope = backgroundScope,
    ) {
        folder.root.resolve("test.preferences_pb")
    }

class MemoryPreferences(vararg saved: Pair<String, Any>) : SharedPreferences {
    private val values = mutableMapOf<String, Any?>(*saved)

    override fun getAll(): Map<String, *> = values.toMap()

    override fun contains(key: String) = key in values

    override fun edit(): SharedPreferences.Editor = Editor()

    override fun getString(key: String, defValue: String?) = error("not used")

    override fun getStringSet(key: String, defValues: Set<String>?) = error("not used")

    override fun getInt(key: String, defValue: Int) = error("not used")

    override fun getLong(key: String, defValue: Long) = error("not used")

    override fun getFloat(key: String, defValue: Float) = error("not used")

    override fun getBoolean(key: String, defValue: Boolean) = error("not used")

    override fun registerOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) = error("not used")

    override fun unregisterOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) = error("not used")

    private inner class Editor : SharedPreferences.Editor {
        private val changes = mutableListOf<(MutableMap<String, Any?>) -> Unit>()

        override fun remove(key: String): SharedPreferences.Editor {
            changes += { it.remove(key) }
            return this
        }

        override fun clear(): SharedPreferences.Editor {
            changes += { it.clear() }
            return this
        }

        override fun commit(): Boolean {
            apply()
            return true
        }

        override fun apply() {
            changes.forEach { it(values) }
            changes.clear()
        }

        override fun putString(key: String, value: String?) = error("not used")

        override fun putStringSet(key: String, values: Set<String>?) = error("not used")

        override fun putInt(key: String, value: Int) = error("not used")

        override fun putLong(key: String, value: Long) = error("not used")

        override fun putFloat(key: String, value: Float) = error("not used")

        override fun putBoolean(key: String, value: Boolean) = error("not used")
    }
}
