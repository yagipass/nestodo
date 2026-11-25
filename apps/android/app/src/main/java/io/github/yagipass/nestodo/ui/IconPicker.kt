package io.github.yagipass.nestodo.ui

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.maxLength
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.ChoosingIcon
import io.github.yagipass.nestodo.IconView
import io.github.yagipass.nestodo.TodoUiState
import io.github.yagipass.nestodo.TodoViewModel
import io.github.yagipass.nestodo.data.fileName
import io.github.yagipass.nestodo.data.squareImage
import io.github.yagipass.nestodo.domain.ICON_IMAGE_SIZE
import io.github.yagipass.nestodo.domain.IconChoices
import io.github.yagipass.nestodo.domain.ImageIcon
import io.github.yagipass.nestodo.domain.MAX_ICON_NAME_LENGTH
import io.github.yagipass.nestodo.domain.MyIcons
import io.github.yagipass.nestodo.domain.Node
import io.github.yagipass.nestodo.domain.OutlineRow
import io.github.yagipass.nestodo.domain.ShownIcon
import io.github.yagipass.nestodo.domain.shownIcon
import io.github.yagipass.nestodo.domain.usesOf
import java.util.WeakHashMap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private val TileShape = RoundedCornerShape(12.dp)

private val PendingIcon = ImageIcon("pending", "", "")

@Stable
internal class IconImages(
    val list: List<ImageIcon>,
    private val bytes: Map<String, ByteArray>,
    private val decoded: MutableMap<ByteArray, ImageBitmap?>,
) {
    fun picture(id: String): ImageBitmap? {
        val image = bytes[id] ?: return null
        if (image !in decoded) decoded[image] = decodePicture(image, ICON_IMAGE_SIZE)
        return decoded[image]
    }

    fun draws(icon: String?): Boolean =
        when (val shown = shownIcon(icon, list)) {
            null -> false
            is ShownIcon.Emoji -> true
            is ShownIcon.Image -> picture(shown.icon.id) != null
        }

    fun with(icon: ImageIcon, image: ByteArray): IconImages =
        IconImages(list + icon, bytes + (icon.id to image), decoded)
}

@Composable
internal fun rememberIconImages(icons: MyIcons): IconImages {
    val decoded = remember { WeakHashMap<ByteArray, ImageBitmap?>() }
    return remember(icons) { IconImages(icons.list, icons.images, decoded) }
}

@Composable
internal fun TodoIcon(
    icon: String?,
    muted: Boolean,
    images: IconImages,
    modifier: Modifier = Modifier,
    size: Dp = 26.dp,
) {
    val shown = shownIcon(icon, images.list) ?: return
    val picture = (shown as? ShownIcon.Image)?.let { images.picture(it.icon.id) ?: return }
    val grayscale = remember {
        ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) })
    }
    val glyph = with(LocalDensity.current) { (size * 0.7f).toSp() }
    Box(modifier, contentAlignment = Alignment.Center) {
        Box(
            Modifier.size(size).graphicsLayer {
                if (muted) {
                    alpha = 0.5f
                    colorFilter = grayscale
                }
            },
            contentAlignment = Alignment.Center,
        ) {
            when {
                picture != null ->
                    Image(
                        picture,
                        contentDescription = null,
                        modifier =
                            Modifier.size(size * 22f / 26f)
                                .clip(RoundedCornerShape(size * 6f / 26f)),
                        contentScale = ContentScale.Crop,
                    )
                shown is ShownIcon.Emoji ->
                    Text(shown.text, fontSize = glyph, lineHeight = glyph, maxLines = 1)
            }
        }
    }
}

