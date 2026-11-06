package io.github.yagipass.nestodo.access

import java.math.BigInteger
import java.security.GeneralSecurityException
import java.security.KeyFactory
import java.security.Signature
import java.security.spec.RSAPublicKeySpec
import java.util.Base64
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.longOrNull

class AccessException(message: String) : Exception(message)

internal object Jwt {
    private const val MIN_RSA_BITS = 2048

    fun unverifiedPayload(token: String): JsonObject? {
        val parts = token.split('.')
        if (parts.size != 3) return null
        return decodeObject(parts[1])
    }

    fun verifyRs256(token: String, keySet: JsonObject): JsonObject {
        fun malformed() = AccessException("the metadata JWT is malformed")
        val parts = token.split('.')
        if (parts.size != 3) throw malformed()
        val (headerPart, payloadPart, signaturePart) = parts
        val header = decodeObject(headerPart) ?: throw malformed()
        val alg = header.string("alg")
        if (alg != "RS256") throw AccessException("the metadata JWT uses $alg, not RS256")
        val kid = header.string("kid")
        val signature = decode(signaturePart) ?: throw malformed()
        val signingInput = "$headerPart.$payloadPart".toByteArray(Charsets.US_ASCII)
        val verified =
            (keySet["keys"] as? JsonArray)
                .orEmpty()
                .filterIsInstance<JsonObject>()
                .filter { it.string("kty") == "RSA" }
                .filter { kid == null || it.string("kid") == kid }
                .any { key ->
                    val n = key.string("n")?.let(::decode)
                    val e = key.string("e")?.let(::decode)
                    n != null && e != null && verifies(signingInput, signature, n, e)
                }
        if (!verified) {
            throw AccessException("the metadata JWT is not signed by the Access team's keys")
        }
        return decodeObject(payloadPart) ?: throw malformed()
    }

    private fun verifies(
        input: ByteArray,
        signature: ByteArray,
        n: ByteArray,
        e: ByteArray,
    ): Boolean {
        val modulus = BigInteger(1, n)
        if (modulus.bitLength() < MIN_RSA_BITS) return false
        return try {
            val key =
                KeyFactory.getInstance("RSA")
                    .generatePublic(RSAPublicKeySpec(modulus, BigInteger(1, e)))
            Signature.getInstance("SHA256withRSA").run {
                initVerify(key)
                update(input)
                verify(signature)
            }
        } catch (_: GeneralSecurityException) {
            false
        }
    }

    private fun decode(part: String): ByteArray? =
        try {
            Base64.getUrlDecoder().decode(part)
        } catch (_: IllegalArgumentException) {
            null
        }

    private fun decodeObject(part: String): JsonObject? {
        val bytes = decode(part) ?: return null
        return try {
            Json.parseToJsonElement(bytes.decodeToString()) as? JsonObject
        } catch (_: IllegalArgumentException) {
            null
        }
    }
}

internal fun JsonObject.string(name: String): String? =
    (this[name] as? JsonPrimitive)?.takeIf { it.isString }?.content

internal fun JsonObject.long(name: String): Long? =
    (this[name] as? JsonPrimitive)?.takeIf { !it.isString }?.longOrNull

class AccessToken
private constructor(
    private val exp: Long,
    private val audiences: List<String>,
    private val issuer: String?,
    val email: String?,
) {
    fun isLive(now: Long): Boolean = now <= exp

    fun isFor(aud: String): Boolean = aud in audiences

    fun isIssuedBy(authDomain: String): Boolean = issuer == "https://$authDomain"

    companion object {
        fun parse(token: String): AccessToken? {
            val payload = Jwt.unverifiedPayload(token) ?: return null
            val exp = payload.long("exp") ?: return null
            val audiences =
                when (val aud = payload["aud"]) {
                    is JsonPrimitive -> listOfNotNull(aud.takeIf { it.isString }?.content)
                    is JsonArray ->
                        aud.filterIsInstance<JsonPrimitive>()
                            .filter { it.isString }
                            .map { it.content }
                    else -> emptyList()
                }
            return AccessToken(exp, audiences, payload.string("iss"), payload.string("email"))
        }
    }
}
