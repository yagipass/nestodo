package io.github.yagipass.nestodo.access

import java.util.Base64
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class JwtTest {
    private val metadataJwt = Fixtures.text("metadata.jwt")
    private val keys = Json.parseToJsonElement(Fixtures.text("metadata-jwks.json")) as JsonObject

    private fun encode(json: String) =
        Base64.getUrlEncoder().withoutPadding().encodeToString(json.toByteArray())

    @Test
    fun `accepts metadata signed by a key in the Access team's key set`() {
        val claims = Jwt.verifyRs256(metadataJwt, keys)
        assertEquals("nestodo.example.com", claims.string("hostname"))
        assertEquals("0123abcd", claims.string("aud"))
    }

    @Test
    fun `rejects metadata whose claims were changed after signing so a forged host cannot get our audience`() {
        val (header, _, signature) = metadataJwt.split('.')
        val forged =
            encode(
                """{"type":"match","hostname":"evil.example.com","auth_domain":"family.cloudflareaccess.com","aud":"0123abcd","iat":1792000000}"""
            )
        assertThrows(AccessException::class.java) {
            Jwt.verifyRs256("$header.$forged.$signature", keys)
        }
    }

    @Test
    fun `rejects metadata signed by a key that is not in the set`() {
        val rotated =
            Json.parseToJsonElement(
                Fixtures.text("metadata-jwks.json").replace("test-kid", "rotated-kid")
            ) as JsonObject
        assertThrows(AccessException::class.java) { Jwt.verifyRs256(metadataJwt, rotated) }
    }

    @Test
    fun `rejects unsigned metadata even when its claims look valid`() {
        val (_, payload, _) = metadataJwt.split('.')
        val none = encode("""{"alg":"none","kid":"test-kid"}""")
        assertThrows(AccessException::class.java) { Jwt.verifyRs256("$none.$payload.", keys) }
    }

    @Test
    fun `an access token can be used until its expiry second`() {
        val token = AccessToken.parse(Fixtures.token("""{"exp":100,"aud":"app"}"""))!!
        assertTrue(token.isLive(100))
        assertFalse(token.isLive(101))
    }

    @Test
    fun `an access token matches an audience given as a string or a list`() {
        val one = AccessToken.parse(Fixtures.token("""{"exp":1,"aud":"app"}"""))!!
        val many = AccessToken.parse(Fixtures.token("""{"exp":1,"aud":["other","app"]}"""))!!
        val none = AccessToken.parse(Fixtures.token("""{"exp":1}"""))!!
        assertTrue(one.isFor("app") && many.isFor("app"))
        assertFalse(one.isFor("other-app") || none.isFor("app"))
    }

    @Test
    fun `values that are not JWTs are not mistaken for access tokens`() {
        assertNull(AccessToken.parse(""))
        assertNull(AccessToken.parse("deleted"))
        assertNull(AccessToken.parse(Fixtures.token("""{"exp":1}""") + ".extra"))
    }
}
