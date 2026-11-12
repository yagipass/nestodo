package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import io.github.yagipass.nestodo.R

val Figtree =
    FontFamily(
        Font(R.font.figtree_bold, FontWeight.Bold),
        Font(R.font.figtree_extrabold, FontWeight.ExtraBold),
    )

private val LightColors =
    lightColorScheme(
        primary = Color(0xFF53419B),
        onPrimary = Color(0xFFFFFFFF),
        secondaryContainer = Color(0xFFECE5FD),
        onSecondaryContainer = Color(0xFF53419B),
        background = Color(0xFFF1EDFA),
        onBackground = Color(0xFF1E1A2B),
        surface = Color(0xFFF1EDFA),
        onSurface = Color(0xFF1E1A2B),
        onSurfaceVariant = Color(0xFF665F78),
        surfaceContainerLowest = Color(0xFFFFFFFF),
        surfaceContainerLow = Color(0xFFFFFFFF),
        surfaceContainer = Color(0xFFFFFFFF),
        surfaceContainerHigh = Color(0xFFFFFFFF),
        surfaceContainerHighest = Color(0xFFF4F0FC),
        outline = Color(0xFF665F78),
        outlineVariant = Color(0xFFE2DBF1),
        error = Color(0xFFC4372B),
        onError = Color(0xFFFFFFFF),
    )

private val DarkColors =
    darkColorScheme(
        primary = Color(0xFFCDBDFF),
        onPrimary = Color(0xFF2A2150),
        secondaryContainer = Color(0xFF33295A),
        onSecondaryContainer = Color(0xFFCDBDFF),
        background = Color(0xFF17122A),
        onBackground = Color(0xFFF3EEFF),
        surface = Color(0xFF17122A),
        onSurface = Color(0xFFF3EEFF),
        onSurfaceVariant = Color(0xFFA69EC0),
        surfaceContainerLowest = Color(0xFF211A36),
        surfaceContainerLow = Color(0xFF2B2345),
        surfaceContainer = Color(0xFF211A36),
        surfaceContainerHigh = Color(0xFF2B2345),
        surfaceContainerHighest = Color(0xFF382F55),
        outline = Color(0xFFA69EC0),
        outlineVariant = Color(0xFF382F55),
        error = Color(0xFFF07468),
        onError = Color(0xFF17122A),
    )

@Immutable
data class ChartColors(
    val added: Color,
    val addedStrong: Color,
    val completedStrong: Color,
    val grid: Color,
    val heat: List<Color>,
)

private val LightChart =
    ChartColors(
        added = Color(0xFFE07A2E),
        addedStrong = Color(0xFFB05A1C),
        completedStrong = Color(0xFF3B2C7A),
        grid = Color(0xFFEFEBF7),
        heat =
            listOf(
                Color(0xFFEEE9F8),
                Color(0xFFDCD2F8),
                Color(0xFFAE99EE),
                Color(0xFF7C61D2),
                Color(0xFF4A3A8C),
            ),
    )

private val DarkChart =
    ChartColors(
        added = Color(0xFFE8894A),
        addedStrong = Color(0xFFF6B585),
        completedStrong = Color(0xFFECE5FF),
        grid = Color(0xFF2E2648),
        heat =
            listOf(
                Color(0xFF2B2345),
                Color(0xFF3F3470),
                Color(0xFF5E4DA6),
                Color(0xFF8F79E0),
                Color(0xFFCDBDFF),
            ),
    )

val LocalChartColors = staticCompositionLocalOf { LightChart }

val LocalGuideColor = staticCompositionLocalOf { Color(0xFFCDBCFA) }

@Composable
fun NestodoTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalChartColors provides if (darkTheme) DarkChart else LightChart,
        LocalGuideColor provides if (darkTheme) Color(0xFF54469A) else Color(0xFFCDBCFA),
    ) {
        MaterialTheme(colorScheme = if (darkTheme) DarkColors else LightColors, content = content)
    }
}
