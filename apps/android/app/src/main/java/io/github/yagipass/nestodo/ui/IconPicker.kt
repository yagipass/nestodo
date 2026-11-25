package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.ChoosingIcon
import io.github.yagipass.nestodo.TodoUiState
import io.github.yagipass.nestodo.TodoViewModel
import io.github.yagipass.nestodo.domain.IconChoices
import io.github.yagipass.nestodo.domain.shownIcon

@Composable
internal fun TodoIcon(
    icon: String?,
    muted: Boolean,
    modifier: Modifier = Modifier,
    size: Dp = 26.dp,
) {
    val emoji = shownIcon(icon) ?: return
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
            Text(emoji, fontSize = glyph, lineHeight = glyph, maxLines = 1)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun IconPicker(state: TodoUiState, choosing: ChoosingIcon, viewModel: TodoViewModel) {
    val node = state.outline[choosing.key] ?: return
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    val name = node.title.ifEmpty { "empty item" }
    val other = remember { TextFieldState() }
    var refused by remember { mutableStateOf(false) }
    LaunchedEffect(other) { snapshotFlow { other.text.toString() }.collect { refused = false } }

    fun applyOther() {
        if (!viewModel.setIcon(other.text.toString())) refused = true
    }

    ModalBottomSheet(
        onDismissRequest = viewModel::closeIconPicker,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            Modifier.fillMaxWidth().imePadding().padding(start = 16.dp, end = 16.dp, bottom = 16.dp)
        ) {
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
            IconChoices.chunked(8).forEach { line ->
                Row(Modifier.fillMaxWidth()) {
                    line.forEach { emoji ->
                        val selected = emoji == node.icon
                        val shape = RoundedCornerShape(12.dp)
                        Box(
                            Modifier.weight(1f)
                                .height(48.dp)
                                .padding(2.dp)
                                .clip(shape)
                                .background(
                                    if (selected) colors.secondaryContainer else Color.Transparent
                                )
                                .then(
                                    if (selected) Modifier.border(2.dp, colors.primary, shape)
                                    else Modifier
                                )
                                .selectable(selected = selected) { viewModel.setIcon(emoji) },
                            contentAlignment = Alignment.Center,
                        ) {
                            TodoIcon(emoji, muted = false, size = 34.dp)
                        }
                    }
                }
            }
            BasicTextField(
                state = other,
                modifier =
                    Modifier.fillMaxWidth()
                        .padding(top = 12.dp)
                        .onPreviewKeyEvent {
                            otherKey(it, other, ::applyOther, viewModel::closeIconPicker)
                        }
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
            if (refused) {
                Text(
                    "Enter one emoji",
                    style = typography.bodySmall,
                    color = colors.error,
                    modifier = Modifier.padding(start = 14.dp, top = 4.dp),
                )
            }
        }
    }
}

private fun otherKey(
    event: KeyEvent,
    field: TextFieldState,
    apply: () -> Unit,
    close: () -> Unit,
): Boolean {
    val down = event.type == KeyEventType.KeyDown
    return when (event.key) {
        Key.Enter,
        Key.NumPadEnter -> {
            if (field.composition != null) return false
            if (down) apply()
            true
        }
        Key.Escape -> {
            if (down) close()
            true
        }
        else -> false
    }
}
