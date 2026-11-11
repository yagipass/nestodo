package io.github.yagipass.nestodo.ui

import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.foundation.text.input.then
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.IconToggleButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.layout
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.InterceptPlatformTextInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.PlatformTextInputInterceptor
import androidx.compose.ui.platform.PlatformTextInputMethodRequest
import androidx.compose.ui.platform.PlatformTextInputSession
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat
import io.github.yagipass.nestodo.R
import io.github.yagipass.nestodo.TodoUiState
import io.github.yagipass.nestodo.TodoViewModel
import io.github.yagipass.nestodo.domain.Caret
import io.github.yagipass.nestodo.domain.Link
import io.github.yagipass.nestodo.domain.MAX_NOTE_LENGTH
import io.github.yagipass.nestodo.domain.MAX_TITLE_LENGTH
import io.github.yagipass.nestodo.domain.OutlineRow
import io.github.yagipass.nestodo.domain.caretIn
import io.github.yagipass.nestodo.domain.destinations
import io.github.yagipass.nestodo.domain.encodePaste
import io.github.yagipass.nestodo.domain.findLinks
import io.github.yagipass.nestodo.domain.singleLine
import kotlin.math.ceil
import kotlin.math.floor
import kotlinx.coroutines.flow.filterNotNull

private val RowHeight = 48.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OutlineScreen(
    state: TodoUiState,
    viewModel: TodoViewModel,
    snackbar: SnackbarHostState,
    onOpenLink: (String) -> Unit,
) {
    val focusManager = LocalFocusManager.current
    val rows = state.rows
    val editing = rows.find { it.key == state.editing }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        AppIcon(28.dp)
                        Text(stringResource(R.string.app_name))
                    }
                },
                actions = {
                    if (editing == null && state.canUndo) {
                        IconButton(onClick = viewModel::undo) {
                            Icon(NestodoIcons.Undo, contentDescription = "Undo")
                        }
                    }
                    IconToggleButton(
                        checked = state.showCompleted,
                        onCheckedChange = { viewModel.toggleShowCompleted() },
                        colors =
                            IconButtonDefaults.iconToggleButtonColors(
                                checkedContentColor = MaterialTheme.colorScheme.primary
                            ),
                        modifier = Modifier.focusProperties { canFocus = false },
                    ) {
                        Icon(NestodoIcons.Completed, contentDescription = "Show completed")
                    }
                    AccountMenu(
                        email = state.email,
                        server = state.server,
                        onStats = viewModel::openStats,
                        onChangeServer = viewModel::changeServer,
                        onSignOut = viewModel::signOut,
                    )
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.background
                    ),
            )
        },
        bottomBar = {
            if (editing != null) {
                EditToolbar(editing, state.inNote, state.canUndo, viewModel) {
                    focusManager.clearFocus()
                }
            }
        },
        floatingActionButton = {
            if (editing == null && state.loaded) {
                FloatingActionButton(
                    onClick = viewModel::add,
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary,
                ) {
                    Icon(NestodoIcons.Add, contentDescription = "Add todo")
                }
            }
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (state.loaded) {
                OutlineList(rows, state, viewModel, onOpenLink)
            } else {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    CircularProgressIndicator()
                }
            }
        }
    }

    state.moving?.let { MovePicker(state, it.key, viewModel) }
    state.deleting?.let { DeleteDialog(state, it.key, viewModel) }
}

@Composable
private fun DeleteDialog(state: TodoUiState, deleting: Long, viewModel: TodoViewModel) {
    val colors = MaterialTheme.colorScheme
    val name = state.outline[deleting]?.title?.ifEmpty { null } ?: "empty item"
    val under = state.outline.subtree(deleting).size - 1

    AlertDialog(
        onDismissRequest = viewModel::cancelDelete,
        title = {
            Text(
                if (under == 0) "Delete this todo?"
                else
                    "Delete this todo and the $under ${if (under == 1) "item" else "items"} under it?"
            )
        },
        text = {
            Text(
                remember(name, colors.primary) { shownText(name, colors.primary) },
                maxLines = 3,
                overflow = TextOverflow.Ellipsis,
            )
        },
        confirmButton = {
            TextButton(onClick = viewModel::confirmDelete) {
                Text(if (under == 0) "Delete" else "Delete all")
            }
        },
        dismissButton = { TextButton(onClick = viewModel::cancelDelete) { Text("Cancel") } },
    )
}

