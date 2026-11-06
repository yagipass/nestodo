package io.github.yagipass.nestodo.access

import io.github.yagipass.nestodo.access.Exchange.Step
import okhttp3.Headers.Companion.headersOf
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ExchangeTest {
    private val now = 1_000L
    private val app = AccessApp("nestodo.example.com", "app-aud", "family.cloudflareaccess.com")
    private val teamLogin =
        "https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com".toHttpUrl()
    private val authorized = "https://nestodo.example.com/cdn-cgi/access/authorized".toHttpUrl()

    private fun flow() = Exchange.Flow(app, "org-secret", now)

    private fun appToken(exp: Long, aud: String) = Fixtures.token("""{"exp":$exp,"aud":"$aud"}""")

    @Test
    fun `sends the org token only to the team login page`() {
        assertEquals(
            "CF_Authorization=org-secret",
            flow().cookieFor("$teamLogin?kid=app-aud".toHttpUrl()),
        )
        listOf(
                "https://evil.example/cdn-cgi/access/login/nestodo.example.com",
                "http://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com",
                "https://nestodo.example.com/cdn-cgi/access/login",
                "https://family.cloudflareaccess.com/other",
            )
            .forEach { elsewhere -> assertNull(elsewhere, flow().cookieFor(elsewhere.toHttpUrl())) }
    }

    @Test
    fun `carries the app session from the app to its authorized endpoint only`() {
        val flow = flow()
        val first =
            flow.step(
                "https://nestodo.example.com/".toHttpUrl(),
                headersOf(
                    "Set-Cookie",
                    "CF_AppSession=session-1; Path=/; Secure; HttpOnly",
                    "Location",
                    teamLogin.toString(),
                ),
            )
        assertTrue(first is Step.Follow)
        assertEquals(
            "CF_AppSession=session-1",
            flow.cookieFor("$authorized?token=x".toHttpUrl()),
        )
        assertNull(flow.cookieFor("https://evil.example/cdn-cgi/access/authorized".toHttpUrl()))
        assertNull(
            flow.cookieFor("http://nestodo.example.com/cdn-cgi/access/authorized".toHttpUrl())
        )
    }

    @Test
    fun `ignores an app session cookie planted by another host`() {
        val flow = flow()
        flow.step(
            teamLogin,
            headersOf("Set-Cookie", "CF_AppSession=planted", "Location", authorized.toString()),
        )
        assertNull(flow.cookieFor(authorized))
    }

    @Test
    fun `returns the app token that the authorized endpoint sets`() {
        val token = appToken(now + 60, "app-aud")
        val step =
            flow()
                .step(
                    authorized,
                    headersOf(
                        "Set-Cookie",
                        "CF_Authorization=$token; Path=/; Secure; HttpOnly",
                        "Location",
                        "/",
                    ),
                )
        assertEquals(Step.Done(token), step)
    }

    @Test
    fun `stops without a token when Access clears it or hands out one for another app`() {
        listOf(
                "CF_Authorization=; Max-Age=0",
                "CF_Authorization=${appToken(now - 1, "app-aud")}",
                "CF_Authorization=${appToken(now + 60, "other-aud")}",
            )
            .forEach { cookie ->
                val step = flow().step(authorized, headersOf("Set-Cookie", cookie, "Location", "/"))
                assertEquals(cookie, Step.Done(null), step)
            }
    }

    @Test
    fun `takes nothing from the app over plain http, where anyone on the path could plant it`() {
        val flow = flow()
        val token = appToken(now + 60, "app-aud")
        val step =
            flow.step(
                "http://nestodo.example.com/cdn-cgi/access/authorized".toHttpUrl(),
                headersOf(
                    "Set-Cookie",
                    "CF_Authorization=$token",
                    "Set-Cookie",
                    "CF_AppSession=planted",
                    "Location",
                    "https://nestodo.example.com/",
                ),
            )
        assertEquals(Step.Follow("https://nestodo.example.com/".toHttpUrl()), step)
        assertNull(flow.cookieFor(authorized))
    }

    @Test
    fun `does not take a token cookie from the team domain as the app token`() {
        val cookie = "CF_Authorization=${appToken(now + 60, "app-aud")}"
        val step =
            flow()
                .step(teamLogin, headersOf("Set-Cookie", cookie, "Location", authorized.toString()))
        assertEquals(Step.Follow(authorized), step)
    }
}
