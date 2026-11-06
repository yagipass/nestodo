package io.github.yagipass.nestodo.access

import java.security.MessageDigest
import java.security.SecureRandom
import org.bouncycastle.crypto.engines.XSalsa20Engine
import org.bouncycastle.crypto.macs.Poly1305
import org.bouncycastle.crypto.params.KeyParameter
import org.bouncycastle.crypto.params.ParametersWithIV
import org.bouncycastle.math.ec.rfc7748.X25519

internal object NaclBox {
    const val NONCE_LEN = 24
    private const val KEY_LEN = 32
    private const val TAG_LEN = 16
    private val SIGMA = "expand 32-byte k".toByteArray(Charsets.US_ASCII)

    fun generateSecretKey(random: SecureRandom): ByteArray =
        ByteArray(KEY_LEN).also { X25519.generatePrivateKey(random, it) }

    fun publicKey(secretKey: ByteArray): ByteArray =
        ByteArray(KEY_LEN).also { X25519.generatePublicKey(secretKey, 0, it, 0) }

    fun open(
        box: ByteArray,
        nonce: ByteArray,
        peerPublicKey: ByteArray,
        secretKey: ByteArray,
    ): ByteArray? {
        if (box.size < TAG_LEN || nonce.size != NONCE_LEN || peerPublicKey.size != KEY_LEN) {
            return null
        }
        val shared = ByteArray(KEY_LEN)
        if (!X25519.calculateAgreement(secretKey, 0, peerPublicKey, 0, shared, 0)) return null
        val stream = XSalsa20Engine()
        stream.init(false, ParametersWithIV(KeyParameter(hsalsa20(shared)), nonce))
        val macKey = ByteArray(KEY_LEN)
        stream.processBytes(ByteArray(KEY_LEN), 0, KEY_LEN, macKey, 0)
        val tag = box.copyOfRange(0, TAG_LEN)
        val ciphertext = box.copyOfRange(TAG_LEN, box.size)
        val expected = ByteArray(TAG_LEN)
        Poly1305().run {
            init(KeyParameter(macKey))
            update(ciphertext, 0, ciphertext.size)
            doFinal(expected, 0)
        }
        if (!MessageDigest.isEqual(tag, expected)) return null
        return ByteArray(ciphertext.size).also {
            stream.processBytes(ciphertext, 0, ciphertext.size, it, 0)
        }
    }

    private fun hsalsa20(key: ByteArray): ByteArray {
        val x = IntArray(16)
        x[0] = littleEndian(SIGMA, 0)
        x[5] = littleEndian(SIGMA, 4)
        x[10] = littleEndian(SIGMA, 8)
        x[15] = littleEndian(SIGMA, 12)
        for (i in 0 until 4) {
            x[1 + i] = littleEndian(key, 4 * i)
            x[11 + i] = littleEndian(key, 16 + 4 * i)
        }
        repeat(10) {
            quarterRound(x, 0, 4, 8, 12)
            quarterRound(x, 5, 9, 13, 1)
            quarterRound(x, 10, 14, 2, 6)
            quarterRound(x, 15, 3, 7, 11)
            quarterRound(x, 0, 1, 2, 3)
            quarterRound(x, 5, 6, 7, 4)
            quarterRound(x, 10, 11, 8, 9)
            quarterRound(x, 15, 12, 13, 14)
        }
        val out = ByteArray(KEY_LEN)
        intArrayOf(0, 5, 10, 15, 6, 7, 8, 9).forEachIndexed { i, word ->
            for (b in 0 until 4) out[4 * i + b] = (x[word] ushr (8 * b)).toByte()
        }
        return out
    }

    private fun quarterRound(x: IntArray, a: Int, b: Int, c: Int, d: Int) {
        x[b] = x[b] xor (x[a] + x[d]).rotateLeft(7)
        x[c] = x[c] xor (x[b] + x[a]).rotateLeft(9)
        x[d] = x[d] xor (x[c] + x[b]).rotateLeft(13)
        x[a] = x[a] xor (x[d] + x[c]).rotateLeft(18)
    }

    private fun littleEndian(bytes: ByteArray, offset: Int): Int =
        (0 until 4).fold(0) { acc, i -> acc or ((bytes[offset + i].toInt() and 0xff) shl (8 * i)) }
}