@Composable
private fun MovePicker(state: TodoUiState, moving: Long, viewModel: TodoViewModel) {
    val colors = MaterialTheme.colorScheme
    val query = remember { TextFieldState() }
    val found = state.outline.destinations(state.viewport, moving, query.text.toString())
    val name = state.outline[moving]?.title?.ifEmpty { null } ?: "empty item"

    Dialog(
        onDismissRequest = viewModel::cancelMove,
        properties =
            DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        val view = LocalView.current
        val light = !isSystemInDarkTheme()
        SideEffect {
            (view.parent as? DialogWindowProvider)?.window?.let { window ->
                WindowCompat.getInsetsController(window, view).apply {
                    isAppearanceLightStatusBars = light
                    isAppearanceLightNavigationBars = light
                }
            }
        }
        val requester = remember { FocusRequester() }
        LaunchedEffect(Unit) { requester.requestFocus() }
        Surface(Modifier.fillMaxSize(), color = colors.background) {
            Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
                Row(
                    Modifier.fillMaxWidth()
                        .padding(start = 4.dp, end = 16.dp, top = 8.dp, bottom = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    IconButton(onClick = viewModel::cancelMove) {
                        Icon(NestodoIcons.Back, contentDescription = "Back")
                    }
                    SearchField(
                        query = query,
                        label = "Move \"$name\" to",
                        modifier = Modifier.weight(1f).focusRequester(requester),
                    )
                }
                HorizontalDivider(color = colors.outlineVariant)
                LazyColumn(
                    Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(vertical = 4.dp),
                ) {
                    if (found.isEmpty()) item { NoMatches() }
                    items(found, key = { it.key }) { destination ->
                        DestinationRow(destination.title, destination.path) {
                            viewModel.moveTo(destination.key)
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun SearchField(query: TextFieldState, label: String, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    BasicTextField(
        state = query,
        modifier = modifier.semantics { contentDescription = label },
        textStyle = typography.bodyLarge.copy(color = colors.onSurface),
        lineLimits = TextFieldLineLimits.SingleLine,
        cursorBrush = SolidColor(colors.primary),
        decorator = { inner ->
            Box(contentAlignment = Alignment.CenterStart) {
                if (query.text.isEmpty()) {
                    Text(
                        "$label…",
                        style = typography.bodyLarge,
                        color = colors.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                inner()
            }
        },
    )
}

@Composable
internal fun DestinationRow(title: String, path: List<String>, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    Column(
        Modifier.fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 20.dp, vertical = 10.dp)
    ) {
        Text(
            remember(title, colors.primary) { shownText(title, colors.primary) },
            style = typography.bodyLarge,
            color = colors.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        if (path.isNotEmpty()) {
            Text(
                path.joinToString(" › "),
                style = typography.bodySmall,
                color = colors.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
internal fun NoMatches() {
    Text(
        "No matches.",
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
    )
}

@Composable
private fun AccountMenu(
    email: String?,
    server: String?,
    onStats: () -> Unit,
    onChangeServer: () -> Unit,
    onSignOut: () -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { expanded = true }) {
            Icon(NestodoIcons.Account, contentDescription = "Account")
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            listOfNotNull(email, server).forEach {
                DropdownMenuItem(text = { Text(it) }, onClick = {}, enabled = false)
            }
            DropdownMenuItem(
                text = { Text("Stats") },
                onClick = {
                    expanded = false
                    onStats()
                },
            )
            DropdownMenuItem(
                text = { Text("Change server") },
                onClick = {
                    expanded = false
                    onChangeServer()
                },
            )
            if (email != null) {
                DropdownMenuItem(
                    text = { Text("Sign out") },
                    onClick = {
                        expanded = false
                        onSignOut()
                    },
                )
            }
        }
    }
}

@Composable
private fun OutlineList(
    rows: List<OutlineRow>,
    state: TodoUiState,
    viewModel: TodoViewModel,
    onOpenLink: (String) -> Unit,
) {
    val listState = rememberLazyListState()
    val entrance = remember { Animatable(0f) }
    val rowHeight = with(LocalDensity.current) { RowHeight.toPx() }

    LaunchedEffect(Unit) { entrance.animateTo(1f, tween(180, easing = LinearOutSlowInEasing)) }
    LaunchedEffect(state.caret) {
        val caret = state.caret ?: return@LaunchedEffect
        val index = rows.indexOfFirst { it.key == caret.key }
        val visible = listState.layoutInfo.visibleItemsInfo
        if (index < 0 || visible.any { it.index == index }) return@LaunchedEffect
        val below = index > (visible.lastOrNull()?.index ?: -1)
        listState.scrollToItem(index)
        if (below) {
            val item = listState.layoutInfo.visibleItemsInfo.firstOrNull { it.index == index }
            val target = listState.layoutInfo.viewportSize.height - 2 * rowHeight
            if (item != null && item.offset < target) listState.scrollBy(item.offset - target)
        }
    }

    LazyColumn(
        state = listState,
        contentPadding = PaddingValues(start = 8.dp, end = 8.dp, top = 4.dp, bottom = 120.dp),
        modifier =
            Modifier.fillMaxSize().graphicsLayer {
                translationX = (1f - entrance.value) * 24.dp.toPx()
                alpha = 0.4f + 0.6f * entrance.value
            },
    ) {
        if (state.outline.nodes.isEmpty()) {
            item {
                Text(
                    "No todos yet.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().padding(top = 48.dp),
                )
            }
        }
        items(rows, key = { it.key }, contentType = { "row" }) { row ->
            OutlineItem(
                row = row,
                revision = state.revision,
                caret = state.caret?.takeIf { it.key == row.key },
                noteOpen = state.noteOpen == row.key,
                viewModel = viewModel,
                onOpenLink = onOpenLink,
            )
        }
    }
}

internal data class Typed(val text: String, val selection: TextRange, val composition: TextRange?)

internal class Field(
    text: String,
    private val revision: () -> Long,
    singleLine: Boolean,
    maxLength: Int,
) {
    val state = TextFieldState(text, TextRange(text.length))
    private var typedIn = revision()
    private var sent = typed()

    val input: InputTransformation =
        state.typing(singleLine, maxLength).then(InputTransformation { typedIn = revision() })

    fun follow(text: String) {
        if (typedIn == revision() || state.text.contentEquals(text)) return
        state.setTextAndPlaceCursorAtEnd(text)
        sent = typed()
    }

    fun place(position: Int) {
        val at = caretIn(state.text.toString(), position)
        state.edit { selection = TextRange(at) }
        typedIn = revision()
        sent = sent.copy(selection = state.selection)
    }

    fun take(): Typed? {
        val now = typed()
        if (now == sent) return null
        sent = now
        return now
    }

    fun typed() = Typed(state.text.toString(), state.selection, state.composition)
}

internal fun TextFieldState.typing(singleLine: Boolean, maxLength: Int): InputTransformation {
    val field = this
    return InputTransformation {
        clean(composing = field.composition != null, singleLine, maxLength)
    }
}

private fun TextFieldBuffer.clean(composing: Boolean, oneLine: Boolean, maxLength: Int) {
    if (!composing) {
        val before = originalText.toString()
        encodePaste(before, originalSelection.min, originalSelection.max, toString())?.let {
            replace(0, length, it.text)
            selection = TextRange(it.caret)
        }
    }
    if (oneLine) {
        val text = toString()
        val line = singleLine(text)
        if (line != text) {
            fun at(index: Int) = singleLine(text.take(index)).length
            val start = at(selection.start)
            val end = at(selection.end)
            replace(0, length, line)
            selection = TextRange(start, end)
        }
    }
    if (length > maxLength) revertAllChanges()
}

internal fun keepInView(cursor: Float, value: Int, viewport: Int, max: Int, slack: Float): Int {
    val low = ceil(cursor + slack - viewport).toInt()
    val high = floor(cursor - slack).toInt()
    return value.coerceIn(low, maxOf(low, high)).coerceIn(0, max.coerceAtLeast(0))
}

private class EdgeScroll {
    val scroll = ScrollState(0)
    private var layout by mutableStateOf<(() -> TextLayoutResult?)?>(null)
    val onTextLayout: Density.(() -> TextLayoutResult?) -> Unit = { layout = it }

    suspend fun follow(state: TextFieldState, active: () -> Boolean) {
        snapshotFlow { target(state, active()) }
            .filterNotNull()
            .collect { (_, to) -> scroll.scrollTo(to) }
    }

    private fun target(state: TextFieldState, active: Boolean): Pair<TextRange, Int>? {
        val result = layout?.invoke() ?: return null
        val selection = state.selection
        val viewport = scroll.viewportSize
        if (!active || !selection.collapsed || viewport <= 0) return null
        val at = selection.start.coerceIn(0, result.layoutInput.text.length)
        val slack = minOf((result.getLineBottom(0) - result.getLineTop(0)) / 2, viewport / 4f)
        val value = Snapshot.withoutReadObservation { scroll.value }
        val cursor = result.getCursorRect(at).left
        return selection to keepInView(cursor, value, viewport, scroll.maxValue, slack)
    }
}

@Composable
private fun OutlineItem(
    row: OutlineRow,
    revision: Long,
    caret: Caret?,
    noteOpen: Boolean,
    viewModel: TodoViewModel,
    onOpenLink: (String) -> Unit,
) {
    val key = row.key
    val colors = MaterialTheme.colorScheme
    val latestRevision by rememberUpdatedState(revision)
    val latestTitle by rememberUpdatedState(row.title)
    val latestNote by rememberUpdatedState(row.note)
    val title = remember {
        Field(row.title, { latestRevision }, singleLine = true, maxLength = MAX_TITLE_LENGTH)
    }
    val note = remember {
        Field(row.note, { latestRevision }, singleLine = false, maxLength = MAX_NOTE_LENGTH)
    }
    var focused by remember { mutableStateOf(false) }
    val requester = remember { FocusRequester() }
    val titleScroll = remember { EdgeScroll() }

    fun sendTitle(select: Boolean = true) {
        val typed = title.take() ?: return
        viewModel.setTitle(key, typed.text, typed.composition != null)
        if (select) viewModel.select(key, typed.selection.min)
    }

    val interceptor =
        remember(viewModel, key) {
            DeleteAtStart {
                sendTitle()
                viewModel.deleteBackward(key)
            }
        }
    val shown = remember(row.title, colors.primary) { shownTitle(row.title, colors.primary) }
    val latestShown by rememberUpdatedState(shown)
    val chips = remember { Chips() }
    var noteFocused by remember { mutableStateOf(false) }
    val noteRequester = remember { FocusRequester() }

    fun sendNote(select: Boolean = true) {
        val typed = note.take() ?: return
        viewModel.setNote(key, typed.text, typed.composition != null)
        if (select) viewModel.selectNote(key, typed.selection.min)
    }

    val shownNote = remember(row.note, colors.primary) { shownTitle(row.note, colors.primary) }
    val latestShownNote by rememberUpdatedState(shownNote)
    var noteLayout by remember { mutableStateOf<TextLayoutResult?>(null) }

    LaunchedEffect(revision, row.title) { title.follow(row.title) }
    LaunchedEffect(revision, row.note) { note.follow(row.note) }
    LaunchedEffect(title) { snapshotFlow { title.typed() }.collect { if (focused) sendTitle() } }
    LaunchedEffect(titleScroll) { titleScroll.follow(title.state) { focused } }
    LaunchedEffect(note) { snapshotFlow { note.typed() }.collect { if (noteFocused) sendNote() } }
    LaunchedEffect(caret) {
        if (caret == null) return@LaunchedEffect
        if (caret.inNote) {
            note.follow(latestNote)
            note.place(caret.position)
            noteRequester.requestFocus()
        } else {
            title.follow(latestTitle)
            title.place(caret.position)
            requester.requestFocus()
        }
        viewModel.caretShown(caret)
    }
    DisposableEffect(key) {
        onDispose {
            if (focused) {
                sendTitle(select = false)
                viewModel.blur(key)
            }
            if (noteFocused) {
                sendNote(select = false)
                viewModel.blurNote(key)
            }
        }
    }

    val textColor = if (row.completed || row.dimmed) colors.onSurfaceVariant else colors.onSurface
    val style =
        MaterialTheme.typography.bodyLarge.copy(
            textDecoration = if (row.completed) TextDecoration.LineThrough else null
        )

    val guide = colors.outlineVariant
    Row(
        Modifier.fillMaxWidth()
            .heightIn(min = RowHeight)
            .background(
                if (focused || noteFocused) colors.secondaryContainer.copy(alpha = 0.6f)
                else Color.Transparent,
                RoundedCornerShape(8.dp),
            )
            .drawBehind {
                repeat(row.depth) { level ->
                    val x = (level * 24 + 21).dp.toPx()
                    drawLine(
                        guide,
                        Offset(x, 0f),
                        Offset(x, size.height),
                        strokeWidth = 1.dp.toPx(),
                    )
                }
            }
    ) {
        Spacer(Modifier.width(24.dp * row.depth))
        Bullet(row) { viewModel.toggleCollapsed(key) }
        Column(Modifier.weight(1f)) {
            Box(Modifier.fillMaxWidth().height(RowHeight)) {
                InterceptPlatformTextInput(interceptor) {
                    BasicTextField(
                        state = title.state,
                        modifier =
                            Modifier.fillMaxSize()
                                .focusRequester(requester)
                                .onFocusChanged {
                                    if (it.isFocused == focused) return@onFocusChanged
                                    if (it.isFocused) {
                                        focused = true
                                        viewModel.select(key, title.state.selection.min)
                                    } else {
                                        sendTitle(select = false)
                                        focused = false
                                        viewModel.blur(key)
                                    }
                                }
                                .onPreviewKeyEvent {
                                    sendTitle()
                                    outlineKey(it, title.state, key, viewModel)
                                }
                                .semantics { contentDescription = "Level ${row.depth + 1}" },
                        inputTransformation = title.input,
                        textStyle =
                            style.copy(color = if (focused) textColor else Color.Transparent),
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                        onKeyboardAction = {
                            sendTitle()
                            viewModel.enter(key, title.state.selection.min)
                        },
                        lineLimits = TextFieldLineLimits.SingleLine,
                        onTextLayout = titleScroll.onTextLayout,
                        cursorBrush = SolidColor(colors.primary),
                        scrollState = titleScroll.scroll,
                        decorator = { inner ->
                            Box(
                                Modifier.fillMaxSize().padding(horizontal = 4.dp),
                                contentAlignment = Alignment.CenterStart,
                            ) {
                                inner()
                                if (!focused) {
                                    val chipColor = colors.surfaceContainerHighest
                                    Text(
                                        shown.text,
                                        style = style,
                                        color = textColor,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        onTextLayout = { chips.layout = it },
                                        modifier =
                                            Modifier.clearAndSetSemantics {}
                                                .onGloballyPositioned { chips.text = it }
                                                .drawBehind {
                                                    chips.bounds(shown.links).forEach { (box, _) ->
                                                        drawRoundRect(
                                                            chipColor,
                                                            box.topLeft,
                                                            box.size,
                                                            CornerRadius(6.dp.toPx()),
                                                        )
                                                    }
                                                },
                                    )
                                }
                            }
                        },
                    )
                }
                if (!focused) {
                    Box(
                        Modifier.matchParentSize()
                            .onGloballyPositioned { chips.touch = it }
                            .pointerInput(onOpenLink) {
                                detectTapGestures { tap ->
                                    chips.linkAt(tap, latestShown.links)?.let {
                                        return@detectTapGestures onOpenLink(it)
                                    }
                                    title.follow(latestTitle)
                                    title.place(title.state.text.length)
                                    requester.requestFocus()
                                }
                            }
                    )
                }
            }
            if (row.note.isNotEmpty() || noteOpen) {
                val noteStyle = MaterialTheme.typography.bodyMedium
                val noteColor = if (noteFocused) colors.onSurface else colors.onSurfaceVariant
                val chipColor = colors.surfaceContainerHighest
                Box(
                    Modifier.fillMaxWidth()
                        .pullUp(8.dp)
                        .padding(start = 4.dp, end = 8.dp, bottom = 10.dp)
                ) {
                    BasicTextField(
                        state = note.state,
                        modifier =
                            Modifier.fillMaxWidth()
                                .focusRequester(noteRequester)
                                .onFocusChanged {
                                    if (it.isFocused == noteFocused) return@onFocusChanged
                                    if (it.isFocused) {
                                        noteFocused = true
                                        viewModel.selectNote(key, note.state.selection.min)
                                    } else {
                                        sendNote(select = false)
                                        noteFocused = false
                                        viewModel.blurNote(key)
                                    }
                                }
                                .onPreviewKeyEvent {
                                    sendNote()
                                    noteKey(it, note.state, viewModel)
                                }
                                .semantics { contentDescription = "Note" },
                        inputTransformation = note.input,
                        textStyle =
                            noteStyle.copy(
                                color = if (noteFocused) noteColor else Color.Transparent
                            ),
                        lineLimits =
                            TextFieldLineLimits.MultiLine(
                                maxHeightInLines = if (noteFocused) Int.MAX_VALUE else 2
                            ),
                        cursorBrush = SolidColor(colors.primary),
                        decorator = { inner ->
                            Box {
                                inner()
                                if (!noteFocused) {
                                    Text(
                                        shownNote.text,
                                        style = noteStyle,
                                        color = noteColor,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                        onTextLayout = { noteLayout = it },
                                        modifier =
                                            Modifier.clearAndSetSemantics {}
                                                .drawBehind {
                                                    val layout = noteLayout ?: return@drawBehind
                                                    val end =
                                                        layout.getLineEnd(
                                                            layout.lineCount - 1,
                                                            visibleEnd = true,
                                                        )
                                                    shownNote.links
                                                        .filter { it.start < end }
                                                        .forEach {
                                                            drawPath(
                                                                layout.getPathForRange(
                                                                    it.start,
                                                                    minOf(it.end, end),
                                                                ),
                                                                chipColor,
                                                            )
                                                        }
                                                },
                                    )
                                }
                            }
                        },
                    )
                    if (!noteFocused) {
                        Box(
                            Modifier.matchParentSize().pointerInput(onOpenLink) {
                                detectTapGestures { tap ->
                                    val shown = latestShownNote
                                    val at = noteLayout?.getOffsetForPosition(tap)
                                    if (at != null) {
                                        shown.links
                                            .firstOrNull { at in it.start until it.end }
                                            ?.let {
                                                return@detectTapGestures onOpenLink(it.url)
                                            }
                                    }
                                    title.follow(latestTitle)
                                    title.place(title.state.text.length)
                                    requester.requestFocus()
                                    viewModel.select(key, title.state.text.length)
                                }
                            }
                        )
                    }
                }
            }
        }
    }
}

private fun Modifier.pullUp(by: Dp): Modifier = layout { measurable, constraints ->
    val placeable = measurable.measure(constraints)
    val shift = by.roundToPx()
    layout(placeable.width, (placeable.height - shift).coerceAtLeast(0)) {
        placeable.place(0, -shift)
    }
}

@Composable
private fun Bullet(row: OutlineRow, onToggle: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val dot = if (row.completed) colors.outline else colors.onSurface.copy(alpha = 0.85f)
    val halo = colors.outlineVariant
    val name = row.title.ifEmpty { "empty item" }
    val toggle =
        if (row.hasChildren) {
            Modifier.focusProperties { canFocus = false }
                .clickable(
                    interactionSource = null,
                    indication = ripple(bounded = false, radius = 20.dp),
                    role = Role.Button,
                    onClick = onToggle,
                )
                .semantics {
                    contentDescription = "${if (row.collapsed) "Expand" else "Collapse"} $name"
                }
        } else {
            Modifier
        }
    Box(
        Modifier.width(44.dp).height(RowHeight).then(toggle),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(7.dp)) {
            if (row.collapsed && row.hasChildren) {
                drawCircle(halo, radius = size.minDimension / 2 + 6.dp.toPx())
            }
            drawCircle(dot)
        }
    }
}

@Composable
private fun EditToolbar(
    row: OutlineRow,
    inNote: Boolean,
    canUndo: Boolean,
    viewModel: TodoViewModel,
    onHideKeyboard: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Surface(color = colors.surfaceContainerLowest, shadowElevation = 3.dp) {
        Column(Modifier.windowInsetsPadding(WindowInsets.ime.union(WindowInsets.navigationBars))) {
            HorizontalDivider(color = colors.outlineVariant)
            Row(
                modifier =
                    Modifier.fillMaxWidth()
                        .padding(start = 6.dp, end = 6.dp, top = 4.dp, bottom = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(2.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ToolButton(NestodoIcons.Outdent, "Outdent", onClick = viewModel::outdent)
                ToolButton(NestodoIcons.Indent, "Indent", onClick = viewModel::indent)
                ToolButton(NestodoIcons.MoveUp, "Move up", onClick = viewModel::moveUp)
                ToolButton(NestodoIcons.MoveDown, "Move down", onClick = viewModel::moveDown)
                ToolButton(NestodoIcons.MoveTo, "Move to", onClick = viewModel::startMove)
                ToolSeparator()
                ToolButton(
                    NestodoIcons.Completed,
                    if (row.completed) "Mark as not done" else "Complete",
                    onClick = viewModel::toggleCompleted,
                )
                ToolButton(
                    NestodoIcons.Fold,
                    if (row.collapsed) "Expand" else "Collapse",
                    enabled = row.hasChildren,
                    onClick = viewModel::toggleCollapsed,
                )
                ToolButton(
                    NestodoIcons.Note,
                    if (inNote) "Back to title" else "Note",
                    selected = inNote,
                    onClick = viewModel::toggleNote,
                )
                ToolButton(NestodoIcons.Delete, "Delete", onClick = viewModel::startDelete)
                ToolSeparator()
                ToolButton(NestodoIcons.Undo, "Undo", enabled = canUndo, onClick = viewModel::undo)
                ToolButton(NestodoIcons.HideKeyboard, "Hide keyboard", onClick = onHideKeyboard)
            }
        }
    }
}

@Composable
private fun RowScope.ToolButton(
    icon: ImageVector,
    label: String,
    enabled: Boolean = true,
    selected: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Box(
        modifier =
            Modifier.weight(1f)
                .height(48.dp)
                .clip(RoundedCornerShape(12.dp))
                .background(if (selected) colors.secondaryContainer else Color.Transparent)
                .focusProperties { canFocus = false }
                .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                .semantics { contentDescription = label },
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            icon,
            contentDescription = null,
            tint =
                when {
                    !enabled -> colors.onSurface.copy(alpha = 0.38f)
                    selected -> colors.onSecondaryContainer
                    else -> colors.onSurface
                },
            modifier = Modifier.size(22.dp),
        )
    }
}

@Composable
private fun ToolSeparator() {
    Box(Modifier.width(1.dp).height(24.dp).background(MaterialTheme.colorScheme.outlineVariant))
}

private fun isUndo(event: KeyEvent): Boolean =
    event.key == Key.Z && event.isCtrlPressed && !event.isShiftPressed && !event.isAltPressed

private fun outlineKey(
    event: KeyEvent,
    value: TextFieldState,
    key: Long,
    viewModel: TodoViewModel,
): Boolean {
    val down = event.type == KeyEventType.KeyDown
    val vertical = event.key == Key.DirectionUp || event.key == Key.DirectionDown
    return when {
        isUndo(event) -> {
            if (down && value.composition == null) viewModel.undo()
            true
        }
        event.key == Key.Enter || event.key == Key.NumPadEnter -> {
            if (value.composition != null) return false
            if (down) {
                when {
                    event.isCtrlPressed -> viewModel.toggleCompleted()
                    event.isShiftPressed -> viewModel.toggleNote()
                    else -> viewModel.enter(key, value.selection.min)
                }
            }
            true
        }
        event.key == Key.Tab -> {
            if (down) {
                if (event.isShiftPressed) viewModel.outdent() else viewModel.indent()
            }
            true
        }
        vertical && event.isAltPressed && event.isShiftPressed -> {
            if (down) {
                if (event.key == Key.DirectionUp) viewModel.moveUp() else viewModel.moveDown()
            }
            true
        }
        event.key == Key.Backspace &&
            down &&
            value.composition == null &&
            value.selection == TextRange.Zero -> viewModel.deleteBackward(key)
        else -> false
    }
}

private fun noteKey(event: KeyEvent, value: TextFieldState, viewModel: TodoViewModel): Boolean {
    val down = event.type == KeyEventType.KeyDown
    val enter = event.key == Key.Enter || event.key == Key.NumPadEnter
    val vertical = event.key == Key.DirectionUp || event.key == Key.DirectionDown
    return when {
        isUndo(event) -> {
            if (down && value.composition == null) viewModel.undo()
            true
        }
        enter && (event.isShiftPressed || event.isCtrlPressed) -> {
            if (value.composition != null) return false
            if (down) {
                if (event.isCtrlPressed) viewModel.toggleCompleted() else viewModel.toggleNote()
            }
            true
        }
        event.key == Key.Escape -> {
            if (down) viewModel.toggleNote()
            true
        }
        event.key == Key.Tab -> {
            if (down) {
                if (event.isShiftPressed) viewModel.outdent() else viewModel.indent()
            }
            true
        }
        vertical && event.isAltPressed && event.isShiftPressed -> {
            if (down) {
                if (event.key == Key.DirectionUp) viewModel.moveUp() else viewModel.moveDown()
            }
            true
        }
        else -> false
    }
}

private class ShownTitle(val text: AnnotatedString, val links: List<Link>)

internal fun shownText(text: String, accent: Color): AnnotatedString = shownTitle(text, accent).text

private fun shownTitle(title: String, accent: Color): ShownTitle {
    val links = mutableListOf<Link>()
    val text = buildAnnotatedString {
        var at = 0
        for (link in findLinks(title)) {
            append(title.substring(at, link.start))
            val from = length
            withStyle(SpanStyle(color = accent)) { append("\u2009${link.label} ↗\u2009") }
            links += Link(from, length, link.url)
            at = link.end
        }
        append(title.substring(at))
    }
    return ShownTitle(text, links)
}

private class Chips {
    var layout by mutableStateOf<TextLayoutResult?>(null)
    var text: LayoutCoordinates? = null
    var touch: LayoutCoordinates? = null

    fun bounds(links: List<Link>): List<Pair<Rect, String>> {
        val layout = layout?.takeIf { links.isNotEmpty() } ?: return emptyList()
        val visible = layout.getLineEnd(0, visibleEnd = true)
        return links
            .filter { it.start < visible }
            .map { layout.getPathForRange(it.start, minOf(it.end, visible)).getBounds() to it.url }
    }

    fun linkAt(tap: Offset, links: List<Link>): String? {
        val text = text?.takeIf { it.isAttached } ?: return null
        val touch = touch?.takeIf { it.isAttached } ?: return null
        val x = text.localPositionOf(touch, tap).x
        return bounds(links).firstOrNull { (box, _) -> x in box.left..box.right }?.second
    }
}

private class DeleteAtStart(private val onDelete: () -> Boolean) : PlatformTextInputInterceptor {
    override suspend fun interceptStartInputMethod(
        request: PlatformTextInputMethodRequest,
        nextHandler: PlatformTextInputSession,
    ): Nothing =
        nextHandler.startInputMethod(
            object : PlatformTextInputMethodRequest {
                override fun createInputConnection(outAttributes: EditorInfo): InputConnection =
                    StartOfTextConnection(request.createInputConnection(outAttributes), onDelete)
            }
        )
}

private class StartOfTextConnection(target: InputConnection, private val onDelete: () -> Boolean) :
    InputConnectionWrapper(target, false) {
    override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean =
        deletesBeforeStart(beforeLength, afterLength) ||
            super.deleteSurroundingText(beforeLength, afterLength)

    override fun deleteSurroundingTextInCodePoints(beforeLength: Int, afterLength: Int): Boolean =
        deletesBeforeStart(beforeLength, afterLength) ||
            super.deleteSurroundingTextInCodePoints(beforeLength, afterLength)

    private fun deletesBeforeStart(beforeLength: Int, afterLength: Int): Boolean =
        beforeLength > 0 &&
            afterLength == 0 &&
            getTextBeforeCursor(1, 0).isNullOrEmpty() &&
            getSelectedText(0).isNullOrEmpty() &&
            onDelete()
}
