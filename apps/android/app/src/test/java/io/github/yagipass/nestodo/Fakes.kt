package io.github.yagipass.nestodo

import android.content.SharedPreferences
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.SharedPreferencesMigration
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import io.github.yagipass.nestodo.data.ApiException
import io.github.yagipass.nestodo.data.Backend
import io.github.yagipass.nestodo.data.Capture
import io.github.yagipass.nestodo.data.Connection
import io.github.yagipass.nestodo.data.LoginFlow
import io.github.yagipass.nestodo.data.Screen
import io.github.yagipass.nestodo.data.Settings
import io.github.yagipass.nestodo.data.TodoRepository
import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.Appearance
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Profile
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.Webhook
import io.github.yagipass.nestodo.domain.WebhookEdit
import io.github.yagipass.nestodo.domain.Webhooks
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.TestScope
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.rules.TemporaryFolder

class FakeRepository(var todos: List<Todo>) : TodoRepository {
    val calls = mutableListOf<String>()
    var days = listOf(Day("2026-10-26", added = 2, completed = 1, open = 3))
    var webhooks = Webhooks(available = true)
    var analytics = Analytics(available = true, enabled = false)
    val savedWebhooks = mutableListOf<WebhookEdit>()
    var profile = Profile("family@example.com")
    var profileFailure: Exception? = null
    var avatarFailure: Exception? = null
    val avatars = mutableMapOf<String, ByteArray>()
    private var nextAvatar = 1
    var failure: Exception? = null
    var analyticsFailure: Exception? = null
    var analyticsGate: CompletableDeferred<Unit>? = null
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
        val changed = if (completed) subtree(id) else generateSequence(id) { find(it).parentId }
        val ids = changed.toSet()
        todos = todos.map {
            when {
                it.id !in ids -> it
                completed -> it.copy(completedAt = it.completedAt ?: "2026-11-13 09:00:00")
                else -> it.copy(completedAt = null)
            }
        }
        return find(id)
    }

    private fun subtree(id: Long): Sequence<Long> =
        sequenceOf(id) + todos.asSequence().filter { it.parentId == id }.flatMap { subtree(it.id) }

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

    override suspend fun webhooks(): Webhooks {
        answer("GET webhook")
        return webhooks
    }

    override suspend fun saveWebhook(edit: WebhookEdit): Webhooks {
        answer("PUT webhook")
        savedWebhooks += edit
        val webhook =
            Webhook(
                url = edit.url.takeUnless { edit.urlSecret },
                hasToken = !edit.token.isNullOrEmpty(),
                headers = edit.headers.map { if (it.secret) it.copy(value = "") else it },
                project = edit.project,
            )
        webhooks = webhooks.copy(webhook = webhook)
        return webhooks
    }

    override suspend fun removeWebhook() {
        answer("DELETE webhook")
        webhooks = webhooks.copy(webhook = null)
    }

    override suspend fun analytics(): Analytics {
        answer("GET analytics")
        analyticsGate?.await()
        analyticsFailure?.let { throw it }
        return analytics
    }

    override suspend fun setAnalytics(enabled: Boolean): Boolean {
        answer("PUT analytics $enabled")
        analytics = analytics.copy(enabled = enabled)
        return enabled
    }

    override suspend fun profile(): Profile {
        answer("GET profile")
        profileFailure?.let { throw it }
        return profile
    }

    override suspend fun saveName(name: String): Profile {
        answer("PUT name \"$name\"")
        profile = profile.copy(name = name.trim().ifEmpty { null })
        return profile
    }

    override suspend fun saveAvatar(image: ByteArray): Profile {
        answer("PUT avatar")
        val url = "/api/v1/avatars/${nextAvatar++}"
        avatars[url] = image
        profile = profile.copy(avatarUrl = url)
        return profile
    }

    override suspend fun removeAvatar() {
        answer("DELETE avatar")
        profile = profile.copy(avatarUrl = null)
    }

    override suspend fun avatar(url: String): ByteArray {
        answer("GET $url")
        avatarFailure?.let { throw it }
        return avatars[url] ?: throw ApiException.Server(404)
    }
}

class FakeBackend(private val repository: TodoRepository, var signedIn: Boolean = true) : Backend {
    var failure: Exception? = null
    var loggedOut = false
    var logoutGate: CompletableDeferred<Unit>? = null

    override suspend fun connect(): Connection {
        failure?.let { throw it }
        return if (signedIn) Connection.Ready(repository, "family@example.com")
        else Connection.SignedOut
    }

    override suspend fun startLogin(): LoginFlow = error("not used")

    override suspend fun logout() {
        logoutGate?.await()
        loggedOut = true
    }
}

class FakeSettings : Settings {
    var baseUrl: HttpUrl? = "https://nestodo.example.com/".toHttpUrl()
    var baseUrlFailure: IOException? = null
    var showCompleted = true
    var shareParent: Long? = null
    var analyticsId: String? = null
    var appearance = Appearance.System

    override suspend fun baseUrl() = baseUrl

    override suspend fun setBaseUrl(value: HttpUrl?) {
        baseUrlFailure?.let { throw it }
        baseUrl = value
    }

    override suspend fun showCompleted() = showCompleted

    override suspend fun setShowCompleted(value: Boolean) {
        showCompleted = value
    }

    override suspend fun shareParent() = shareParent

    override suspend fun setShareParent(value: Long?) {
        shareParent = value
    }

    override suspend fun analyticsId() = analyticsId

    override suspend fun setAnalyticsId(value: String?) {
        analyticsId = value
    }

    override suspend fun appearance() = appearance

    override suspend fun setAppearance(value: Appearance) {
        appearance = value
    }
}

class RecordingCapture : Capture {
    data class Sent(val token: String, val distinctId: String, val screen: Screen)

    val sent = mutableListOf<Sent>()

    override suspend fun screen(token: String, distinctId: String, screen: Screen) {
        sent += Sent(token, distinctId, screen)
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
