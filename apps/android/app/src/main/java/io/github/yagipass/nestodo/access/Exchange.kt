package io.github.yagipass.nestodo.access

import kotlin.time.Duration.Companion.seconds
import kotlin.time.toJavaDuration
import okhttp3.Headers
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

internal object Exchange {
    private const val TOKEN_COOKIE = "CF_Authorization"
    private const val APP_SESSION_COOKIE = "CF_AppSession"
    private const val LOGIN_PATH = "/cdn-cgi/access/login"
    private const val AUTHORIZED_PATH = "/cdn-cgi/access/authorized"
    private const val MAX_HOPS = 10
    private val TIMEOUT = 7.seconds

    suspend fun exchangeOrgToken(
        http: OkHttpClient,
        base: HttpUrl,
        app: AccessApp,
        orgToken: String,
        now: Long,
    ): String? {
        val client = http.newBuilder().callTimeout(TIMEOUT.toJavaDuration()).build()
        val flow = Flow(app, orgToken, now)
        var url = base
        repeat(MAX_HOPS) {
            val request =
                Request.Builder()
                    .url(url)
                    .head()
                    .apply { flow.cookieFor(url)?.let { header("cookie", it) } }
                    .build()
            when (val step = flow.step(url, client.fetch(request).headers)) {
                is Step.Done -> return step.token
                is Step.Follow -> url = step.url
            }
        }
        return null
    }

    sealed interface Step {
        data class Follow(val url: HttpUrl) : Step

        data class Done(val token: String?) : Step
    }

    class Flow(private val app: AccessApp, private val orgToken: String, private val now: Long) {
        private var appSession: String? = null

        fun cookieFor(url: HttpUrl): String? {
            if (url.isSecureOn(app.authDomain) && url.encodedPath.startsWith(LOGIN_PATH)) {
                return "$TOKEN_COOKIE=$orgToken"
            }
            if (url.isSecureOn(app.hostname) && url.encodedPath.startsWith(AUTHORIZED_PATH)) {
                return appSession?.let { "$APP_SESSION_COOKIE=$it" }
            }
            return null
        }

        fun step(url: HttpUrl, headers: Headers): Step {
            val next = headers["Location"]?.let(url::resolve)
            if (!url.isSecureOn(app.hostname)) {
                return next?.let(Step::Follow) ?: Step.Done(null)
            }
            headers.cookie(APP_SESSION_COOKIE)?.let { appSession = it }
            val token = headers.cookie(TOKEN_COOKIE)
            if (
                token != null &&
                    AccessToken.parse(token)?.let { it.isLive(now) && it.isFor(app.aud) } == true
            ) {
                return Step.Done(token)
            }
            if (url.encodedPath.startsWith(AUTHORIZED_PATH)) return Step.Done(null)
            return next?.let(Step::Follow) ?: Step.Done(null)
        }
    }
}

private fun HttpUrl.isSecureOn(host: String): Boolean = isHttps && this.host == host
