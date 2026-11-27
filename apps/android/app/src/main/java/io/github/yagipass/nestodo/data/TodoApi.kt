package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.access.HttpResult
import io.github.yagipass.nestodo.access.fetch
import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.CommentImage
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.ImageIcon
import io.github.yagipass.nestodo.domain.Profile
import io.github.yagipass.nestodo.domain.TimelineItem
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.WebhookEdit
import io.github.yagipass.nestodo.domain.Webhooks
import java.util.Base64
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonArray
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

    class Invalid(
        val fields: Map<String, String> = emptyMap(),
        val serverMessage: String? = null,
    ) : ApiException("the server rejected the input")

    class Server(val status: Int, val serverMessage: String? = null) :
        ApiException("the server returned $status")
}

interface TodoRepository {
    suspend fun list(): List<Todo>

    suspend fun create(title: String, parentId: Long?, index: Int, comment: String = ""): Todo

    suspend fun rename(id: Long, title: String): Todo

    suspend fun setCompleted(id: Long, completed: Boolean): Todo

    suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo

    suspend fun setIcon(id: Long, icon: String?): Todo

    suspend fun move(id: Long, parentId: Long?, index: Int): Todo

    suspend fun delete(id: Long, recursive: Boolean)

    suspend fun restore(id: Long, parentId: Long?, index: Int): Todo

    suspend fun timeline(id: Long): List<TimelineItem>

    suspend fun comment(
        todoId: Long,
        body: String,
        images: List<String> = emptyList(),
    ): TimelineItem.Comment

    suspend fun addCommentImage(image: ByteArray): CommentImage

    suspend fun commentImage(url: String): ByteArray

    suspend fun editComment(id: Long, body: String): TimelineItem.Comment

    suspend fun deleteComment(id: Long)

    suspend fun stats(timeZone: String): List<Day>

    suspend fun webhooks(): Webhooks

    suspend fun saveWebhook(edit: WebhookEdit): Webhooks

    suspend fun removeWebhook()

    suspend fun analytics(): Analytics

    suspend fun setAnalytics(enabled: Boolean): Boolean

    suspend fun profile(): Profile

    suspend fun saveName(name: String): Profile

    suspend fun saveAvatar(image: ByteArray): Profile

    suspend fun removeAvatar()

    suspend fun avatar(url: String): ByteArray

    suspend fun icons(): List<ImageIcon>

    suspend fun addIcon(name: String, image: ByteArray): ImageIcon

    suspend fun renameIcon(id: String, name: String): ImageIcon

    suspend fun deleteIcon(id: String)

    suspend fun iconImage(url: String): ByteArray
}

