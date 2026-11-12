package io.github.yagipass.nestodo.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.yagipass.nestodo.StatsState
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.RANGES
import io.github.yagipass.nestodo.domain.Summary
import io.github.yagipass.nestodo.domain.heatLevel
import io.github.yagipass.nestodo.domain.summarize
import io.github.yagipass.nestodo.domain.ticks
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.util.Locale

private val ShortDate = DateTimeFormatter.ofPattern("MMM d", Locale.US)
private val LongDate = DateTimeFormatter.ofPattern("EEE, MMM d", Locale.US)
private val Weekday = DateTimeFormatter.ofPattern("EEE", Locale.US)
private val MonthName = DateTimeFormatter.ofPattern("MMM", Locale.US)

private fun signed(value: Int): String =
    when {
        value > 0 -> "+$value"
        value < 0 -> "−${-value}"
        else -> "±0"
    }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun StatsScreen(
    stats: StatsState,
    snackbar: SnackbarHostState,
    onBack: () -> Unit,
    onRetry: () -> Unit,
) {
    BackHandler(onBack = onBack)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Stats") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(NestodoIcons.Back, contentDescription = "Back")
                    }
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.background
                    ),
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            when (stats) {
                StatsState.Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                is StatsState.Failed ->
                    Column(
                        Modifier.align(Alignment.Center).padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(stats.message, textAlign = TextAlign.Center)
                        TextButton(onClick = onRetry) { Text("Retry") }
                    }
                is StatsState.Loaded -> StatsContent(stats.days)
            }
        }
    }
}

@Composable
private fun StatsContent(days: List<Day>) {
    var range by rememberSaveable { mutableIntStateOf(30) }
    val dates = remember(days) { days.map { LocalDate.parse(it.date) } }
    Column(
        Modifier.fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 24.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Calendar(days, dates)
        RangePicker(range) { range = it }
        Tiles(summarize(days, range), range)
        AddedCompleted(days.takeLast(range), dates.takeLast(range))
    }
}

@Composable
private fun RangePicker(range: Int, onPick: (Int) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(10.dp)
    Row(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        RANGES.forEach { value ->
            val selected = value == range
            Row(
                Modifier.minimumInteractiveComponentSize()
                    .height(36.dp)
                    .clip(shape)
                    .border(1.dp, if (selected) colors.primary else colors.outlineVariant, shape)
                    .background(
                        if (selected) colors.secondaryContainer else colors.surfaceContainer
                    )
                    .selectable(selected, role = Role.RadioButton, onClick = { onPick(value) })
                    .padding(start = if (selected) 10.dp else 14.dp, end = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (selected) {
                    Icon(
                        NestodoIcons.Selected,
                        contentDescription = null,
                        tint = colors.primary,
                        modifier = Modifier.size(16.dp),
                    )
                }
                Text(
                    "$value days",
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (selected) colors.primary else colors.onSurfaceVariant,
                    fontWeight = if (selected) FontWeight.Medium else null,
                )
            }
        }
    }
}

@Composable
private fun Tiles(totals: Summary, range: Int) {
    val period = "$range days"
    val since = ShortDate.format(LocalDate.parse(totals.since))
    val tiles =
        listOf(
            Triple(
                "Completed",
                "${totals.completed}",
                "${signed(totals.completed - totals.completedBefore)} vs prior $period",
            ),
            Triple(
                "Added",
                "${totals.added}",
                "${signed(totals.added - totals.addedBefore)} vs prior $period",
            ),
            Triple(
                "Open now",
                "${totals.open}",
                "${signed(totals.open - totals.openBefore)} since $since",
            ),
            Triple(
                "Per day",
                String.format(Locale.US, "%.1f", totals.completed.toDouble() / range),
                "completed on average",
            ),
        )
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        tiles.chunked(2).forEach { row ->
            Row(
                Modifier.height(IntrinsicSize.Min),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                row.forEach { (label, value, note) ->
                    Tile(label, value, note, Modifier.weight(1f).fillMaxHeight())
                }
            }
        }
    }
}

