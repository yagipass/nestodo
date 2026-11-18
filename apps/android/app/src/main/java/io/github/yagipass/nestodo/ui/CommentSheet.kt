package io.github.yagipass.nestodo.ui

import android.view.KeyCharacterMap
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.github.yagipass.nestodo.CommentsState
import io.github.yagipass.nestodo.TodoUiState
import io.github.yagipass.nestodo.TodoViewModel
import io.github.yagipass.nestodo.domain.Avatar
import io.github.yagipass.nestodo.domain.MAX_COMMENT_LENGTH
import io.github.yagipass.nestodo.domain.Pending
import io.github.yagipass.nestodo.domain.Stamped
import io.github.yagipass.nestodo.domain.TimelineItem
import io.github.yagipass.nestodo.domain.Via
import io.github.yagipass.nestodo.domain.byDay
import io.github.yagipass.nestodo.domain.nextMidnight
import java.time.Duration
import java.time.Instant
import java.time.ZoneId
import kotlin.math.roundToInt
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

internal sealed interface LogLine {
    val key: String

    data class Date(val label: String, override val key: String) : LogLine

    data class Item(
        val stamped: Stamped,
        val first: Boolean,
        val last: Boolean,
        override val key: String,
    ) : LogLine

    data object Unwritten : LogLine {
        override val key = "unwritten"
    }

    data object Unloaded : LogLine {
        override val key = "unloaded"
    }
}

internal fun logLines(comments: CommentsState, now: Instant, zone: ZoneId): List<LogLine> =
    buildList {
        byDay(comments.entries, now, zone).forEach { day ->
            add(LogLine.Date(day.label, "date:$size"))
            day.entries.forEachIndexed { index, stamped ->
                val key =
                    when (val entry = stamped.entry) {
                        is TimelineItem.Comment -> "comment:${entry.id}"
                        is Pending -> "pending:${entry.key}"
                        else -> "event:$size"
                    }
                add(LogLine.Item(stamped, index == 0, index == day.entries.lastIndex, key))
            }
        }
        if (comments.items != null && !comments.written) add(LogLine.Unwritten)
        if (comments.failed) add(LogLine.Unloaded)
    }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CommentSheet(
    state: TodoUiState,
    comments: CommentsState,
    viewModel: TodoViewModel,
    onOpenLink: (String) -> Unit,
) {
    val node = state.outline[comments.key] ?: return
    val colors = MaterialTheme.colorScheme
    val sheetState = rememberModalBottomSheetState()
    val scope = rememberCoroutineScope()
    val expand: () -> Unit = { scope.launch { sheetState.expand() } }
    val path =
        generateSequence(node.parent) { state.outline[it]?.parent }
            .mapNotNull { state.outline[it]?.title?.ifEmpty { "Untitled" } }
            .toList()
            .asReversed()

    ModalBottomSheet(onDismissRequest = viewModel::closeComments, sheetState = sheetState) {
        Column(Modifier.fillMaxWidth().fillMaxHeight().inView(sheetState).imePadding()) {
            Header(path, node.title, node.completed, node.comments) {
                scope
                    .launch { sheetState.hide() }
                    .invokeOnCompletion {
                        if (!sheetState.isVisible) viewModel.closeComments()
                    }
            }
            HorizontalDivider(color = colors.outlineVariant)
            Log(comments, state.avatar, viewModel, onOpenLink, expand, Modifier.weight(1f))
            HorizontalDivider(color = colors.outlineVariant)
            Composer(viewModel, expand)
        }
        comments.deleting?.let { DeleteCommentDialog(it, viewModel) }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
private fun Modifier.inView(sheet: SheetState): Modifier = layout { measurable, constraints ->
    val hidden = if (sheet.hasExpandedState) sheet.requireOffset().roundToInt() else 0
    val shown = (constraints.maxHeight - hidden).coerceIn(0, constraints.maxHeight)
    val placeable = measurable.measure(constraints.copy(minHeight = shown, maxHeight = shown))
    layout(placeable.width, constraints.maxHeight) { placeable.place(0, 0) }
}

@Composable
private fun Header(
    path: List<String>,
    title: String,
    done: Boolean,
    count: Int,
    onClose: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    val name = title.ifEmpty { "Untitled" }
    Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 8.dp, bottom = 14.dp)) {
        Column(Modifier.weight(1f).padding(top = 4.dp)) {
            if (path.isNotEmpty()) {
                Text(
                    path.joinToString(" › "),
                    style = typography.bodySmall,
                    color = colors.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
            }
            Text(
                remember(name, colors.primary) { shownText(name, colors.primary) },
                style = typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
                color = colors.onSurface,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
                inlineContent = LinkIcon,
            )
            Row(
                Modifier.padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Tag(if (done) "Completed" else "Open", strong = done)
                Text(
                    when (count) {
                        0 -> "No comments yet"
                        1 -> "1 comment"
                        else -> "$count comments"
                    },
                    style = typography.labelMedium,
                    color = colors.onSurfaceVariant,
                )
            }
        }
        IconButton(onClick = onClose) {
            Icon(NestodoIcons.Close, contentDescription = "Close comments")
        }
    }
}

@Composable
private fun Tag(text: String, strong: Boolean = false) {
    val colors = MaterialTheme.colorScheme
    Text(
        text,
        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
        color = if (strong) colors.onPrimary else colors.primary,
        modifier =
            Modifier.background(
                    if (strong) colors.primary else colors.secondaryContainer,
                    RoundedCornerShape(6.dp),
                )
                .padding(horizontal = 8.dp, vertical = 2.dp),
    )
}

@Composable
private fun Log(
    comments: CommentsState,
    avatar: Avatar?,
    viewModel: TodoViewModel,
    onOpenLink: (String) -> Unit,
    onEditing: () -> Unit,
    modifier: Modifier,
) {
    val zone = ZoneId.systemDefault()
    val now by
        produceState(Instant.now(), zone) {
            while (true) {
                delay(Duration.between(Instant.now(), nextMidnight(value, zone)).toMillis())
                value = Instant.now()
            }
        }
    val lines =
        remember(comments.items, comments.local, comments.failed, now) {
                logLines(comments, now, zone)
            }
            .asReversed()
    val listState = rememberLazyListState()
    LaunchedEffect(lines.size) { listState.scrollToItem(0) }

    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        state = listState,
        reverseLayout = true,
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
    ) {
        items(lines.size, key = { lines[it].key }) { index ->
            when (val line = lines[index]) {
                is LogLine.Date -> DateLine(line.label)
                is LogLine.Item ->
                    when (val entry = line.stamped.entry) {
                        is TimelineItem.Event -> EventLine(entry, line)
                        is TimelineItem.Comment ->
                            CommentLine(
                                entry,
                                line,
                                avatar,
                                comments.editing == entry.id,
                                viewModel,
                                onOpenLink,
                                onEditing,
                            )
                        is Pending -> PendingLine(entry, line, avatar, onOpenLink)
                    }
                LogLine.Unwritten -> Notice("Nothing written yet. Note what you did or found.")
                LogLine.Unloaded ->
                    Notice("Couldn't load the comments.") {
                        TextButton(onClick = viewModel::retryComments) { Text("Try again") }
                    }
            }
        }
    }
}

