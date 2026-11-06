package io.github.yagipass.nestodo.access

import java.io.InterruptedIOException
import java.security.SecureRandom
import java.util.Base64
import kotlin.time.Duration.Companion.seconds
import kotlin.time.toJavaDuration
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request

data class Tokens(val appToken: String, val orgToken: String)

class PendingLogin
internal constructor(
    val loginUrl: HttpUrl,
    val publicKey: String,
    internal val secretKey: ByteArray,
)

internal object Transfer {
    private const val TRANSFER_URL = "https://login.cloudflareaccess.org/transfer/"
    private const val POLL_ATTEMPTS = 10
    private val POLL_TIMEOUT = 60.seconds

    fun start(base: HttpUrl, aud: String, random: SecureRandom = SecureRandom()): PendingLogin {
        val secret = NaclBox.generateSecretKey(random)
        val publicKey = Base64.getUrlEncoder().encodeToString(NaclBox.publicKey(secret))
        return PendingLogin(loginUrl(base, aud, publicKey), publicKey, secret)
    }

    suspend fun await(http: OkHttpClient, pending: PendingLogin): Tokens {
        val (body, serviceKey) = poll(http, "$TRANSFER_URL${pending.publicKey}".toHttpUrl())
        return decrypt(pending.secretKey, body, serviceKey)
    }

    fun loginUrl(base: HttpUrl, aud: String, publicKey: String): HttpUrl {
        val redirect =
            base
                .newBuilder()
                .query(null)
                .addQueryParameter("aud", aud)
                .addQueryParameter("token", publicKey)
                .build()
        return base
            .newBuilder()
            .encodedPath("/cdn-cgi/access/cli")
            .query(null)
            .addQueryParameter("aud", aud)
            .addQueryParameter("edge_token_transfer", "true")
            .addQueryParameter("redirect_url", redirect.toString())
            .addQueryParameter("send_org_token", "true")
            .addQueryParameter("token", publicKey)
            .build()
    }

    private suspend fun poll(http: OkHttpClient, transfer: HttpUrl): Pair<ByteArray, String> {
        val client =
            http
                .newBuilder()
                .readTimeout(POLL_TIMEOUT.toJavaDuration())
                .callTimeout(POLL_TIMEOUT.toJavaDuration())
                .build()
        repeat(POLL_ATTEMPTS) {
            val response =
                try {
                    client.fetch(Request.Builder().url(transfer).build())
                } catch (_: InterruptedIOException) {
                    return@repeat
                }
            if (response.code >= 500) {
                throw AccessException("the Cloudflare login service returned ${response.code}")
            }
            if (response.code == 200 && response.body.isNotEmpty()) {
                return response.body to response.header("service-public-key").orEmpty()
            }
        }
        throw AccessException("timed out waiting for the browser login")
    }

    fun decrypt(secretKey: ByteArray, body: ByteArray, serviceKey: String): Tokens {
        fun fail(reason: String) = AccessException("could not read the login response: $reason")
        val sealed =
            try {
                Base64.getDecoder().decode(body.decodeToString().trim())
            } catch (_: IllegalArgumentException) {
                throw fail("it is not base64")
            }
        if (sealed.size < NaclBox.NONCE_LEN) throw fail("it is too short")
        val service =
            try {
                Base64.getUrlDecoder().decode(serviceKey)
            } catch (_: IllegalArgumentException) {
                null
            }
        if (service == null || service.size != 32) {
            throw fail("the service public key is missing or malformed")
        }
        val plaintext =
            NaclBox.open(
                box = sealed.copyOfRange(NaclBox.NONCE_LEN, sealed.size),
                nonce = sealed.copyOfRange(0, NaclBox.NONCE_LEN),
                peerPublicKey = service,
                secretKey = secretKey,
            ) ?: throw fail("it was not encrypted for this login")
        val json =
            try {
                Json.parseToJsonElement(plaintext.decodeToString()) as? JsonObject
            } catch (_: IllegalArgumentException) {
                null
            }
        val appToken = json?.string("app_token")
        val orgToken = json?.string("org_token")
        if (appToken == null || orgToken == null) throw fail("it does not contain tokens")
        return Tokens(appToken, orgToken)
    }
}