internal fun deleteWarning(uses: Int): String =
    when (uses) {
        0 -> "Deleting it cannot be undone."
        1 ->
            "It is the icon of 1 todo. " +
                "Deleting it removes the icon from that todo and cannot be undone."
        else ->
            "It is the icon of $uses todos. " +
                "Deleting it removes the icon from them and cannot be undone."
    }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun IconPicker(
    state: TodoUiState,
    choosing: ChoosingIcon,
    images: IconImages,
    viewModel: TodoViewModel,
) {
    val node = state.outline[choosing.key] ?: return
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val pick =
        rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
            if (uri == null) return@rememberLauncherForActivityResult
            scope.launch {
                val resolver = context.contentResolver
                val image =
                    withContext(Dispatchers.IO) { squareImage(resolver, uri, ICON_IMAGE_SIZE) }
                if (image == null) {
                    viewModel.iconImageUnreadable()
                } else {
                    val name = withContext(Dispatchers.IO) { fileName(resolver, uri) }
                    viewModel.startAddingIcon(image, name.orEmpty())
                }
            }
        }

    ModalBottomSheet(
        onDismissRequest = viewModel::closeIconPicker,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier.fillMaxWidth()
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(start = 16.dp, end = 16.dp, bottom = 16.dp)
        ) {
            when (val view = choosing.view) {
                is IconView.Grid ->
                    IconGrid(node, view, images, viewModel) {
                        pick.launch(
                            PickVisualMediaRequest(
                                ActivityResultContracts.PickVisualMedia.ImageOnly
                            )
                        )
                    }
                is IconView.Adding -> AddImage(node, view, images, viewModel)
                is IconView.Editing -> EditImage(view, images, viewModel)
                is IconView.Deleting ->
                    DeleteImage(view, state.outline.usesOf(view.icon.value), viewModel)
            }
        }
    }
}

