package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Profile
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.Webhook
import io.github.yagipass.nestodo.domain.WebhookEdit
import io.github.yagipass.nestodo.domain.WebhookHeader
import io.github.yagipass.nestodo.domain.Webhooks
import java.util.Base64
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.RecordedRequest
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okio.Buffer
import org.junit.After
import org.junit.Assert.assertArrayEquals
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

    @Test
    fun `the webhook is read with its secrets hidden and whether the server can send webhooks`() =
        runTest {
            respond(
                200,
                """{"available":true,"webhook":{"hasToken":true,"unreadable":false,"url":null,
                    "headers":[{"name":"X-Api-Key","value":"","secret":true}],
                    "project":"Side projects","lastSentAt":"2026-11-11 09:00:00","lastError":null}}""",
            )
            assertEquals(
                Webhooks(
                    available = true,
                    webhook =
                        Webhook(
                            url = null,
                            hasToken = true,
                            headers = listOf(WebhookHeader("X-Api-Key", "", secret = true)),
                            project = "Side projects",
                            lastSentAt = "2026-11-11 09:00:00",
                        ),
                ),
                api().webhooks(),
            )
            assertEquals("/api/v1/webhook", server.takeRequest().url.encodedPath)
        }

    @Test
    fun `removing the saved token sends an explicit null, because a blank token keeps the saved one`() =
        runTest {
            respond(200, """{"available":true,"webhook":{"hasToken":false,"project":"Work"}}""")
            api()
                .saveWebhook(
                    WebhookEdit(
                        url = "",
                        urlSecret = true,
                        token = null,
                        headers = listOf(WebhookHeader("X-Api-Key", "", secret = true)),
                        project = "Work",
                    )
                )
            val request = server.takeRequest()
            assertEquals("PUT", request.method)
            assertEquals("/api/v1/webhook", request.url.encodedPath)
            val body = request.json()
            assertEquals(JsonNull, body["token"])
            assertEquals(JsonPrimitive(""), body["url"])
            assertEquals(
                Json.parseToJsonElement("""[{"name":"X-Api-Key","value":"","secret":true}]"""),
                body["headers"],
            )
        }

    @Test
    fun `a refused webhook value names its field, so the form can show the message under it`() =
        runTest {
            respond(
                400,
                """{"error":"Validation failed","issues":[{"slot":"body","issues":[
                    {"message":"Enter an http or https URL","path":"url"},
                    {"message":"A header name is required","path":"headers.0.name"}
                ]}]}""",
            )
            val refused =
                failsWith<ApiException.Invalid> {
                    api().saveWebhook(WebhookEdit("ftp://x", true, "", emptyList(), "Work"))
                }
            assertEquals(
                mapOf(
                    "url" to "Enter an http or https URL",
                    "headers" to "A header name is required",
                ),
                refused.fields,
            )
        }

    @Test
    fun `removing the webhook asks the server to delete it`() = runTest {
        respond(204)
        api().removeWebhook()
        val request = server.takeRequest()
        assertEquals("DELETE", request.method)
        assertEquals("/api/v1/webhook", request.url.encodedPath)
    }

    @Test
    fun `usage data counts as unavailable on a server that does not say, so an older server shows no switch`() =
        runTest {
            respond(200, """{"enabled":true}""")
            assertEquals(Analytics(available = false, enabled = true), api().analytics())
        }

    @Test
    fun `the PostHog project token the server sends is kept, so usage data goes to the project this server chose`() =
        runTest {
            respond(200, """{"available":true,"enabled":true,"token":"phc_test"}""")
            assertEquals(
                Analytics(available = true, enabled = true, token = "phc_test"),
                api().analytics(),
            )
        }

    @Test
    fun `turning usage data on sends the choice and returns what the server saved`() = runTest {
        respond(200, """{"enabled":true}""")
        assertEquals(true, api().setAnalytics(true))
        val request = server.takeRequest()
        assertEquals("PUT", request.method)
        assertEquals("/api/v1/analytics", request.url.encodedPath)
        assertEquals(JsonPrimitive(true), request.json()["enabled"])
    }

    @Test
    fun `a profile with no name or picture reads them as null, so the app falls back to the email and the generic icon`() =
        runTest {
            respond(200, """{"email":"kim@example.com","name":null,"avatarUrl":null}""")
            assertEquals(Profile("kim@example.com"), api().profile())
            assertEquals("/api/v1/profile", server.takeRequest().url.encodedPath)
        }

    @Test
    fun `a name is sent as typed and the saved one is read back, because the server trims it and keeps a blank one as no name`() =
        runTest {
            respond(200, """{"email":"kim@example.com","name":"Kim","avatarUrl":null}""")
            assertEquals("Kim", api().saveName("  Kim  ").name)
            val request = server.takeRequest()
            assertEquals("PUT", request.method)
            assertEquals("/api/v1/profile", request.url.encodedPath)
            assertEquals(JsonPrimitive("  Kim  "), request.json()["name"])
        }

    @Test
    fun `a refused name names its field, so the form can show the message under it`() = runTest {
        respond(
            400,
            """{"error":"Validation failed","issues":[{"slot":"body","issues":[
                {"message":"The name must be 50 characters or fewer","path":"name"}
            ]}]}""",
        )
        val refused = failsWith<ApiException.Invalid> { api().saveName("x".repeat(51)) }
        assertEquals(mapOf("name" to "The name must be 50 characters or fewer"), refused.fields)
    }

    @Test
    fun `a picture is sent as standard base64 inside JSON, because the server accepts only JSON bodies`() =
        runTest {
            val image = byteArrayOf(0x52, 0x49, 0x46, 0x46, -5, -1, 0x00, 0x3e)
            respond(
                200,
                """{"email":"kim@example.com","name":null,"avatarUrl":"/api/v1/avatars/abc"}""",
            )
            assertEquals("/api/v1/avatars/abc", api().saveAvatar(image).avatarUrl)
            val request = server.takeRequest()
            assertEquals("PUT", request.method)
            assertEquals("/api/v1/profile/avatar", request.url.encodedPath)
            val sent = (request.json()["image"] as JsonPrimitive).content
            assertEquals(Base64.getEncoder().encodeToString(image), sent)
            assertArrayEquals(image, Base64.getDecoder().decode(sent))
        }

    @Test
    fun `a picture the server refuses names the image field, so the message shows next to the picture`() =
        runTest {
            respond(
                400,
                """{"error":"Validation failed","issues":[{"slot":"body","issues":[
                    {"message":"The image must be a PNG, JPEG or WebP file","path":"image"}
                ]}]}""",
            )
            val refused = failsWith<ApiException.Invalid> { api().saveAvatar(byteArrayOf(1)) }
            assertEquals(
                mapOf("image" to "The image must be a PNG, JPEG or WebP file"),
                refused.fields,
            )
        }

    @Test
    fun `removing the picture asks the server to delete it`() = runTest {
        respond(204)
        api().removeAvatar()
        val request = server.takeRequest()
        assertEquals("DELETE", request.method)
        assertEquals("/api/v1/profile/avatar", request.url.encodedPath)
    }

    @Test
    fun `the picture is downloaded byte for byte with the Access token, because the server shows it only to its owner`() =
        runTest {
            val image = byteArrayOf(-119, 0x50, 0x4e, 0x47, 0x00, -1, 0x0a)
            server.enqueue(MockResponse.Builder().code(200).body(Buffer().write(image)).build())
            val credentials = FakeCredentials(server.hostName, "app-token", null)
            assertArrayEquals(image, api(credentials).avatar("/api/v1/avatars/abc"))
            val request = server.takeRequest()
            assertEquals("/api/v1/avatars/abc", request.url.encodedPath)
            assertEquals("app-token", request.headers["cf-access-token"])
        }

    @Test
    fun `a picture on another port is not downloaded, because the Access token would go along to whoever listens there`() =
        runTest {
            MockWebServer().use { other ->
                other.enqueue(
                    MockResponse.Builder().code(200).body(Buffer().write(byteArrayOf(1))).build()
                )
                other.start()
                val credentials = FakeCredentials(server.hostName, "app-token", null)
                failsWith<IllegalArgumentException> {
                    api(credentials).avatar(other.url("/api/v1/avatars/abc").toString())
                }
                assertEquals(0, other.requestCount)
            }
        }

    @Test
    fun `a picture address outside the avatars is not requested, so a profile cannot send the token to another endpoint`() =
        runTest {
            respond(200, """{"todos":[]}""")
            val credentials = FakeCredentials(server.hostName, "app-token", null)
            failsWith<IllegalArgumentException> {
                api(credentials).avatar("/api/v1/avatars/../todos")
            }
            assertEquals(0, server.requestCount)
        }
}
