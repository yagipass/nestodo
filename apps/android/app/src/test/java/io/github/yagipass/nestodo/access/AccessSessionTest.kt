package io.github.yagipass.nestodo.access

import kotlinx.coroutines.test.runTest
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AccessSessionTest {
    private val now = 1_000L
    private val app = AccessApp("nestodo.example.com", "app", "family.cloudflareaccess.com")

    private fun token(exp: Long, aud: String, team: String = "family.cloudflareaccess.com") =
        Fixtures.token("""{"exp":$exp,"aud":["$aud"],"iss":"https://$team"}""")

    private fun session(
        store: TokenStore = MemoryTokenStore(),
        token: String = token(now + 60, "app"),
    ) =
        AccessSession(
            OkHttpClient(),
            "https://nestodo.example.com".toHttpUrl(),
            app,
            store,
            token,
        ) {
            now
        }

    @Test
    fun `uses a saved app token while it is live so the browser does not open on every launch`() {
        val saved = token(now + 60, "app")
        assertEquals(Plan.Use(saved), plan(saved, null, app, now))
    }

    @Test
    fun `renews an expired app token from the org token without opening the browser`() {
        val org = token(now + 3600, "org")
        assertEquals(Plan.Exchange(org), plan(token(now - 1, "app"), org, app, now))
        assertEquals(Plan.Exchange(org), plan(null, org, app, now))
    }

    @Test
    fun `does not use an app token issued for another application`() {
        assertEquals(Plan.SignIn, plan(token(now + 60, "other"), null, app, now))
    }

    @Test
    fun `asks for a login when both tokens are expired, missing or broken`() {
        assertEquals(Plan.SignIn, plan(token(now - 1, "app"), token(now - 1, "org"), app, now))
        assertEquals(Plan.SignIn, plan(null, null, app, now))
        assertEquals(Plan.SignIn, plan("garbage", "garbage", app, now))
    }

    @Test
    fun `never sends the org token to a team other than the one that issued it, since it opens every app of that team`() {
        val org = token(now + 3600, "org")
        val elsewhere = app.copy(authDomain = "other.cloudflareaccess.com")
        assertEquals(Plan.SignIn, plan(null, org, elsewhere, now))
        assertEquals(Plan.Exchange(org), plan(null, org, app, now))
        assertEquals(Plan.SignIn, plan(null, Fixtures.token("""{"exp":${now + 3600}}"""), app, now))
    }

    @Test
    fun `sends the app token only to the host it was issued for`() {
        val session = session()
        assertTrue(session.tokenFor("https://nestodo.example.com/api/v1/todos".toHttpUrl()) != null)
        listOf(
                "https://evil.example/api/v1/todos",
                "https://nestodo.example.com.evil.example/api/v1/todos",
                "https://sub.nestodo.example.com/api/v1/todos",
                "https://family.cloudflareaccess.com/cdn-cgi/access/login",
                "https://login.cloudflareaccess.org/transfer/key",
            )
            .forEach { other -> assertNull(other, session.tokenFor(other.toHttpUrl())) }
    }

    @Test
    fun `a request rejected with a token that was already replaced is retried without another exchange`() =
        runTest {
            assertTrue(session().refresh(rejected = "an older token"))
        }

    @Test
    fun `without an org token a rejected request cannot be renewed and needs a new login`() =
        runTest {
            val current = token(now + 60, "app")
            assertFalse(session(token = current).refresh(rejected = current))
        }
}