@Composable
private fun Tile(label: String, value: String, note: String, modifier: Modifier) {
    val muted = MaterialTheme.colorScheme.onSurfaceVariant
    Surface(
        modifier,
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            Modifier.padding(horizontal = 14.dp, vertical = 12.dp).semantics(
                mergeDescendants = true
            ) {}
        ) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = muted)
            Text(
                value,
                fontFamily = Figtree,
                fontWeight = FontWeight.Bold,
                fontSize = 26.sp,
                lineHeight = 32.sp,
                style = MaterialTheme.typography.headlineMedium.copy(fontFeatureSettings = "tnum"),
            )
            Text(note, style = MaterialTheme.typography.bodySmall, color = muted)
        }
    }
}

@Composable
private fun Panel(content: @Composable ColumnScope.() -> Unit) {
    Surface(
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(
            Modifier.fillMaxWidth()
                .padding(start = 14.dp, top = 14.dp, end = 14.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            content = content,
        )
    }
}

@Composable
private fun Readout(caption: String, text: String) {
    Column(
        Modifier.fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.background)
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .semantics { liveRegion = LiveRegionMode.Polite }
    ) {
        Text(
            caption,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(text, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium)
    }
}

@Composable
private fun Swatch(color: Color, label: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(9.dp).clip(RoundedCornerShape(2.dp)).background(color))
        Text(
            label,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun TickLabel(text: String, modifier: Modifier) {
    Text(
        text,
        modifier,
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun AddedCompleted(days: List<Day>, dates: List<LocalDate>) {
    val chart = LocalChartColors.current
    var selected by rememberSaveable(days.size) { mutableIntStateOf(days.lastIndex) }
    val day = days[selected]
    val date = LongDate.format(dates[selected])
    val top = remember(days) { ticks(days.maxOf { maxOf(it.added, it.completed) }).last() }
    Panel {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                "Added and completed",
                Modifier.weight(1f),
                style = MaterialTheme.typography.titleMedium,
            )
            Column(
                Modifier.clearAndSetSemantics {},
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Swatch(chart.added, "Added ↑")
                Swatch(MaterialTheme.colorScheme.primary, "Completed ↓")
            }
        }
        Readout(
            caption = if (selected == days.lastIndex) "Today · $date" else date,
            text = "${day.added} added · ${day.completed} completed · ${day.open} open",
        )
        Row(Modifier.height(168.dp)) {
            MirroredBars(
                days,
                selected,
                top,
                onSelect = { selected = it },
                Modifier.weight(1f).fillMaxHeight(),
            )
            Box(Modifier.width(24.dp).fillMaxHeight().clearAndSetSemantics {}) {
                TickLabel("$top", Modifier.align(Alignment.TopEnd))
                TickLabel("0", Modifier.align(Alignment.CenterEnd))
                TickLabel("$top", Modifier.align(Alignment.BottomEnd))
            }
        }
        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(
                "Open at the end of each day",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OpenLine(
                days,
                selected,
                onSelect = { selected = it },
                Modifier.padding(end = 24.dp).fillMaxWidth().height(80.dp),
            )
            AxisLabels(dates, Modifier.padding(end = 24.dp).fillMaxWidth().height(16.dp))
        }
    }
}

private fun Modifier.scrub(count: Int, onSelect: (Int) -> Unit): Modifier =
    pointerInput(count) {
            detectTapGestures { onSelect(columnAt(it.x, size.width, count)) }
        }
        .pointerInput(count) {
            detectHorizontalDragGestures(
                onDragStart = { onSelect(columnAt(it.x, size.width, count)) }
            ) { change, _ ->
                onSelect(columnAt(change.position.x, size.width, count))
            }
        }

private fun columnAt(x: Float, width: Int, count: Int): Int =
    (x / width * count).toInt().coerceIn(0, count - 1)

private fun DrawScope.bar(
    left: Float,
    top: Float,
    right: Float,
    bottom: Float,
    color: Color,
    roundTop: Boolean,
) {
    if (bottom <= top) return
    val round = CornerRadius(minOf(3.dp.toPx(), (right - left) / 2))
    val square = CornerRadius.Zero
    val shape =
        RoundRect(
            left,
            top,
            right,
            bottom,
            topLeftCornerRadius = if (roundTop) round else square,
            topRightCornerRadius = if (roundTop) round else square,
            bottomRightCornerRadius = if (roundTop) square else round,
            bottomLeftCornerRadius = if (roundTop) square else round,
        )
    drawPath(Path().apply { addRoundRect(shape) }, color)
}

@Composable
private fun MirroredBars(
    days: List<Day>,
    selected: Int,
    top: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier,
) {
    val chart = LocalChartColors.current
    val colors = MaterialTheme.colorScheme
    Canvas(
        modifier.scrub(days.size, onSelect).semantics {
            contentDescription =
                "Column chart of todos added per day above the line and completed per day below it"
        }
    ) {
        val inset = 4.dp.toPx()
        val middle = size.height / 2
        val half = middle - inset
        val column = size.width / days.size
        val gap =
            when {
                days.size > 30 -> 0.5.dp.toPx()
                days.size > 7 -> 1.dp.toPx()
                else -> 0f
            }
        val width = minOf(column - 2 * gap, 24.dp.toPx())
        for (y in listOf(inset, size.height - inset)) {
            drawLine(chart.grid, Offset(0f, y), Offset(size.width, y), 1.dp.toPx())
        }
        drawRoundRect(
            colors.primary.copy(alpha = 0.1f),
            topLeft = Offset(selected * column, 0f),
            size = Size(column, size.height),
            cornerRadius = CornerRadius(4.dp.toPx()),
        )
        days.forEachIndexed { index, day ->
            val left = (index + 0.5f) * column - width / 2
            val active = index == selected
            bar(
                left,
                middle - day.added.toFloat() / top * half,
                left + width,
                middle,
                if (active) chart.addedStrong else chart.added,
                roundTop = true,
            )
            bar(
                left,
                middle,
                left + width,
                middle + day.completed.toFloat() / top * half,
                if (active) chart.completedStrong else colors.primary,
                roundTop = false,
            )
        }
        drawLine(colors.outlineVariant, Offset(0f, middle), Offset(size.width, middle), 1.dp.toPx())
    }
}

@Composable
private fun OpenLine(days: List<Day>, selected: Int, onSelect: (Int) -> Unit, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    val measurer = rememberTextMeasurer()
    val labelStyle =
        MaterialTheme.typography.labelMedium.copy(color = LocalChartColors.current.completedStrong)
    val top = remember(days) { ticks(days.maxOf { it.open }).last() }
    Canvas(
        modifier.scrub(days.size, onSelect).semantics {
            contentDescription = "Line chart of open todos at the end of each day"
        }
    ) {
        val label = measurer.measure("${days[selected].open} open", labelStyle)
        val gap = 10.dp.toPx()
        val column = size.width / days.size
        val lift = size.height - label.size.height - gap - 7.dp.toPx()
        fun x(index: Int) = (index + 0.5f) * column
        fun y(day: Day) = size.height - day.open.toFloat() / top * lift
        val line = Path()
        val area = Path().apply { moveTo(x(0), size.height) }
        days.forEachIndexed { index, day ->
            if (index == 0) line.moveTo(x(index), y(day)) else line.lineTo(x(index), y(day))
            area.lineTo(x(index), y(day))
        }
        area.lineTo(x(days.lastIndex), size.height)
        area.close()
        drawPath(area, colors.primary.copy(alpha = 0.12f))
        drawPath(line, colors.primary, style = Stroke(2.dp.toPx(), join = StrokeJoin.Round))

        val focus = Offset(x(selected), y(days[selected]))
        drawLine(
            colors.outlineVariant,
            Offset(focus.x, 0f),
            Offset(focus.x, size.height),
            1.dp.toPx(),
        )
        drawCircle(colors.surfaceContainer, 7.dp.toPx(), focus)
        drawCircle(colors.primary, 5.dp.toPx(), focus)
        drawText(
            label,
            topLeft =
                Offset(
                    (focus.x - label.size.width / 2).coerceIn(0f, size.width - label.size.width),
                    focus.y - gap - label.size.height,
                ),
        )
    }
}

@Composable
private fun AxisLabels(dates: List<LocalDate>, modifier: Modifier) {
    val measurer = rememberTextMeasurer()
    val style =
        MaterialTheme.typography.labelSmall.copy(color = MaterialTheme.colorScheme.onSurfaceVariant)
    val every =
        when {
            dates.size <= 7 -> 1
            dates.size <= 30 -> 7
            else -> 14
        }
    val format = if (dates.size <= 7) Weekday else ShortDate
    Canvas(modifier.clearAndSetSemantics {}) {
        val column = size.width / dates.size
        for (index in dates.lastIndex downTo 0 step every) {
            val label = measurer.measure(format.format(dates[index]), style)
            val left =
                ((index + 0.5f) * column - label.size.width / 2).coerceIn(
                    0f,
                    size.width - label.size.width,
                )
            drawText(label, topLeft = Offset(left, 0f))
        }
    }
}

@Composable
private fun Calendar(days: List<Day>, dates: List<LocalDate>) {
    val chart = LocalChartColors.current
    val colors = MaterialTheme.colorScheme
    val measurer = rememberTextMeasurer()
    val labelStyle = MaterialTheme.typography.labelSmall.copy(color = colors.onSurfaceVariant)
    var selected by rememberSaveable { mutableStateOf<Int?>(null) }
    val blanks = dates.first().dayOfWeek.value % 7
    val weeks = (blanks + days.size + 6) / 7
    val busiest = remember(days) { days.maxOf { it.added + it.completed } }
    val picked = selected?.let { days.getOrNull(it) }
    Panel {
        Column {
            Text("Past 26 weeks", style = MaterialTheme.typography.titleMedium)
            Text(
                "Todos added and completed each day",
                style = MaterialTheme.typography.bodySmall,
                color = colors.onSurfaceVariant,
            )
        }
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val gap = 2.dp
            val labels = 24.dp
            val months = 14.dp
            val cell = (maxWidth - labels - gap * weeks) / weeks
            Canvas(
                Modifier.fillMaxWidth()
                    .height(months + gap + cell * 7 + gap * 6)
                    .pointerInput(days) {
                        detectTapGestures { tap ->
                            val step = (size.width - labels.toPx()) / weeks
                            val column = ((tap.x - labels.toPx()) / step).toInt()
                            val row = ((tap.y - months.toPx() - gap.toPx()) / step).toInt()
                            val index = column * 7 + row - blanks
                            if (tap.x >= labels.toPx() && row in 0..6 && index in days.indices) {
                                selected = if (selected == index) null else index
                            }
                        }
                    }
                    .semantics {
                        contentDescription =
                            "Calendar of todos added and completed each day over the past 26 weeks"
                    }
            ) {
                val gapPx = gap.toPx()
                val left = labels.toPx()
                val side = (size.width - left - gapPx * weeks) / weeks
                val step = side + gapPx
                val firstRow = months.toPx() + gapPx
                fun cellAt(index: Int) =
                    Offset(
                        left + gapPx + (blanks + index) / 7 * step,
                        firstRow + (blanks + index) % 7 * step,
                    )
                dates.forEachIndexed { index, date ->
                    if (date.dayOfMonth == 1) {
                        drawText(
                            measurer.measure(MonthName.format(date), labelStyle),
                            topLeft = Offset(cellAt(index).x, 0f),
                        )
                    }
                }
                listOf(1 to "Mon", 3 to "Wed", 5 to "Fri").forEach { (row, name) ->
                    val label = measurer.measure(name, labelStyle)
                    drawText(
                        label,
                        topLeft =
                            Offset(0f, firstRow + row * step + (side - label.size.height) / 2),
                    )
                }
                days.forEachIndexed { index, day ->
                    drawRoundRect(
                        chart.heat[heatLevel(day.added + day.completed, busiest)],
                        topLeft = cellAt(index),
                        size = Size(side, side),
                        cornerRadius = CornerRadius(3.dp.toPx()),
                    )
                }
                selected?.let { index ->
                    val ring = 1.dp.toPx()
                    drawRoundRect(
                        colors.onSurface,
                        topLeft = cellAt(index) - Offset(ring, ring),
                        size = Size(side + 2 * ring, side + 2 * ring),
                        cornerRadius = CornerRadius(4.dp.toPx()),
                        style = Stroke(2.dp.toPx()),
                    )
                }
            }
        }
        Text(
            if (picked != null) {
                "${picked.added} added · ${picked.completed} completed · " +
                    LongDate.format(dates[selected!!])
            } else {
                "${days.sumOf { it.added }} added · ${days.sumOf { it.completed }} completed " +
                    "in the past 26 weeks"
            },
            Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            style = MaterialTheme.typography.bodySmall,
        )
        Row(
            Modifier.align(Alignment.End).clearAndSetSemantics {},
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp),
        ) {
            TickLabel("Less", Modifier)
            chart.heat.forEach {
                Box(Modifier.size(9.dp).clip(RoundedCornerShape(2.dp)).background(it))
            }
            TickLabel("More", Modifier)
        }
    }
}