@Composable
private fun DateLine(label: String) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().padding(top = 16.dp, bottom = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HorizontalDivider(Modifier.weight(1f), color = colors.outlineVariant)
        Text(
            label,
            style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
            color = colors.onSurfaceVariant,
        )
        HorizontalDivider(Modifier.weight(1f), color = colors.outlineVariant)
    }
}

private val RailAt = 17.dp

private fun Modifier.rail(line: LogLine.Item, color: Color) = drawBehind {
    val x = 14.dp.toPx()
    val top = if (line.first) RailAt.toPx() else 0f
    val bottom = if (line.last) RailAt.toPx() else size.height
    if (bottom > top) drawLine(color, Offset(x, top), Offset(x, bottom), 2.dp.toPx())
}

@Composable
private fun EventLine(event: TimelineItem.Event, line: LogLine.Item) {
    val colors = MaterialTheme.colorScheme
    val (label, icon) =
        when (event.kind) {
            TimelineItem.Event.Kind.Created -> "Created" to NestodoIcons.Add
            TimelineItem.Event.Kind.Completed -> "Completed" to NestodoIcons.Tick
            TimelineItem.Event.Kind.Reopened -> "Reopened" to NestodoIcons.Reopen
        }
    val (fill, tint) =
        when (event.kind) {
            TimelineItem.Event.Kind.Created -> colors.secondaryContainer to colors.primary
            TimelineItem.Event.Kind.Completed -> colors.primary to colors.onPrimary
            TimelineItem.Event.Kind.Reopened -> colors.surfaceContainerLow to colors.primary
        }
    Row(
        Modifier.fillMaxWidth().heightIn(min = 34.dp).rail(line, colors.secondaryContainer),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            Modifier.padding(horizontal = 3.dp)
                .size(22.dp)
                .clip(CircleShape)
                .background(fill)
                .then(
                    if (event.kind == TimelineItem.Event.Kind.Reopened) {
                        Modifier.border(2.dp, LocalGuideColor.current, CircleShape)
                    } else {
                        Modifier
                    }
                ),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(12.dp))
        }
        Text(
            buildAnnotatedString {
                withStyle(SpanStyle(color = colors.onSurface, fontWeight = FontWeight.SemiBold)) {
                    append(label)
                }
                append(" at ${line.stamped.time}")
            },
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
            color = colors.onSurfaceVariant,
        )
    }
}

