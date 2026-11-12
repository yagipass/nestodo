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
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import io.github.yagipass.nestodo.domain.Avatar

@Composable
internal fun AccountPicture(avatar: Avatar?, size: Dp, iconSize: Dp, contentDescription: String?) {
    val bitmap =
        remember(avatar) {
            avatar
                ?.let { BitmapFactory.decodeByteArray(it.bytes, 0, it.bytes.size) }
                ?.asImageBitmap()
        }
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
