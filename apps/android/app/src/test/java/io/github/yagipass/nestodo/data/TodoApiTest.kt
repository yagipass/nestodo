package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Todo
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

suspend inline fun <reified T : Throwable> failsWith(block: suspend () -> Unit): T {
    try {
        block()
    } catch (e: Throwable) {
        if (e is T) return e
        throw e
    }
    throw AssertionError("expected ${T::class.simpleName} to be thrown")
}

class TodoApiTest {
    private val server = MockWebServer()
    private val http = OkHttpClient.Builder().followRedirects(false).build()
    private val todoJson =
        """{"id":3,"parentId":1,"position":0,"title":"buy milk","completedAt":null,"collapsed":false}"""

    private class FakeCredentials(
        private val host: String,
        var token: String,
        private val renewed: String?,
    ) : Credentials {
        var refreshes = 0

        override fun tokenFor(url: HttpUrl): String? = token.takeIf { url.host == host }

        override suspend fun refresh(rejected: String?): Boolean {
            refreshes += 1
            token = renewed ?: return false
            return true
        }
    }

    @Before fun start() = server.start()

    @After fun stop() = server.close()

    private fun respond(code: Int, body: String = "", vararg headers: Pair<String, String>) {
        server.enqueue(
            MockResponse.Builder()
                .code(code)
                .body(body)
                .apply { headers.forEach { (name, value) -> addHeader(name, value) } }
                .build()
        )
    }

    private fun api(credentials: Credentials? = null) = TodoApi(http, server.url("/"), credentials)

    private fun RecordedRequest.json(): JsonObject =
        Json.parseToJsonElement(body!!.utf8()) as JsonObject

    @Test
    fun `a new row is created under its parent at its index so it appears where Enter was pressed`() =
        runTest {
            respond(201, """{"todo":$todoJson}""")
            assertEquals(3L, api().create("", parentId = 1, index = 2).id)
            val request = server.takeRequest()
            assertEquals("POST", request.method)
            assertEquals("/api/v1/todos", request.url.encodedPath)
            assertEquals(
                Json.parseToJsonElement("""{"title":"","parentId":1,"index":2}"""),
                request.json(),
            )
        }

    @Test
    fun `a shared todo is created with its note in one request so it never exists without it`() =
        runTest {
            respond(201, """{"todo":$todoJson}""")
            api().create("Pancakes", parentId = null, index = 0, note = "Serves 4")
            assertEquals(
                Json.parseToJsonElement(
                    """{"title":"Pancakes","parentId":null,"index":0,"note":"Serves 4"}"""
                ),
                server.takeRequest().json(),
            )
        }

    @Test
    fun `moving a row to the top level sends an explicit null parent because a missing parentId keeps the old one`() =
        runTest {
            respond(200, """{"todo":$todoJson}""")
            api().move(3, parentId = null, index = 1)
            val request = server.takeRequest()
            assertEquals("PATCH", request.method)
            assertEquals("/api/v1/todos/3", request.url.encodedPath)
            assertEquals(
                Json.parseToJsonElement("""{"parentId":null,"index":1}"""),
                request.json(),
            )
        }

    @Test
    fun `checking, folding and renaming send only their own field so they cannot undo an edit made on another device`() =
        runTest {
            repeat(3) { respond(200, """{"todo":$todoJson}""") }
            api().setCompleted(3, true)
            api().setCollapsed(3, true)
            api().rename(3, "oat milk")
            assertEquals(mapOf("completed" to JsonPrimitive(true)), server.takeRequest().json())
            assertEquals(mapOf("collapsed" to JsonPrimitive(true)), server.takeRequest().json())
            assertEquals(mapOf("title" to JsonPrimitive("oat milk")), server.takeRequest().json())
        }

    @Test
    fun `a note is sent on its own with its line breaks so it never overwrites a title edited elsewhere`() =
        runTest {
            respond(200, """{"todo":$todoJson}""")
            api().setNote(3, "oat\nnot soy")
            assertEquals(
                mapOf("note" to JsonPrimitive("oat\nnot soy")),
                server.takeRequest().json(),
            )
        }

    @Test
    fun `a todo's note is read from the server, and a server without notes gives an empty one`() =
        runTest {
            respond(
                200,
                """{"todos":[
                    {"id":1,"parentId":null,"position":0,"title":"milk","note":"oat\nnot soy","completedAt":null,"collapsed":false},
                    {"id":2,"parentId":null,"position":1,"title":"eggs","completedAt":null,"collapsed":false}
                ]}""",
            )
            assertEquals(listOf("oat\nnot soy", ""), api().list().map { it.note })
        }

