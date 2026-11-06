package io.github.yagipass.nestodo.access

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.OkHttpClient
import okhttp3.Request

data class AccessApp(val hostname: String, val aud: String, val authDomain: String)

internal object Metadata {
    private const val ACCESS_DOMAIN_SUFFIX = ".cloudflareaccess.com"
    private const val MAX_AGE_SECS = 24L * 60 * 60
    private const val ALLOWED_SKEW_SECS = 5L * 60
    private const val MAX_KEY_SET_BYTES = 1 shl 20

    data class Claims(
        val type: String,
        val hostname: String,
        val authDomain: String,
        val aud: String,
        val iat: Long,
    ) {
        companion object {
            fun from(json: JsonObject) =
                Claims(
                    type = json.string("type").orEmpty(),
                    hostname = json.string("hostname").orEmpty(),
                    authDomain = json.string("auth_domain").orEmpty(),
                    aud = json.string("aud").orEmpty(),
                    iat = json.long("iat") ?: 0,
                )
        }
    }

    suspend fun discover(http: OkHttpClient, base: HttpUrl, now: Long): AccessApp? {
        val request =
            Request.Builder().url(base).head().header("cf-access-metadata-request", "true").build()
        val token = http.fetch(request).header("cf-access-metadata") ?: return null
        val unverified =
            Jwt.unverifiedPayload(token) ?: throw AccessException("the metadata JWT is malformed")
        val authDomain = authDomainUrl(unverified.string("auth_domain").orEmpty())
        val claims = Claims.from(Jwt.verifyRs256(token, fetchKeys(http, authDomain)))
        validate(claims, base.host, now)
        return AccessApp(hostname = base.host, aud = claims.aud, authDomain = authDomain.host)
    }

    private suspend fun fetchKeys(http: OkHttpClient, authDomain: HttpUrl): JsonObject {
        val url = authDomain.resolve("/cdn-cgi/access/certs")!!
        val response = http.fetch(Request.Builder().url(url).build(), MAX_KEY_SET_BYTES)
        if (response.code != 200) throw AccessException("$url returned ${response.code}")
        return try {
            Json.parseToJsonElement(response.body.decodeToString()) as? JsonObject
        } catch (_: IllegalArgumentException) {
            null
        } ?: throw AccessException("$url did not return a key set")
    }

    fun authDomainUrl(authDomain: String): HttpUrl {
        val host =
            "https://$authDomain".toHttpUrlOrNull()?.host?.lowercase()
                ?: throw AccessException("auth_domain \"$authDomain\" is not a host name")
        if (!host.endsWith(ACCESS_DOMAIN_SUFFIX)) {
            throw AccessException(
                "auth_domain \"$authDomain\" is not a team domain under $ACCESS_DOMAIN_SUFFIX"
            )
        }
        return HttpUrl.Builder().scheme("https").host(host).build()
    }

    fun validate(claims: Claims, requestHost: String, now: Long) {
        if (!claims.hostname.equals(requestHost, ignoreCase = true)) {
            throw AccessException(
                "the metadata is for \"${claims.hostname}\", not \"$requestHost\""
            )
        }
        if (claims.type != "match") {
            throw AccessException("the metadata type is \"${claims.type}\", not \"match\"")
        }
        if (claims.aud.isEmpty()) throw AccessException("the metadata has no application audience")
        if (claims.iat <= 0) throw AccessException("the metadata has no issue time")
        if (claims.iat < now - MAX_AGE_SECS) {
            throw AccessException("the metadata is more than 24 hours old")
        }
        if (claims.iat > now + ALLOWED_SKEW_SECS) {
            throw AccessException("the metadata is more than 5 minutes in the future")
        }
    }
}
