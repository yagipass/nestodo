package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.domain.Analytics
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlinx.serialization.json.putJsonObject
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.coroutines.executeAsync

enum class Screen(val label: String) {
    Todos("Todos"),
    Stats("Stats"),
    Settings("Settings"),
}

fun interface Capture {
    suspend fun screen(token: String, distinctId: String, screen: Screen)
}

class PostHogCapture(
    private val http: OkHttpClient,
    private val endpoint: HttpUrl,
    private val version: String,
) : Capture {
    override suspend fun screen(token: String, distinctId: String, screen: Screen) {
        val body = buildJsonObject {
            put("api_key", token)
            put("event", "\$screen")
            put("distinct_id", distinctId)
            putJsonObject("properties") {
                put("\$screen_name", screen.label)
                put("\$process_person_profile", false)
                put("\$lib", "nestodo-android")
                put("\$lib_version", version)
                put("\$app_version", version)
                put("\$os", "Android")
                put("\$device_type", "Mobile")
            }
        }
        val request =
            Request.Builder().url(endpoint).post(body.toString().toRequestBody(JSON)).build()
        http.newCall(request).executeAsync().close()
    }

    companion object {
        val ENDPOINT = "https://us.i.posthog.com/i/v0/e/".toHttpUrl()
        private val JSON = "application/json".toMediaType()
    }
}

class UsageAnalytics(private val settings: Settings, private val capture: Capture) {
    private val ids = Mutex()
    private var token: String? = null
    private var enabled = false
    private var id: String? = null
    private var shown: Screen? = null
    private var counted = false

    private val sendingTo: String?
        get() = token?.takeIf { enabled }

    fun stop() {
        token = null
        enabled = false
        shown = null
        counted = false
    }

    suspend fun start(analytics: Analytics) {
        consent(analytics)
        count()
    }

    suspend fun consent(analytics: Analytics) {
        token = analytics.token
        consent(analytics.enabled)
    }

    suspend fun consent(enabled: Boolean) {
        this.enabled = enabled
        if (!enabled) {
            ids.withLock {
                id = null
                settings.setAnalyticsId(null)
            }
        }
    }

    suspend fun show(screen: Screen?) {
        shown = screen
        counted = false
        count()
    }

    private suspend fun count() {
        val screen = shown?.takeUnless { counted } ?: return
        val token = sendingTo ?: return
        counted = true
        val id = ids.withLock {
            if (sendingTo != token) return
            id ?: (settings.analyticsId() ?: newId()).also { id = it }
        }
        if (sendingTo != token) return
        try {
            capture.screen(token, id, screen)
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {}
    }

    private suspend fun newId(): String =
        UUID.randomUUID().toString().also { settings.setAnalyticsId(it) }
}