@Composable
private fun IconGrid(
    node: Node,
    view: IconView.Grid,
    images: IconImages,
    viewModel: TodoViewModel,
    onAdd: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    val name = node.title.ifEmpty { "empty item" }
    val other = remember { TextFieldState() }
    var refused by remember { mutableStateOf(false) }
    LaunchedEffect(other) { snapshotFlow { other.text.toString() }.collect { refused = false } }

    fun applyOther() {
        if (!viewModel.setIcon(other.text.toString())) refused = true
    }

    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp).padding(start = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            "Icon for $name",
            style = typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
            color = colors.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (node.icon != null) {
            TextButton(onClick = { viewModel.setIcon(null) }) { Text("Remove icon") }
        }
    }
    SectionLabel("My icons")
    (images.list + null).chunked(8).forEach { line ->
        Row(Modifier.fillMaxWidth()) {
            line.forEach { icon ->
                if (icon == null) {
                    AddTile(onAdd)
                } else {
                    ImageTile(
                        icon,
                        selected = icon.value == node.icon,
                        images = images,
                        onChoose = { viewModel.setIcon(icon.value) },
                        onEdit = { viewModel.editIcon(icon.id) },
                    )
                }
            }
            repeat(8 - line.size) { Spacer(Modifier.weight(1f)) }
        }
    }
    if (view.unreadable) Problem("The image could not be read.")
    SectionLabel("Emoji")
    IconChoices.chunked(8).forEach { line ->
        Row(Modifier.fillMaxWidth()) {
            line.forEach { emoji ->
                val selected = emoji == node.icon
                Tile(
                    selected,
                    Modifier.selectable(selected = selected) { viewModel.setIcon(emoji) },
                ) {
                    TodoIcon(emoji, muted = false, images = images, size = 34.dp)
                }
            }
        }
    }
    BasicTextField(
        state = other,
        modifier =
            Modifier.fillMaxWidth()
                .padding(top = 12.dp)
                .onPreviewKeyEvent { fieldKey(it, other, ::applyOther, viewModel::closeIconPicker) }
                .semantics { contentDescription = "Other icon" },
        textStyle = typography.bodyLarge.copy(color = colors.onSurface),
        lineLimits = TextFieldLineLimits.SingleLine,
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        onKeyboardAction = { applyOther() },
        cursorBrush = SolidColor(colors.primary),
        decorator = { inner ->
            Box(
                Modifier.heightIn(min = 48.dp)
                    .border(
                        1.5.dp,
                        if (refused) colors.error else colors.outlineVariant,
                        RoundedCornerShape(12.dp),
                    )
                    .padding(horizontal = 14.dp),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (other.text.isEmpty()) {
                    Text(
                        "Other: type or paste one emoji",
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
    if (refused) Problem("Enter one emoji", start = 14.dp)
}

@Composable
private fun RowScope.Tile(selected: Boolean, modifier: Modifier, content: @Composable () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Box(
        Modifier.weight(1f)
            .height(48.dp)
            .padding(2.dp)
            .clip(TileShape)
            .background(if (selected) colors.secondaryContainer else Color.Transparent)
            .then(if (selected) Modifier.border(2.dp, colors.primary, TileShape) else Modifier)
            .then(modifier),
        contentAlignment = Alignment.Center,
    ) {
        content()
    }
}

@Composable
private fun RowScope.ImageTile(
    icon: ImageIcon,
    selected: Boolean,
    images: IconImages,
    onChoose: () -> Unit,
    onEdit: () -> Unit,
) {
    Tile(
        selected,
        Modifier.combinedClickable(
                onLongClickLabel = "Edit image",
                onLongClick = onEdit,
                onClick = onChoose,
            )
            .semantics {
                this.selected = selected
                contentDescription = icon.name
            },
    ) {
        TodoIcon(icon.value, muted = false, images = images, size = 34.dp)
    }
}

@Composable
private fun RowScope.AddTile(onAdd: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Tile(
        selected = false,
        Modifier.border(1.dp, colors.outlineVariant, TileShape)
            .clickable(role = Role.Button, onClick = onAdd)
            .semantics { contentDescription = "Add image" },
    ) {
        Icon(
            NestodoIcons.Add,
            contentDescription = null,
            tint = colors.onSurfaceVariant,
            modifier = Modifier.size(22.dp),
        )
    }
}

@Composable
private fun AddImage(
    node: Node,
    view: IconView.Adding,
    images: IconImages,
    viewModel: TodoViewModel,
) {
    val colors = MaterialTheme.colorScheme
    val preview = remember(images, view.image) { images.with(PendingIcon, view.image) }
    val name = remember(view.image) { TextFieldState(view.name, TextRange(0, view.name.length)) }
    var empty by remember(view.image) { mutableStateOf(false) }
    LaunchedEffect(name) { snapshotFlow { name.text.toString() }.collect { empty = false } }

    fun add() {
        if (!viewModel.addIcon(name.text.toString())) empty = true
    }

    ViewHeader("Add image", viewModel::backInIconPicker)
    Row(
        Modifier.fillMaxWidth().padding(top = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        LargePicture(preview.picture(PendingIcon.id))
        Column(Modifier.weight(1f)) {
            SectionLabel("In the outline")
            Column(
                Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(12.dp))
                    .background(colors.surfaceContainerLowest)
            ) {
                PreviewRow(node, preview, completed = false)
                PreviewRow(node, preview, completed = true)
            }
        }
    }
    NameField(name, if (empty) "Enter a name" else view.problem, ::add, viewModel::closeIconPicker)
    ViewButtons {
        TextButton(onClick = viewModel::backInIconPicker) { Text("Back") }
        Button(onClick = ::add, enabled = !view.sending) { Text("Add and use") }
    }
}

@Composable
private fun EditImage(view: IconView.Editing, images: IconImages, viewModel: TodoViewModel) {
    val colors = MaterialTheme.colorScheme
    val saved = view.icon.name
    val name = remember(view.icon.id) { TextFieldState(saved, TextRange(saved.length)) }
    var empty by remember(view.icon.id) { mutableStateOf(false) }
    LaunchedEffect(name) { snapshotFlow { name.text.toString() }.collect { empty = false } }

    fun save() {
        if (!viewModel.renameIcon(name.text.toString())) empty = true
    }

    ViewHeader("Edit image", viewModel::backInIconPicker)
    LargePicture(images.picture(view.icon.id))
    NameField(name, if (empty) "Enter a name" else view.problem, ::save, viewModel::closeIconPicker)
    Row(
        Modifier.fillMaxWidth().padding(top = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(
            onClick = viewModel::startDeleteIcon,
            enabled = !view.sending,
            colors = ButtonDefaults.textButtonColors(contentColor = colors.error),
        ) {
            Text("Delete image")
        }
        Spacer(Modifier.weight(1f))
        Button(onClick = ::save, enabled = !view.sending) { Text("Save") }
    }
}

@Composable
private fun DeleteImage(view: IconView.Deleting, uses: Int, viewModel: TodoViewModel) {
    val colors = MaterialTheme.colorScheme
    ViewHeader("Delete \"${view.icon.name}\"?", viewModel::backInIconPicker)
    Text(
        deleteWarning(uses),
        style = MaterialTheme.typography.bodyMedium,
        color = colors.onSurface,
        modifier = Modifier.padding(horizontal = 4.dp),
    )
    view.problem?.let { Problem(it) }
    ViewButtons {
        TextButton(onClick = viewModel::backInIconPicker) { Text("Back") }
        Button(
            onClick = viewModel::deleteIcon,
            enabled = !view.sending,
            colors =
                ButtonDefaults.buttonColors(
                    containerColor = colors.error,
                    contentColor = colors.onError,
                ),
        ) {
            Text("Delete")
        }
    }
}

@Composable
private fun ViewHeader(title: String, onBack: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().heightIn(min = 48.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) { Icon(NestodoIcons.Back, contentDescription = "Back") }
        Text(
            title,
            style = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
    }
}

@Composable
private fun LargePicture(picture: ImageBitmap?) {
    Box(
        Modifier.size(72.dp)
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHighest)
    ) {
        if (picture != null) {
            Image(
                picture,
                contentDescription = null,
                modifier = Modifier.fillMaxSize(),
                contentScale = ContentScale.Crop,
            )
        }
    }
}

@Composable
private fun PreviewRow(node: Node, images: IconImages, completed: Boolean) {
    val colors = MaterialTheme.colorScheme
    val row =
        OutlineRow(
            key = node.key,
            title = node.title,
            depth = 0,
            completed = completed,
            dimmed = false,
            hasChildren = false,
            collapsed = false,
            icon = PendingIcon.value,
        )
    Row(
        Modifier.fillMaxWidth().clearAndSetSemantics {},
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Bullet(row, enabled = false, images = images) {}
        Text(
            remember(node.title, colors.primary) { shownText(node.title, colors.primary) },
            style =
                MaterialTheme.typography.bodyLarge.copy(
                    fontWeight = if (node.children.isNotEmpty()) FontWeight.SemiBold else null,
                    textDecoration = if (completed) TextDecoration.LineThrough else null,
                ),
            color = if (completed) colors.onSurfaceVariant else colors.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            inlineContent = LinkIcon,
            modifier = Modifier.weight(1f).padding(start = 4.dp, end = 8.dp),
        )
    }
}

@Composable
private fun NameField(
    state: TextFieldState,
    problem: String?,
    onSubmit: () -> Unit,
    onClose: () -> Unit,
) {
    val requester = remember { FocusRequester() }
    LaunchedEffect(requester) { requester.requestFocus() }
    OutlinedTextField(
        state = state,
        modifier =
            Modifier.fillMaxWidth()
                .padding(top = 12.dp)
                .focusRequester(requester)
                .onPreviewKeyEvent { fieldKey(it, state, onSubmit, onClose) },
        label = { Text("Name") },
        isError = problem != null,
        supportingText = problem?.let { { Text(it) } },
        inputTransformation = InputTransformation.maxLength(MAX_ICON_NAME_LENGTH),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        onKeyboardAction = { onSubmit() },
        lineLimits = TextFieldLineLimits.SingleLine,
        shape = RoundedCornerShape(12.dp),
    )
}

@Composable
private fun ViewButtons(content: @Composable RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(top = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        content = content,
    )
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 4.dp, top = 8.dp, bottom = 4.dp),
    )
}

@Composable
private fun Problem(text: String, start: Dp = 4.dp) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.error,
        modifier = Modifier.padding(start = start, top = 4.dp),
    )
}

private fun fieldKey(
    event: KeyEvent,
    field: TextFieldState,
    submit: () -> Unit,
    close: () -> Unit,
): Boolean {
    val down = event.type == KeyEventType.KeyDown
    return when (event.key) {
        Key.Enter,
        Key.NumPadEnter -> {
            if (field.composition != null) return false
            if (down) submit()
            true
        }
        Key.Escape -> {
            if (down) close()
            true
        }
        else -> false
    }
}
