package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ProfileTest {
    @Test
    fun `a photo is cut to its middle square, so a face in the center stays in the picture`() {
        assertEquals(Square(left = 500, top = 0, size = 3000), centerSquare(4000, 3000))
        assertEquals(Square(left = 0, top = 420, size = 1080), centerSquare(1080, 1920))
        assertEquals(Square(left = 0, top = 0, size = 256), centerSquare(256, 256))
    }

    @Test
    fun `a large photo is decoded at a fraction of its size to save memory, but never below the picture's size so it stays sharp`() {
        assertEquals(8, sampleSize(4000, 3000))
        assertEquals(2, sampleSize(512, 900))
        assertEquals(1, sampleSize(511, 900))
        assertEquals(1, sampleSize(100, 100))
    }

    @Test
    fun `an extreme panorama is decoded at most 2048 pixels long, so picking it cannot run the phone out of memory`() {
        val sample = sampleSize(100_000, 300)
        assertEquals(64, sample)
        assertTrue(100_000 / sample <= MAX_DECODED_SIDE)
    }

    @Test
    fun `a picture is decoded only as large as the size it is shown at, so a huge upload does not fill memory`() {
        assertEquals(16, sampleSize(4096, 4096, target = 192))
        assertEquals(1, sampleSize(256, 256, target = 192))
    }

    @Test
    fun `two downloads of the same picture count as one, so the account button is not decoded and redrawn again`() {
        assertEquals(
            Avatar("/api/v1/avatars/1", byteArrayOf(1, 2)),
            Avatar("/api/v1/avatars/1", byteArrayOf(1, 2)),
        )
    }
}
