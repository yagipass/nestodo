package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.access.Fixtures
import io.github.yagipass.nestodo.access.MemoryTokenStore
import kotlinx.coroutines.test.runTest
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import org.junit.Assert.assertTrue
import org.junit.Test

class AccessBackendTest {
    private val metadata = ArrayDeque<String?>()

    private val http =
        OkHttpClient.Builder()
            .addInterceptor { chain ->
                val request = chain.request()
                val response =
                    Response.Builder().request(request).protocol(Protocol.HTTP_1_1).message("")
                when {
                    request.method == "HEAD" -> {
                        response.code(200)
                        metadata.removeFirst()?.let { response.header("cf-access-metadata", it) }
                    }
                    request.url.encodedPath == "/cdn-cgi/access/certs" ->
                        response
                            .code(200)
                            .body(Fixtures.text("metadata-jwks.json").toResponseBody())
                    else -> response.code(404)
                }
                response.build()
            }
            .build()

    private fun backend() =
        AccessBackend(http, "https://nestodo.example.com/".toHttpUrl(), MemoryTokenStore()) {
            Fixtures.NOW
        }

    @Test
    fun `a reply without Access metadata does not keep the app from signing in once Access answers`() =
        runTest {
            metadata.addAll(listOf(null, Fixtures.text("metadata.jwt")))
            val backend = backend()

            backend.connect()

            assertTrue(
                backend
                    .startLogin()
                    .url
                    .startsWith("https://nestodo.example.com/cdn-cgi/access/cli?")
            )
        }
}
