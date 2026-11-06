package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yagipass.nestodo.R
import io.github.yagipass.nestodo.ShareState
import io.github.yagipass.nestodo.ShareViewModel
import io.github.yagipass.nestodo.domain.Shared
import io.github.yagipass.nestodo.domain.destinations
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.delay

@Composable
fun ShareScreen(
    viewModel: ShareViewModel,
    onBack: () -> Unit,
    onOpenApp: () -> Unit,
    onAdded: (String?) -> Unit,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val colors = MaterialTheme.colorScheme
    val appName = stringResource(R.string.app_name)

    LaunchedEffect(state) { (state as? ShareState.Added)?.let { onAdded(it.parent) } }

    Surface(Modifier.fillMaxSize(), color = colors.background) {
        Column(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing)) {
            Row(
                Modifier.fillMaxWidth().padding(start = 4.dp, end = 16.dp, top = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onBack) {
                    Icon(NestodoIcons.Back, contentDescription = "Back")
                }
                Text("Add to $appName", style = MaterialTheme.typography.titleLarge)
            }
            Preview(viewModel.shared)
            HorizontalDivider(color = colors.outlineVariant)
            when (val current = state) {
                ShareState.Loading ->
                    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                is ShareState.Added -> {}
                ShareState.SignedOut ->
                    Message("Sign in to $appName to add todos.", "Open $appName", onOpenApp)
                is ShareState.Failed -> Message(current.message, "Try again", viewModel::retry)
                is ShareState.Ready -> Picker(current, viewModel::add)
            }
        }
    }
}

@Composable
private fun Preview(shared: Shared) {
    val colors = MaterialTheme.colorScheme
    val typography = MaterialTheme.typography
    var expanded by rememberSaveable { mutableStateOf(false) }
    var clipped by remember { mutableStateOf(false) }
    val lines = if (expanded) Int.MAX_VALUE else PREVIEW_LINES
    val onLayout = { layout: TextLayoutResult -> if (layout.hasVisualOverflow) clipped = true }
    Column(Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 4.dp, bottom = 8.dp)) {
        Column(Modifier.heightIn(max = 280.dp).verticalScroll(rememberScrollState())) {
            Text(
                remember(shared.title, colors.primary) { shownText(shared.title, colors.primary) },
                style = typography.bodyLarge,
                color = colors.onSurface,
                maxLines = lines,
                overflow = TextOverflow.Ellipsis,
                onTextLayout = onLayout,
            )
            if (shared.note.isNotEmpty()) {
                Text(
                    remember(shared.note, colors.primary) {
                        shownText(shared.note, colors.primary)
                    },
                    style = typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = lines,
                    overflow = TextOverflow.Ellipsis,
                    onTextLayout = onLayout,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
        if (clipped) {
            TextButton(
                onClick = { expanded = !expanded },
                contentPadding = PaddingValues(horizontal = 0.dp),
            ) {
                Text(if (expanded) "Show less" else "Show all")
            }
        } else {
            Spacer(Modifier.height(8.dp))
        }
    }
}

@Composable
private fun Picker(state: ShareState.Ready, onPick: (Long?) -> Unit) {
    val colors = MaterialTheme.colorScheme
    var query by remember { mutableStateOf("") }
    var armed by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) {
        delay(PICK_DELAY)
        armed = true
    }
    val pick = { parent: Long? -> if (armed) onPick(parent) }
    val found = state.outline.destinations(state.viewport, null, query)
    val browsing = query.isBlank()

    SearchField(
        query = query,
        onQueryChange = { query = it },
        label = "Search todos",
        modifier = Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 14.dp),
    )
    if (state.adding) {
        LinearProgressIndicator(Modifier.fillMaxWidth())
    } else {
        HorizontalDivider(color = colors.outlineVariant)
    }
    state.error?.let {
        Text(
            it,
            style = MaterialTheme.typography.bodyMedium,
            color = colors.error,
            modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp),
        )
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(vertical = 4.dp)) {
        if (browsing) {
            state.last?.let { last ->
                item(key = "last") {
                    Column {
                        Text(
                            "Last used",
                            style = MaterialTheme.typography.labelMedium,
                            color = colors.primary,
                            modifier = Modifier.padding(start = 20.dp, end = 20.dp, top = 8.dp),
                        )
                        DestinationRow(last.title, last.path) { pick(last.key) }
                        HorizontalDivider(
                            color = colors.outlineVariant,
                            modifier = Modifier.padding(vertical = 4.dp),
                        )
                    }
                }
            }
            item(key = "top") { DestinationRow("Top level", emptyList()) { pick(null) } }
        } else if (found.isEmpty()) {
            item { NoMatches() }
        }
        items(found, key = { it.key }) { destination ->
            DestinationRow(destination.title, destination.path) { pick(destination.key) }
        }
    }
}

@Composable
private fun Message(text: String, action: String, onAction: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Button(onClick = onAction) { Text(action) }
    }
}

private const val PREVIEW_LINES = 2

private val PICK_DELAY = 500.milliseconds
