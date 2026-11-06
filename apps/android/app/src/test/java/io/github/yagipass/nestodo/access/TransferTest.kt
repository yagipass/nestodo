package io.github.yagipass.nestodo.access

import java.util.Base64
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class TransferTest {
    private val clientPublicKey = "e06Qm75__kTEZaIgA31gjuNYl9Me-XLwf3SJLLD3PxM="
    private val servicePublicKey = "D6poTtKIZ7l_Smot7l34zpdOdrcBjj8iocTPJnhXDyA="
    private val sealedByGoNaclBox =
        "MzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzidj3Q9nvJVpYeA5GREvgT00+r+nOtpkzy9d1AaPWpzWNOUW1mmO/ix6g4qL6MdB1hck3rK6cMyapJz88p+szI7mtZ9VmmzLWETpjrrA="

    private val client = ByteArray(32) { 0x11 }

    private fun encodedPublicKey(secret: ByteArray) =
        Base64.getUrlEncoder().encodeToString(NaclBox.publicKey(secret))

    @Test
    fun `opens a login response sealed by the same NaCl box that cloudflared uses`() {
        val tokens = Transfer.decrypt(client, sealedByGoNaclBox.toByteArray(), servicePublicKey)
        assertEquals(Tokens("app.token.value", "org.token.value"), tokens)
    }

    @Test
    fun `encodes the public key the way the transfer service expects`() {
        assertEquals(clientPublicKey, encodedPublicKey(client))
    }

    @Test
    fun `refuses a login response that was not sealed for this login`() {
        val stranger = ByteArray(32) { 0x44 }
        assertThrows(AccessException::class.java) {
            Transfer.decrypt(stranger, sealedByGoNaclBox.toByteArray(), servicePublicKey)
        }
        val otherService = encodedPublicKey(ByteArray(32) { 0x55 })
        assertThrows(AccessException::class.java) {
            Transfer.decrypt(client, sealedByGoNaclBox.toByteArray(), otherService)
        }
    }

    @Test
    fun `refuses malformed login responses`() {
        assertThrows(AccessException::class.java) {
            Transfer.decrypt(client, "not base64!".toByteArray(), servicePublicKey)
        }
        val tooShort = Base64.getEncoder().encodeToString(ByteArray(23))
        assertThrows(AccessException::class.java) {
            Transfer.decrypt(client, tooShort.toByteArray(), servicePublicKey)
        }
        assertThrows(AccessException::class.java) {
            Transfer.decrypt(client, sealedByGoNaclBox.toByteArray(), "")
        }
    }

    @Test
    fun `builds the same login URL as cloudflared so Access hands the token to this app`() {
        val login =
            Transfer.loginUrl("https://nestodo.example.com".toHttpUrl(), "app-aud", clientPublicKey)
        assertEquals(Fixtures.text("login-url.txt"), login.toString())
    }
}
