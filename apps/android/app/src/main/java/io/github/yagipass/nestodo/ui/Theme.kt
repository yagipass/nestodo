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

private val LightColors =
    lightColorScheme(
        primary = Color(0xFF2F6FDB),
        onPrimary = Color(0xFFFFFFFF),
        secondaryContainer = Color(0xFFDDE7FA),
        onSecondaryContainer = Color(0xFF1C1C1A),
        background = Color(0xFFF6F6F4),
        onBackground = Color(0xFF1C1C1A),
        surface = Color(0xFFF6F6F4),
        onSurface = Color(0xFF1C1C1A),
        onSurfaceVariant = Color(0xFF6B6B66),
        surfaceContainerLowest = Color(0xFFFFFFFF),
        surfaceContainerLow = Color(0xFFFFFFFF),
        surfaceContainer = Color(0xFFFFFFFF),
        surfaceContainerHigh = Color(0xFFFFFFFF),
        surfaceContainerHighest = Color(0xFFEFEFEB),
        outline = Color(0xFF8A8A84),
        outlineVariant = Color(0xFFE2E2DD),
        error = Color(0xFFC4372B),
        onError = Color(0xFFFFFFFF),
    )

private val DarkColors =
    darkColorScheme(
        primary = Color(0xFF6B9CF0),
        onPrimary = Color(0xFF0B2A5C),
        secondaryContainer = Color(0xFF2A3D5E),
        onSecondaryContainer = Color(0xFFECECE8),
        background = Color(0xFF141413),
        onBackground = Color(0xFFECECE8),
        surface = Color(0xFF141413),
        onSurface = Color(0xFFECECE8),
        onSurfaceVariant = Color(0xFF9A9A93),
        surfaceContainerLowest = Color(0xFF1E1E1C),
        surfaceContainerLow = Color(0xFF2A2A27),
        surfaceContainer = Color(0xFF1E1E1C),
        surfaceContainerHigh = Color(0xFF2A2A27),
        surfaceContainerHighest = Color(0xFF33332F),
        outline = Color(0xFF8F8F88),
        outlineVariant = Color(0xFF33332F),
        error = Color(0xFFF07468),
        onError = Color(0xFF141413),
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
        added = Color(0xFFEB6834),
        addedStrong = Color(0xFFB8461A),
        completedStrong = Color(0xFF1D4FA8),
        grid = Color(0xFFECECE8),
        heat =
            listOf(
                Color(0xFFEBEBE7),
                Color(0xFFB7D3F6),
                Color(0xFF6DA7EC),
                Color(0xFF2A78D6),
                Color(0xFF184F95),
            ),
    )

private val DarkChart =
    ChartColors(
        added = Color(0xFFD95926),
        addedStrong = Color(0xFFFEA382),
        completedStrong = Color(0xFFA3C2F7),
        grid = Color(0xFF2C2C29),
        heat =
            listOf(
                Color(0xFF2A2A27),
                Color(0xFF184F95),
                Color(0xFF256ABF),
                Color(0xFF3987E5),
                Color(0xFF86B6EF),
            ),
    )

val LocalChartColors = staticCompositionLocalOf { LightChart }

@Composable
fun NestodoTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalChartColors provides if (darkTheme) DarkChart else LightChart) {
        MaterialTheme(colorScheme = if (darkTheme) DarkColors else LightColors, content = content)
    }
}
