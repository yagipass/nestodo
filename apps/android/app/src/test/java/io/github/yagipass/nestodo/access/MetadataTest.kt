package io.github.yagipass.nestodo.access

import io.github.yagipass.nestodo.access.Fixtures.NOW
import io.github.yagipass.nestodo.data.failsWith
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import mockwebserver3.SocketEffect
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okio.Buffer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class MetadataTest {
    private val day = 24L * 60 * 60
    private val skew = 5L * 60
    private val keySetLimit = 1 shl 20

    private fun claims(
        type: String = "match",
        hostname: String = "nestodo.example.com",
        aud: String = "0123abcd",
        iat: Long = NOW,
    ) = Metadata.Claims(type, hostname, "family.cloudflareaccess.com", aud, iat)

    private fun rejects(
        claims: Metadata.Claims,
        host: String = "nestodo.example.com",
        now: Long = NOW,
    ) {
        assertThrows(AccessException::class.java) { Metadata.validate(claims, host, now) }
    }

    private suspend fun discoverWithKeys(server: MockWebServer, keys: MockResponse): AccessApp? {
        server.enqueue(
            MockResponse.Builder()
                .code(200)
                .addHeader("cf-access-metadata", Fixtures.text("metadata.jwt"))
                .build()
        )
        server.enqueue(keys)
        server.start()
        val http =
            OkHttpClient.Builder()
                .addInterceptor { chain ->
                    val request = chain.request()
                    chain.proceed(
                        request.newBuilder().url(server.url(request.url.encodedPath)).build()
                    )
                }
                .build()
        return Metadata.discover(http, "https://nestodo.example.com/".toHttpUrl(), NOW)
    }

    @Test
    fun `accepts fresh metadata for the host the app asked`() {
        Metadata.validate(claims(), "nestodo.example.com", NOW)
        Metadata.validate(claims(), "NESTODO.example.com", NOW + day)
        Metadata.validate(claims(), "nestodo.example.com", NOW - skew)
    }

    @Test
    fun `rejects metadata for another host so its audience cannot be reused here`() {
        rejects(claims(), host = "evil.example.com")
    }

    @Test
    fun `rejects metadata that does not describe a matched application`() {
        rejects(claims(type = "no_match"))
        rejects(claims(aud = ""))
    }

    @Test
    fun `rejects replayed or future metadata`() {
        rejects(claims(), now = NOW + day + 1)
        rejects(claims(), now = NOW - skew - 1)
        rejects(claims(iat = 0))
    }

    @Test
    fun `fetches signing keys only from a Cloudflare Access team domain`() {
        assertEquals(
            "https://family.cloudflareaccess.com/",
            Metadata.authDomainUrl("Family.CloudflareAccess.com").toString(),
        )
        listOf(
                "evil.example.com",
                "cloudflareaccess.com.evil.example",
                "evil.example/x.cloudflareaccess.com",
                "family.cloudflareaccess.com@evil.example",
            )
            .forEach { spoofed ->
                assertThrows(spoofed, AccessException::class.java) {
                    Metadata.authDomainUrl(spoofed)
                }
            }
    }

    @Test
    fun `the signed fixture carries claims that pass validation`() {
        val keys = Json.parseToJsonElement(Fixtures.text("metadata-jwks.json")) as JsonObject
        val claims = Metadata.Claims.from(Jwt.verifyRs256(Fixtures.text("metadata.jwt"), keys))
        Metadata.validate(claims, "nestodo.example.com", NOW)
        assertEquals("family.cloudflareaccess.com", claims.authDomain)
    }

    @Test
    fun `a server without Access metadata needs no login so the app can use a local dev server`() =
        runTest {
            MockWebServer().use { server ->
                server.enqueue(MockResponse.Builder().code(200).build())
                server.start()
                assertNull(Metadata.discover(OkHttpClient(), server.url("/"), NOW))
            }
        }

    @Test
    fun `a key set over 1 MiB is refused as soon as it passes the limit so an endless body cannot fill memory`() =
        runTest {
            MockWebServer().use { server ->
                val endless =
                    MockResponse.Builder()
                        .code(200)
                        .body(Buffer().write(ByteArray(2 * (keySetLimit + 1))))
                        .onResponseBody(SocketEffect.Stall)
                        .build()
                val error = failsWith<AccessException> { discoverWithKeys(server, endless) }
                assertEquals(
                    "https://family.cloudflareaccess.com/cdn-cgi/access/certs returned more than $keySetLimit bytes",
                    error.message,
                )
            }
        }

    @Test
    fun `a key set of exactly 1 MiB is still used so the limit refuses only what is over it`() =
        runTest {
            MockWebServer().use { server ->
                val padded =
                    MockResponse.Builder()
                        .code(200)
                        .body(Fixtures.text("metadata-jwks.json").padEnd(keySetLimit))
                        .build()
                assertEquals(
                    "family.cloudflareaccess.com",
                    discoverWithKeys(server, padded)?.authDomain,
                )
            }
        }
}
