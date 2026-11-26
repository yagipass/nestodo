package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.domain.Branch
import io.github.yagipass.nestodo.domain.Guide
import io.github.yagipass.nestodo.domain.guides

@Composable
internal fun DestinationTree(
    tree: List<Branch>,
    match: String,
    onToggle: (Long) -> Unit,
    onPick: (Long) -> Unit,
    above: LazyListScope.() -> Unit,
) {
    val lines = remember(tree) { guides(tree.map { it.row }) }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(vertical = 4.dp)) {
        above()
        itemsIndexed(tree, key = { _, branch -> branch.row.key }) { index, branch ->
            BranchRow(
                branch = branch,
                guide = lines[index],
                match = match,
                foldable = match.isEmpty(),
                onToggle = { onToggle(branch.row.key) },
                onPick = { onPick(branch.row.key) },
            )
        }
    }
}

@Composable
private fun BranchRow(
    branch: Branch,
    guide: Guide,
    match: String,
    foldable: Boolean,
    onToggle: () -> Unit,
    onPick: () -> Unit,
) {
    val row = branch.row
    val colors = MaterialTheme.colorScheme
    val guideColor = LocalGuideColor.current
    val mark = colors.primary.copy(alpha = 0.2f)
    val title =
        remember(row.title, match, colors.primary, mark) {
            marked(shownText(row.title, colors.primary), match, mark)
        }
    val faded = row.completed || row.dimmed || !branch.offered
    Row(
        Modifier.fillMaxWidth()
            .height(RowHeight)
            .clickable(enabled = branch.offered, role = Role.Button, onClick = onPick)
            .drawBehind { drawGuide(guide, row.depth, guideColor) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Spacer(Modifier.width(24.dp * row.depth))
        Box(Modifier.alpha(if (branch.offered) 1f else 0.5f)) {
            Bullet(row, enabled = foldable, onToggle = onToggle)
        }
        Text(
            title,
            style =
                MaterialTheme.typography.bodyLarge.copy(
                    fontWeight = if (row.hasChildren) FontWeight.SemiBold else null,
                    textDecoration = if (row.completed) TextDecoration.LineThrough else null,
                ),
            color = if (faded) colors.onSurfaceVariant else colors.onSurface,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            inlineContent = LinkIcon,
            modifier = Modifier.weight(1f).padding(start = 4.dp, end = 16.dp),
        )
    }
}

internal fun marked(text: AnnotatedString, match: String, color: Color): AnnotatedString {
    if (match.isEmpty()) return text
    return buildAnnotatedString {
        append(text)
        var at = text.text.indexOf(match, ignoreCase = true)
        while (at >= 0) {
            addStyle(SpanStyle(background = color), at, at + match.length)
            at = text.text.indexOf(match, at + match.length, ignoreCase = true)
        }
    }
}