    @Test
    fun `deleting a row never asks for its subtree, so the server refuses if it gained children elsewhere`() =
        runTest {
            respond(204)
            api().delete(3, recursive = false)
            val request = server.takeRequest()
            assertEquals("DELETE", request.method)
            assertEquals("/api/v1/todos/3", request.url.encodedPath)
            assertNull(request.url.query)
        }

    @Test
    fun `deleting a row with its subtree asks for it, because the server refuses to delete a todo with children otherwise`() =
        runTest {
            respond(204)
            api().delete(3, recursive = true)
            val request = server.takeRequest()
            assertEquals("DELETE", request.method)
            assertEquals("/api/v1/todos/3", request.url.encodedPath)
            assertEquals("recursive=true", request.url.query)
        }

    @Test
    fun `the outline fields are read from the server so the tree can be rebuilt`() = runTest {
        respond(
            200,
            """{"todos":[
                {"id":1,"parentId":null,"position":0,"title":"home","completedAt":null,"collapsed":true},
                {"id":2,"parentId":1,"position":0,"title":"milk","completedAt":"2026-10-23 09:00:00","collapsed":false}
            ]}""",
        )
        assertEquals(
            listOf(
                Todo(1, null, 0, "home", null, collapsed = true),
                Todo(2, 1, 0, "milk", "2026-10-23 09:00:00", collapsed = false),
            ),
            api().list(),
        )
    }

    @Test
    fun `stats are asked for in the phone's time zone so each day matches the phone's calendar`() =
        runTest {
            respond(
                200,
                """{"timeZone":"Etc/GMT+12","days":[
                    {"date":"2026-10-25","added":2,"completed":1,"open":4},
                    {"date":"2026-10-26","added":0,"completed":3,"open":1}
                ]}""",
            )
            assertEquals(
                listOf(Day("2026-10-25", 2, 1, 4), Day("2026-10-26", 0, 3, 1)),
                api().stats("Etc/GMT+12"),
            )
            val request = server.takeRequest()
            assertEquals("/api/v1/stats", request.url.encodedPath)
            assertEquals("Etc/GMT+12", request.url.queryParameter("tz"))
        }

    @Test
    fun `requests carry the Access token and no Origin header because the server rejects cross-origin writes`() =
        runTest {
            respond(201, """{"todo":$todoJson}""")
            api(FakeCredentials(server.hostName, "app-token", null)).create("buy milk", null, 0)
            val request = server.takeRequest()
            assertEquals("app-token", request.headers["cf-access-token"])
            assertNull(request.headers["Origin"])
        }

    @Test
    fun `an expired Access token is renewed once and the request is retried with the new token`() =
        runTest {
            respond(401)
            respond(200, """{"todos":[$todoJson]}""")
            val credentials = FakeCredentials(server.hostName, "old", "new")
            assertEquals(listOf(3L), api(credentials).list().map { it.id })
            assertEquals("old", server.takeRequest().headers["cf-access-token"])
            assertEquals("new", server.takeRequest().headers["cf-access-token"])
            assertEquals(1, credentials.refreshes)
        }

    @Test
    fun `a request still rejected after renewing asks the user to sign in instead of retrying forever`() =
        runTest {
            respond(403)
            respond(403)
            val credentials = FakeCredentials(server.hostName, "old", "new")
            failsWith<ApiException.NotSignedIn> { api(credentials).list() }
            assertEquals(2, server.requestCount)
        }

    @Test
    fun `a redirect to the Access login page means the login has expired`() = runTest {
        respond(
            302,
            "",
            "Location" to
                "https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com",
        )
        val credentials = FakeCredentials(server.hostName, "old", null)
        failsWith<ApiException.NotSignedIn> { api(credentials).list() }
    }

    @Test
    fun `a todo deleted on another device is reported as not found so the app can drop it`() =
        runTest {
            respond(404, """{"error":"not_found","message":"Todo not found"}""")
            assertEquals(
                3L,
                failsWith<ApiException.NotFound> { api().delete(3, recursive = false) }.id,
            )
        }

    @Test
    fun `a change the server rejects is reported as invalid input`() = runTest {
        respond(400, """{"error":"invalid_placement"}""")
        failsWith<ApiException.Invalid> { api().move(3, parentId = 3, index = 0) }
    }

    @Test
    fun `the app keeps working when the server adds fields to a todo`() = runTest {
        respond(
            200,
            """{"todos":[{"id":1,"parentId":null,"position":0,"title":"a","completedAt":null,"collapsed":false,"createdAt":"x"}]}""",
        )
        assertEquals("a", api().list().single().title)
    }
}
