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
    val Add = line("Add", "M12 5.5v13M5.5 12h13")
    val Back = line("Back", "M19 12H5.5M11 18.5 4.5 12 11 5.5")
    val Close = line("Close", "M7 7l10 10M17 7 7 17")
    val Account =
        line("Account", circle(12f, 8.5f, 3.5f) + "M5.5 19.5c1.2-3 3.7-4.5 6.5-4.5s5.3 1.5 6.5 4.5")
    val Completed = line("Completed", rect(3.5f, 3.5f, 17f, 17f, 5f) + "M8.5 12.5l2.5 2.5 4.5-5")
    val Filter = line("Filter", circle(10.5f, 10.5f, 6.5f) + "M15.5 15.5 20 20")
    val Link = line("Link", "M8 16 16 8M10 8h6v6", width = 2.6f)
    val Outdent =
        line(
            "Outdent",
            "M18 9v5a3.5 3.5 0 0 1-3.5 3.5H5M8.5 14 5 17.5 8.5 21",
            dots = circle(18f, 5.5f, 0.75f),
        )
    val Indent =
        line(
            "Indent",
            "M6 9v5a3.5 3.5 0 0 0 3.5 3.5H19M15.5 14l3.5 3.5-3.5 3.5",
            dots = circle(6f, 5.5f, 0.75f),
        )
    val MoveUp = line("MoveUp", "M12 19V5.5M6.5 11 12 5.5l5.5 5.5")
    val MoveDown = line("MoveDown", "M12 5v13.5M6.5 13l5.5 5.5 5.5-5.5")
    val MoveTo =
        line(
            "MoveTo",
            "M5.5 15v-3.5a5 5 0 0 1 5-5H19M15.5 3 19 6.5 15.5 10",
            dots = circle(5.5f, 18.5f, 0.75f),
        )
    val Fold = line("Fold", "M7.5 15.5 12 20l4.5-4.5M7.5 8.5 12 4l4.5 4.5")
    val Note =
        line(
            "Note",
            rect(4.5f, 3.5f, 15f, 17f, 4f) + "M12.5 9h3M9 13.5h6.5M9 17h3.5",
            dots = circle(9f, 9f, 0.75f),
        )
    val Delete =
        line(
            "Delete",
            "M4.5 6.5h15M9.5 6.5V5A1.5 1.5 0 0 1 11 3.5h2A1.5 1.5 0 0 1 14.5 5v1.5" +
                "M6.5 6.5l.7 11.2a3 3 0 0 0 3 2.8h3.6a3 3 0 0 0 3-2.8l.7-11.2M10.25 10.5V16M13.75 10.5V16",
        )
    val Undo = line("Undo", "M9 14.5 4.5 10 9 5.5M4.5 10H14a5 5 0 0 1 0 10h-3")
    val HideKeyboard =
        line(
            "HideKeyboard",
            rect(3f, 3.5f, 18f, 12f, 3.5f) + "M8.5 11.5h7M9 18.5l3 2.5 3-2.5",
            dots = circle(7.5f, 7.5f, 0.5f) + circle(12f, 7.5f, 0.5f) + circle(16.5f, 7.5f, 0.5f),
        )
    val Tick = line("Tick", "M5 12l5 5 9-10", width = 3.6f)
    val Selected = line("Selected", "M5 12l5 5 9-10", width = 2.4f)

    private fun rect(x: Float, y: Float, w: Float, h: Float, r: Float) =
        "M${x + r} ${y}h${w - 2 * r}a$r $r 0 0 1 $r ${r}v${h - 2 * r}a$r $r 0 0 1 -$r ${r}" +
            "h-${w - 2 * r}a$r $r 0 0 1 -$r -${r}v-${h - 2 * r}a$r $r 0 0 1 $r -${r}z"

    private fun circle(cx: Float, cy: Float, r: Float) =
        "M${cx - r} ${cy}a$r $r 0 1 0 ${2 * r} 0a$r $r 0 1 0 -${2 * r} 0z"

    private fun line(name: String, path: String, dots: String? = null, width: Float = 2f) =
        ImageVector.Builder(
                name = name,
                defaultWidth = 24.dp,
                defaultHeight = 24.dp,
                viewportWidth = 24f,
                viewportHeight = 24f,
            )
            .addPath(
                pathData = addPathNodes(path),
                stroke = SolidColor(Color.Black),
                strokeLineWidth = width,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
            .apply {
                if (dots != null) {
                    addPath(
                        pathData = addPathNodes(dots),
                        fill = SolidColor(Color.Black),
                        stroke = SolidColor(Color.Black),
                        strokeLineWidth = width,
                    )
                }
            }
            .build()
}
