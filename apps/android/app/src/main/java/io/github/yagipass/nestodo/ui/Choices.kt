package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.minimumInteractiveComponentSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun <T> Choices(options: List<T>, selected: T, label: (T) -> String, onPick: (T) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(10.dp)
    FlowRow(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { value ->
            val chosen = value == selected
            Row(
                Modifier.minimumInteractiveComponentSize()
                    .height(36.dp)
                    .clip(shape)
                    .border(1.dp, if (chosen) colors.primary else colors.outlineVariant, shape)
                    .background(if (chosen) colors.secondaryContainer else colors.surfaceContainer)
                    .selectable(chosen, role = Role.RadioButton, onClick = { onPick(value) })
                    .padding(start = if (chosen) 10.dp else 14.dp, end = 14.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (chosen) {
                    Icon(
                        NestodoIcons.Selected,
                        contentDescription = null,
                        tint = colors.primary,
                        modifier = Modifier.size(16.dp),
                    )
                }
                Text(
                    label(value),
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (chosen) colors.primary else colors.onSurfaceVariant,
                    fontWeight = if (chosen) FontWeight.Medium else null,
                )
            }
        }
    }
}
