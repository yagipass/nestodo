package io.github.yagipass.nestodo.data

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.provider.OpenableColumns
import io.github.yagipass.nestodo.domain.MAX_COMMENT_IMAGE_BYTES
import io.github.yagipass.nestodo.domain.centerSquare
import io.github.yagipass.nestodo.domain.fitWithin
import io.github.yagipass.nestodo.domain.sampleSize
import java.io.ByteArrayOutputStream
import java.io.IOException

private const val IMAGE_QUALITY = 85

private val COMMENT_QUALITIES = listOf(IMAGE_QUALITY, 70, 50)

fun squareImage(resolver: ContentResolver, uri: Uri, size: Int): ByteArray? {
    val decoded =
        decode(resolver, uri) { width, height ->
            setTargetSampleSize(sampleSize(width, height, target = size))
        } ?: return null
    val square = centerSquare(decoded.width, decoded.height)
    val cropped = Bitmap.createBitmap(decoded, square.left, square.top, square.size, square.size)
    val scaled = Bitmap.createScaledBitmap(cropped, size, size, true)
    return encode(scaled, IMAGE_QUALITY)
}

fun commentImage(resolver: ContentResolver, uri: Uri): ByteArray? {
    val decoded =
        decode(resolver, uri) { width, height ->
            val fitted = fitWithin(width, height)
            setTargetSize(fitted.width, fitted.height)
        } ?: return null
    return COMMENT_QUALITIES.firstNotNullOfOrNull { quality ->
        encode(decoded, quality)?.takeIf { it.size <= MAX_COMMENT_IMAGE_BYTES }
    }
}

private fun decode(
    resolver: ContentResolver,
    uri: Uri,
    size: ImageDecoder.(width: Int, height: Int) -> Unit,
): Bitmap? =
    try {
        val source = ImageDecoder.createSource(resolver, uri)
        ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
            decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            decoder.size(info.size.width, info.size.height)
        }
    } catch (_: IOException) {
        null
    } catch (_: IllegalArgumentException) {
        null
    } catch (_: SecurityException) {
        null
    }

private fun encode(bitmap: Bitmap, quality: Int): ByteArray? {
    val out = ByteArrayOutputStream()
    if (!bitmap.compress(Bitmap.CompressFormat.WEBP_LOSSY, quality, out)) return null
    return out.toByteArray()
}

fun fileName(resolver: ContentResolver, uri: Uri): String? =
    try {
        resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst() && !it.isNull(0)) it.getString(0) else null
        }
    } catch (_: IllegalArgumentException) {
        null
    } catch (_: SecurityException) {
        null
    }
