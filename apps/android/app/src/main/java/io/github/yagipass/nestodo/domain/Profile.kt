package io.github.yagipass.nestodo.domain

import kotlinx.serialization.Serializable

const val AVATAR_SIZE = 256

const val MAX_DECODED_SIDE = 2048

@Serializable
data class Profile(val email: String, val name: String? = null, val avatarUrl: String? = null)

class Avatar(val url: String, val bytes: ByteArray) {
    override fun equals(other: Any?): Boolean = other is Avatar && other.url == url

    override fun hashCode(): Int = url.hashCode()
}

data class Square(val left: Int, val top: Int, val size: Int)

fun centerSquare(width: Int, height: Int): Square {
    val size = minOf(width, height)
    return Square((width - size) / 2, (height - size) / 2, size)
}

fun sampleSize(
    width: Int,
    height: Int,
    target: Int = AVATAR_SIZE,
    maxSide: Int = MAX_DECODED_SIDE,
): Int {
    var sample = 1
    while (
        minOf(width, height) / (sample * 2) >= target || maxOf(width, height) / sample > maxSide
    ) {
        sample *= 2
    }
    return sample
}
