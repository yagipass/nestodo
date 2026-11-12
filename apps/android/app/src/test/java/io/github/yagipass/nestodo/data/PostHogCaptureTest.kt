package io.github.yagipass.nestodo.data

import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

class PostHogCaptureTest {
    private val server = MockWebServer()

    @Before fun start() = server.start()

    @After fun stop() = server.close()

    @Test
    fun `a screen view carries only the screen and the app, and asks PostHog to keep no profile of the member`() =
        runTest {
            server.enqueue(MockResponse.Builder().code(200).body("""{"status":"Ok"}""").build())

            PostHogCapture(OkHttpClient(), server.url("/i/v0/e/"), "1.2.3")
                .screen("phc_test", "install-id", Screen.Stats)

            val request = server.takeRequest()
            assertEquals("POST", request.method)
            assertEquals("/i/v0/e/", request.url.encodedPath)
            assertEquals(
                buildJsonObject {
                    put("api_key", "phc_test")
                    put("event", "\$screen")
                    put("distinct_id", "install-id")
                    putJsonObject("properties") {
                        put("\$screen_name", "Stats")
                        put("\$process_person_profile", false)
                        put("\$lib", "nestodo-android")
                        put("\$lib_version", "1.2.3")
                        put("\$app_version", "1.2.3")
                        put("\$os", "Android")
                        put("\$device_type", "Mobile")
                    }
                },
                Json.parseToJsonElement(request.body!!.utf8()),
            )
        }

    @Test
    fun `screen names stay the same as PostHog has recorded them, so renaming a screen in code does not split its history`() {
        assertEquals(listOf("Todos", "Stats", "Settings"), Screen.entries.map { it.label })
    }
}
