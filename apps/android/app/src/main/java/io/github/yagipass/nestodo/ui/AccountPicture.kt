package io.github.yagipass.nestodo.ui

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import io.github.yagipass.nestodo.domain.Avatar
import io.github.yagipass.nestodo.domain.sampleSize

@Composable
internal fun AccountPicture(avatar: Avatar?, size: Dp, iconSize: Dp, contentDescription: String?) {
    val pixels = with(LocalDensity.current) { size.roundToPx() }
    val bitmap = remember(avatar, pixels) { avatar?.let { decode(it.bytes, pixels) } }
    if (bitmap == null) {
        Icon(NestodoIcons.Account, contentDescription, Modifier.size(iconSize))
    } else {
        Image(
            bitmap,
            contentDescription,
            Modifier.size(size).clip(CircleShape),
            contentScale = ContentScale.Crop,
        )
    }
}

private fun decode(bytes: ByteArray, pixels: Int): ImageBitmap? {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
    val options =
        BitmapFactory.Options().apply {
            inSampleSize = sampleSize(bounds.outWidth, bounds.outHeight, target = pixels)
        }
    return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)?.asImageBitmap()
}
