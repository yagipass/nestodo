package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.R

@Composable
internal fun AppIcon(size: Dp) {
    Box(
        modifier =
            Modifier.size(size)
                .clip(RoundedCornerShape(size * 0.225f))
                .background(colorResource(R.color.ic_launcher_background))
    ) {
        Image(
            painterResource(R.drawable.ic_launcher_foreground),
            contentDescription = null,
            modifier = Modifier.fillMaxSize().scale(1.25f),
        )
    }
}

internal object NestodoIcons {
    val Add = icon("Add", "M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z")
    val Back = stroke("Back", "M19 12H5M12 19l-7-7 7-7")
    val Close = stroke("Close", "M18 6L6 18M6 6l12 12")
    val Account =
        icon(
            "Account",
            "M12,12c2.21,0 4,-1.79 4,-4s-1.79,-4 -4,-4 -4,1.79 -4,4 1.79,4 4,4zM12,14c-2.67,0 -8,1.34 -8,4v2h16v-2c0,-2.66 -5.33,-4 -8,-4z",
        )
    val Completed = stroke("Completed", "M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0M8 12l3 3 5-6")
    val Outdent = stroke("Outdent", "M21 5H11M21 12H13M21 19H11M7 8l-4 4 4 4")
    val Indent = stroke("Indent", "M21 5H11M21 12H13M21 19H11M3 8l4 4-4 4")
    val MoveUp = stroke("MoveUp", "M12 19V5M6 11l6-6 6 6")
    val MoveDown = stroke("MoveDown", "M12 5v14M6 13l6 6 6-6")
    val MoveTo = stroke("MoveTo", "M15 10l5 5-5 5M4 4v7a4 4 0 0 0 4 4h12")
    val Fold = stroke("Fold", "M7 15l5 5 5-5M7 9l5-5 5 5")
    val Note = stroke("Note", "M6 3h9l4 4v14H6zM15 3v4h4M9 12h7M9 16h5")
    val Delete = stroke("Delete", "M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7")
    val Undo = stroke("Undo", "M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3")
    val HideKeyboard =
        stroke(
            "HideKeyboard",
            "M5 4h14a2 2 0 0 1 2 2v7a2 2 0 0 1 -2 2H5a2 2 0 0 1 -2 -2V6a2 2 0 0 1 2 -2z" +
                "M7 8h.01M11 8h.01M15 8h.01M8 11h8M9 19l3 2 3-2",
        )

    private fun builder(name: String) =
        ImageVector.Builder(
            name = name,
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        )

    private fun icon(name: String, path: String): ImageVector =
        builder(name).addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black)).build()

    private fun stroke(name: String, path: String): ImageVector =
        builder(name)
            .addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = 2f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
            .build()
}
