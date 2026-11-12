package io.github.yagipass.nestodo.data

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import io.github.yagipass.nestodo.domain.AVATAR_SIZE
import io.github.yagipass.nestodo.domain.centerSquare
import io.github.yagipass.nestodo.domain.sampleSize
import java.io.ByteArrayOutputStream
import java.io.IOException

private const val AVATAR_QUALITY = 85

fun avatarImage(resolver: ContentResolver, uri: Uri): ByteArray? {
    val decoded =
        try {
            val source = ImageDecoder.createSource(resolver, uri)
            ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                decoder.setTargetSampleSize(sampleSize(info.size.width, info.size.height))
            }
        } catch (_: IOException) {
            return null
        } catch (_: IllegalArgumentException) {
            return null
        } catch (_: SecurityException) {
            return null
        }
    val square = centerSquare(decoded.width, decoded.height)
    val cropped = Bitmap.createBitmap(decoded, square.left, square.top, square.size, square.size)
    val scaled = Bitmap.createScaledBitmap(cropped, AVATAR_SIZE, AVATAR_SIZE, true)
    val out = ByteArrayOutputStream()
    if (!scaled.compress(Bitmap.CompressFormat.WEBP_LOSSY, AVATAR_QUALITY, out)) return null
    return out.toByteArray()
}