class TodoApi(
    private val http: OkHttpClient,
    private val base: HttpUrl,
    private val credentials: Credentials?,
) : TodoRepository {
    @Serializable private data class Many(val todos: List<Todo>)

    @Serializable private data class One(val todo: Todo)

    @Serializable private data class Stats(val days: List<Day>)

    @Serializable private data class Timeline(val items: List<TimelineItem>)

    @Serializable private data class Written(val comment: TimelineItem.Comment)

    @Serializable private data class Uploaded(val image: CommentImage)

    @Serializable private data class Consent(val enabled: Boolean)

    @Serializable private data class Shelf(val icons: List<ImageIcon>)

    @Serializable private data class Shelved(val icon: ImageIcon)

    @Serializable private data class Problem(val message: String? = null)

    @Serializable
    private data class Issue(val path: String = "", val message: String) {
        val field: String
            get() = path.substringBefore('.')
    }

    @Serializable private data class Slot(val issues: List<Issue> = emptyList())

    @Serializable private data class Refusal(val issues: List<Slot> = emptyList())

    override suspend fun list(): List<Todo> = parse<Many>(call("GET", url(COLLECTION))).todos

    override suspend fun create(title: String, parentId: Long?, index: Int, comment: String): Todo {
        val body = buildJsonObject {
            put("title", title)
            put("parentId", parentId)
            put("index", index)
            if (comment.isNotBlank()) put("comment", comment)
        }
        return parse<One>(call("POST", url(COLLECTION), body)).todo
    }

    override suspend fun rename(id: Long, title: String): Todo =
        patch(id, buildJsonObject { put("title", title) })

    override suspend fun setCompleted(id: Long, completed: Boolean): Todo =
        patch(id, buildJsonObject { put("completed", completed) })

    override suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo =
        patch(id, buildJsonObject { put("collapsed", collapsed) })

    override suspend fun setIcon(id: Long, icon: String?): Todo =
        patch(id, buildJsonObject { put("icon", icon) })

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

    override suspend fun restore(id: Long, parentId: Long?, index: Int): Todo {
        val body = buildJsonObject {
            put("parentId", parentId)
            put("index", index)
        }
        return parse<One>(call("POST", url("${item(id)}/restore"), body, id)).todo
    }

    override suspend fun timeline(id: Long): List<TimelineItem> =
        parse<Timeline>(call("GET", url("${item(id)}/timeline"), id = id)).items

    override suspend fun comment(
        todoId: Long,
        body: String,
        images: List<String>,
    ): TimelineItem.Comment {
        val sent = buildJsonObject {
            put("body", body)
            putJsonArray("images") { images.forEach { add(JsonPrimitive(it)) } }
        }
        return parse<Written>(call("POST", url("${item(todoId)}/comments"), sent, todoId)).comment
    }

    override suspend fun addCommentImage(image: ByteArray): CommentImage =
        parse<Uploaded>(
                call("POST", url(COMMENT_IMAGES), buildJsonObject { put("image", encoded(image)) })
            )
            .image

    override suspend fun commentImage(url: String): ByteArray = download(url, "$COMMENT_IMAGES/")

    override suspend fun editComment(id: Long, body: String): TimelineItem.Comment =
        parse<Written>(call("PATCH", url(commentItem(id)), commentBody(body), id)).comment

    override suspend fun deleteComment(id: Long) {
        call("DELETE", url(commentItem(id)), id = id)
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

    override suspend fun profile(): Profile = parse(call("GET", url(PROFILE)))

    override suspend fun saveName(name: String): Profile =
        parse(call("PUT", url(PROFILE), buildJsonObject { put("name", name) }))

    override suspend fun saveAvatar(image: ByteArray): Profile =
        parse(call("PUT", url(AVATAR), buildJsonObject { put("image", encoded(image)) }))

    override suspend fun removeAvatar() {
        call("DELETE", url(AVATAR))
    }

    override suspend fun avatar(url: String): ByteArray = download(url, AVATARS)

    override suspend fun icons(): List<ImageIcon> = parse<Shelf>(call("GET", url(ICONS))).icons

    override suspend fun addIcon(name: String, image: ByteArray): ImageIcon {
        val body = buildJsonObject {
            put("name", name)
            put("image", encoded(image))
        }
        return parse<Shelved>(call("POST", url(ICONS), body)).icon
    }

    override suspend fun renameIcon(id: String, name: String): ImageIcon =
        parse<Shelved>(call("PATCH", icon(id), buildJsonObject { put("name", name) })).icon

    override suspend fun deleteIcon(id: String) {
        call("DELETE", icon(id))
    }

    override suspend fun iconImage(url: String): ByteArray = download(url, "$ICONS/")

    private suspend fun download(url: String, under: String): ByteArray {
        val target =
            base.resolve(url)?.takeIf {
                it.scheme == base.scheme &&
                    it.host == base.host &&
                    it.port == base.port &&
                    it.encodedPath.startsWith(under)
            } ?: throw IllegalArgumentException("$url is not under $under on the server")
        return call("GET", target).body
    }

    private fun encoded(image: ByteArray): String = Base64.getEncoder().encodeToString(image)

    private fun icon(id: String): HttpUrl = url(ICONS).newBuilder().addPathSegment(id).build()

    private fun commentBody(body: String): JsonObject = buildJsonObject { put("body", body) }

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
            response.code == 400 ->
                throw ApiException.Invalid(refusedFields(response), messageOf(response))
            else -> throw ApiException.Server(response.code, messageOf(response))
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

    private fun messageOf(response: HttpResult): String? =
        try {
            json.decodeFromString<Problem>(response.body.decodeToString()).message
        } catch (_: SerializationException) {
            null
        } catch (_: IllegalArgumentException) {
            null
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
        const val COMMENTS = "/api/v1/comments"
        const val COMMENT_IMAGES = "/api/v1/comment-images"
        const val STATS = "/api/v1/stats"
        const val WEBHOOK = "/api/v1/webhook"
        const val ANALYTICS = "/api/v1/analytics"
        const val PROFILE = "/api/v1/profile"
        const val AVATAR = "/api/v1/profile/avatar"
        const val AVATARS = "/api/v1/avatars/"
        const val ICONS = "/api/v1/icons"
        const val LOGIN_PATH = "/cdn-cgi/access/login"
        val JSON = "application/json".toMediaType()
        val json = Json { ignoreUnknownKeys = true }

        fun item(id: Long) = "$COLLECTION/$id"

        fun commentItem(id: Long) = "$COMMENTS/$id"
    }
}