@Composable
private fun Portrait(avatar: Avatar?, via: Via) {
    val colors = MaterialTheme.colorScheme
    val bot = via == Via.Mcp
    Box(
        Modifier.padding(top = 3.dp)
            .size(28.dp)
            .clip(CircleShape)
            .background(if (bot) colors.surfaceContainerLow else colors.secondaryContainer)
            .then(
                if (bot) Modifier.border(1.5.dp, LocalGuideColor.current, CircleShape) else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        CompositionLocalProvider(LocalContentColor provides colors.primary) {
            if (bot) {
                Icon(NestodoIcons.Bot, contentDescription = null, modifier = Modifier.size(16.dp))
            } else {
                AccountPicture(avatar, size = 28.dp, iconSize = 18.dp, contentDescription = null)
            }
        }
    }
}

@Composable
private fun CommentLine(
    comment: TimelineItem.Comment,
    line: LogLine.Item,
    avatar: Avatar?,
    editing: Boolean,
    viewModel: TodoViewModel,
    onOpenLink: (String) -> Unit,
    onEditing: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().rail(line, colors.secondaryContainer),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Portrait(avatar, comment.via)
        Column(Modifier.weight(1f).padding(bottom = 14.dp)) {
            Row(
                Modifier.heightIn(min = 34.dp),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (comment.via == Via.Mcp) Tag("via MCP")
                Meta(line.stamped.time + if (comment.editedAt != null) " (edited)" else "")
                Spacer(Modifier.weight(1f))
                if (!editing) {
                    IconButton(
                        onClick = { viewModel.editComment(comment.id) },
                        modifier = Modifier.size(36.dp),
                    ) {
                        Icon(
                            NestodoIcons.Edit,
                            contentDescription = "Edit this comment",
                            tint = colors.onSurfaceVariant,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(
                        onClick = { viewModel.startDeleteComment(comment) },
                        modifier = Modifier.size(36.dp),
                    ) {
                        Icon(
                            NestodoIcons.Delete,
                            contentDescription = "Delete this comment",
                            tint = colors.onSurfaceVariant,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
            if (editing) {
                CommentEditor(comment, viewModel, onEditing)
            } else {
                MarkdownText(comment.body, onOpenLink)
            }
        }
    }
}

@Composable
private fun PendingLine(
    pending: Pending,
    line: LogLine.Item,
    avatar: Avatar?,
    onOpenLink: (String) -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().rail(line, colors.secondaryContainer),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.alpha(0.6f)) { Portrait(avatar, Via.App) }
        Column(Modifier.weight(1f).padding(bottom = 14.dp).alpha(0.6f)) {
            Row(Modifier.heightIn(min = 34.dp), verticalAlignment = Alignment.CenterVertically) {
                Meta("Sending…")
            }
            MarkdownText(pending.body, onOpenLink)
        }
    }
}

@Composable
private fun Meta(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun Notice(text: String, action: (@Composable () -> Unit)? = null) {
    Row(
        Modifier.fillMaxWidth().padding(start = 40.dp, top = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodySmall.copy(fontSize = 13.sp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        action?.invoke()
    }
}

@Composable
private fun CommentField(
    state: TextFieldState,
    label: String,
    placeholder: String?,
    modifier: Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val style = MaterialTheme.typography.bodyMedium
    val input = remember(state) { state.typing(singleLine = false, maxLength = MAX_COMMENT_LENGTH) }
    var focused by remember { mutableStateOf(false) }
    BasicTextField(
        state = state,
        modifier =
            modifier
                .onFocusChanged { focused = it.isFocused }
                .semantics { contentDescription = label },
        inputTransformation = input,
        textStyle = style.copy(color = colors.onSurface),
        lineLimits = TextFieldLineLimits.MultiLine(maxHeightInLines = 6),
        cursorBrush = SolidColor(colors.primary),
        decorator = { inner ->
            Box(
                Modifier.heightIn(min = 48.dp)
                    .border(
                        1.5.dp,
                        if (focused) colors.primary else colors.outlineVariant,
                        RoundedCornerShape(12.dp),
                    )
                    .background(colors.surfaceContainerLowest, RoundedCornerShape(12.dp))
                    .padding(horizontal = 12.dp, vertical = 12.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (placeholder != null && state.text.isEmpty()) {
                    Text(placeholder, style = style, color = colors.onSurfaceVariant)
                }
                inner()
            }
        },
    )
}

internal fun isPostKey(
    enter: Boolean,
    shift: Boolean,
    ctrl: Boolean,
    meta: Boolean,
    alt: Boolean,
    hardware: Boolean,
) = enter && !shift && !alt && (hardware || ctrl || meta)

private fun submitKey(
    event: KeyEvent,
    field: TextFieldState,
    submit: () -> Unit,
    cancel: (() -> Unit)? = null,
): Boolean {
    val down = event.type == KeyEventType.KeyDown
    val enter = event.key == Key.Enter || event.key == Key.NumPadEnter
    return when {
        isPostKey(
            enter,
            event.isShiftPressed,
            event.isCtrlPressed,
            event.isMetaPressed,
            event.isAltPressed,
            hardware = event.nativeKeyEvent.deviceId != KeyCharacterMap.VIRTUAL_KEYBOARD,
        ) -> {
            if (field.composition != null) return false
            if (down) submit()
            true
        }
        event.key == Key.Escape && cancel != null -> {
            if (down) cancel()
            true
        }
        else -> false
    }
}

@Composable
private fun Composer(viewModel: TodoViewModel, onFocused: () -> Unit) {
    val draft = remember { TextFieldState() }

    fun post() {
        if (viewModel.postComment(draft.text.toString())) draft.clearText()
    }

    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 10.dp, bottom = 12.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.Bottom,
    ) {
        CommentField(
            draft,
            label = "Add a comment",
            placeholder = "Add to the log…",
            modifier =
                Modifier.weight(1f)
                    .onFocusChanged { if (it.isFocused) onFocused() }
                    .onPreviewKeyEvent { submitKey(it, draft, ::post) },
        )
        FilledIconButton(
            onClick = ::post,
            enabled = draft.text.isNotBlank(),
            modifier = Modifier.size(48.dp),
            shape = CircleShape,
        ) {
            Icon(NestodoIcons.Send, contentDescription = "Post", modifier = Modifier.size(22.dp))
        }
    }
}

@Composable
private fun CommentEditor(
    comment: TimelineItem.Comment,
    viewModel: TodoViewModel,
    onFocused: () -> Unit,
) {
    val draft =
        remember(comment.id) { TextFieldState(comment.body, TextRange(comment.body.length)) }
    val requester = remember { FocusRequester() }
    LaunchedEffect(requester) { requester.requestFocus() }

    fun save() {
        viewModel.saveComment(draft.text.toString())
    }

    Column {
        CommentField(
            draft,
            label = "Edit the comment",
            placeholder = null,
            modifier =
                Modifier.fillMaxWidth()
                    .focusRequester(requester)
                    .onFocusChanged { if (it.isFocused) onFocused() }
                    .onPreviewKeyEvent {
                        submitKey(it, draft, ::save, cancel = viewModel::cancelEditComment)
                    },
        )
        Row(
            Modifier.fillMaxWidth().padding(top = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        ) {
            TextButton(onClick = viewModel::cancelEditComment) { Text("Cancel") }
            Button(onClick = ::save, enabled = draft.text.isNotBlank()) { Text("Save") }
        }
    }
}

@Composable
private fun DeleteCommentDialog(comment: TimelineItem.Comment, viewModel: TodoViewModel) {
    val colors = MaterialTheme.colorScheme
    AlertDialog(
        onDismissRequest = viewModel::cancelDeleteComment,
        title = { Text("Delete this comment?") },
        text = {
            Text(
                remember(comment.body, colors.primary) { shownText(comment.body, colors.primary) },
                maxLines = 5,
                overflow = TextOverflow.Ellipsis,
                inlineContent = LinkIcon,
            )
        },
        confirmButton = {
            TextButton(onClick = viewModel::confirmDeleteComment) { Text("Delete") }
        },
        dismissButton = {
            TextButton(onClick = viewModel::cancelDeleteComment) { Text("Cancel") }
        },
    )
}
