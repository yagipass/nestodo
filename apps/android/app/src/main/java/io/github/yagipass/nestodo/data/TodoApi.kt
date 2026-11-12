package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.access.HttpResult
import io.github.yagipass.nestodo.access.fetch
import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.WebhookEdit
import io.github.yagipass.nestodo.domain.Webhooks
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import okhttp3.HttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

interface Credentials {
    fun tokenFor(url: HttpUrl): String?

    suspend fun refresh(rejected: String?): Boolean
}

sealed class ApiException(message: String) : Exception(message) {
    class NotSignedIn : ApiException("the Access login has expired")

    class NotFound(val id: Long) : ApiException("todo $id was not found")

    class Invalid(val fields: Map<String, String> = emptyMap()) :
        ApiException("the server rejected the input")

    class Server(val status: Int) : ApiException("the server returned $status")
}

interface TodoRepository {
    suspend fun list(): List<Todo>

    suspend fun create(title: String, parentId: Long?, index: Int, note: String = ""): Todo

    suspend fun rename(id: Long, title: String): Todo

    suspend fun setNote(id: Long, note: String): Todo

    suspend fun setCompleted(id: Long, completed: Boolean): Todo

    suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo

    suspend fun move(id: Long, parentId: Long?, index: Int): Todo

    suspend fun delete(id: Long, recursive: Boolean)

    suspend fun stats(timeZone: String): List<Day>

    suspend fun webhooks(): Webhooks

    suspend fun saveWebhook(edit: WebhookEdit): Webhooks

    suspend fun removeWebhook()

    suspend fun analytics(): Analytics

    suspend fun setAnalytics(enabled: Boolean): Boolean
}

class TodoApi(
    private val http: OkHttpClient,
    private val base: HttpUrl,
    private val credentials: Credentials?,
) : TodoRepository {
    @Serializable private data class Many(val todos: List<Todo>)

    @Serializable private data class One(val todo: Todo)

    @Serializable private data class Stats(val days: List<Day>)

    @Serializable private data class Consent(val enabled: Boolean)

    @Serializable
    private data class Issue(val path: String = "", val message: String) {
        val field: String
            get() = path.substringBefore('.')
    }

    @Serializable private data class Slot(val issues: List<Issue> = emptyList())

    @Serializable private data class Refusal(val issues: List<Slot> = emptyList())

    override suspend fun list(): List<Todo> = parse<Many>(call("GET", url(COLLECTION))).todos

    override suspend fun create(title: String, parentId: Long?, index: Int, note: String): Todo {
        val body = buildJsonObject {
            put("title", title)
            put("parentId", parentId)
            put("index", index)
            if (note.isNotEmpty()) put("note", note)
        }
        return parse<One>(call("POST", url(COLLECTION), body)).todo
    }

    override suspend fun rename(id: Long, title: String): Todo =
        patch(id, buildJsonObject { put("title", title) })

    override suspend fun setNote(id: Long, note: String): Todo =
        patch(id, buildJsonObject { put("note", note) })

    override suspend fun setCompleted(id: Long, completed: Boolean): Todo =
        patch(id, buildJsonObject { put("completed", completed) })

    override suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo =
        patch(id, buildJsonObject { put("collapsed", collapsed) })

    override suspend fun move(id: Long, parentId: Long?, index: Int): Todo =
        patch(
            id,
            buildJsonObject {
                put("parentId", parentId)
                put("index", index)
            },
        )

    override suspend fun delete(id: Long, recursive: Boolean) {
        val url =
            url(item(id))
                .newBuilder()
                .apply { if (recursive) addQueryParameter("recursive", "true") }
                .build()
        call("DELETE", url, id = id)
    }

    override suspend fun stats(timeZone: String): List<Day> {
        val url = url(STATS).newBuilder().addQueryParameter("tz", timeZone).build()
        return parse<Stats>(call("GET", url)).days
    }

    override suspend fun webhooks(): Webhooks = parse(call("GET", url(WEBHOOK)))

    override suspend fun saveWebhook(edit: WebhookEdit): Webhooks =
        parse(call("PUT", url(WEBHOOK), json.encodeToJsonElement(edit).jsonObject))

    override suspend fun removeWebhook() {
        call("DELETE", url(WEBHOOK))
    }

    override suspend fun analytics(): Analytics = parse(call("GET", url(ANALYTICS)))

    override suspend fun setAnalytics(enabled: Boolean): Boolean =
        parse<Consent>(call("PUT", url(ANALYTICS), buildJsonObject { put("enabled", enabled) }))
            .enabled

    private suspend fun patch(id: Long, body: JsonObject): Todo =
        parse<One>(call("PATCH", url(item(id)), body, id)).todo

    private fun url(path: String): HttpUrl = base.resolve(path)!!

    private suspend fun call(
        method: String,
        url: HttpUrl,
        body: JsonObject? = null,
        id: Long? = null,
    ): HttpResult {
        var token = credentials?.tokenFor(url)
        var response = send(method, url, body, token)
        if (needsSignIn(response)) {
            if (credentials == null || !credentials.refresh(token)) throw ApiException.NotSignedIn()
            token = credentials.tokenFor(url)
            response = send(method, url, body, token)
            if (needsSignIn(response)) throw ApiException.NotSignedIn()
        }
        return when {
            response.code in 200..299 -> response
            response.code == 404 && id != null -> throw ApiException.NotFound(id)
            response.code == 400 -> throw ApiException.Invalid(refusedFields(response))
            else -> throw ApiException.Server(response.code)
        }
    }

    private suspend fun send(
        method: String,
        url: HttpUrl,
        body: JsonObject?,
        token: String?,
    ): HttpResult {
        val request =
            Request.Builder()
                .url(url)
                .header("Accept", "application/json")
                .apply { token?.let { header("cf-access-token", it) } }
                .method(method, body?.toString()?.toRequestBody(JSON))
                .build()
        return http.fetch(request)
    }

    private fun needsSignIn(response: HttpResult): Boolean =
        response.code == 401 ||
            response.code == 403 ||
            response.location()?.encodedPath?.startsWith(LOGIN_PATH) == true

    private fun refusedFields(response: HttpResult): Map<String, String> =
        try {
            json
                .decodeFromString<Refusal>(response.body.decodeToString())
                .issues
                .flatMap { it.issues }
                .distinctBy { it.field }
                .associate { it.field to it.message }
        } catch (_: SerializationException) {
            emptyMap()
        } catch (_: IllegalArgumentException) {
            emptyMap()
        }

    private inline fun <reified T> parse(response: HttpResult): T =
        try {
            json.decodeFromString<T>(response.body.decodeToString())
        } catch (_: SerializationException) {
            throw ApiException.Server(response.code)
        } catch (_: IllegalArgumentException) {
            throw ApiException.Server(response.code)
        }

    private companion object {
        const val COLLECTION = "/api/v1/todos"
        const val STATS = "/api/v1/stats"
        const val WEBHOOK = "/api/v1/webhook"
        const val ANALYTICS = "/api/v1/analytics"
        const val LOGIN_PATH = "/cdn-cgi/access/login"
        val JSON = "application/json".toMediaType()
        val json = Json { ignoreUnknownKeys = true }

        fun item(id: Long) = "$COLLECTION/$id"
    }
}
