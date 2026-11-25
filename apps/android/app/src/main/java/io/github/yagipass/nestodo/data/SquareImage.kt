package io.github.yagipass.nestodo.data

import android.content.ContentResolver
import android.graphics.Bitmap
import android.graphics.ImageDecoder
import android.net.Uri
import android.provider.OpenableColumns
import io.github.yagipass.nestodo.domain.centerSquare
import io.github.yagipass.nestodo.domain.sampleSize
import java.io.ByteArrayOutputStream
import java.io.IOException

private const val IMAGE_QUALITY = 85

fun squareImage(resolver: ContentResolver, uri: Uri, size: Int): ByteArray? {
    val decoded =
        try {
            val source = ImageDecoder.createSource(resolver, uri)
            ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
                decoder.setTargetSampleSize(
                    sampleSize(info.size.width, info.size.height, target = size)
                )
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
    val scaled = Bitmap.createScaledBitmap(cropped, size, size, true)
    val out = ByteArrayOutputStream()
    if (!scaled.compress(Bitmap.CompressFormat.WEBP_LOSSY, IMAGE_QUALITY, out)) return null
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
