package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import io.github.yagipass.nestodo.domain.Block
import io.github.yagipass.nestodo.domain.Inline
import io.github.yagipass.nestodo.domain.markdown

private data class Look(val link: Color, val code: Color)

private fun decoration(struck: Boolean, linked: Boolean) =
    TextDecoration.combine(
        listOfNotNull(
            TextDecoration.LineThrough.takeIf { struck },
            TextDecoration.Underline.takeIf { linked },
        )
    )

private fun AnnotatedString.Builder.styled(
    nodes: List<Inline>,
    look: Look,
    onOpenLink: (String) -> Unit,
    struck: Boolean = false,
    linked: Boolean = false,
) {
    for (node in nodes) {
        when (node) {
            is Inline.Text -> append(node.text)
            is Inline.Code ->
                withStyle(
                    SpanStyle(
                        fontFamily = FontFamily.Monospace,
                        fontSize = 0.875.em,
                        background = look.code,
                    )
                ) {
                    append(node.text)
                }
            is Inline.Strong ->
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) {
                    styled(node.children, look, onOpenLink, struck, linked)
                }
            is Inline.Em ->
                withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                    styled(node.children, look, onOpenLink, struck, linked)
                }
            is Inline.Del ->
                withStyle(SpanStyle(textDecoration = decoration(struck = true, linked))) {
                    styled(node.children, look, onOpenLink, struck = true, linked)
                }
            is Inline.Link -> {
                val style = SpanStyle(color = look.link, textDecoration = decoration(struck, true))
                withLink(
                    LinkAnnotation.Clickable(node.href, TextLinkStyles(style)) {
                        onOpenLink(node.href)
                    }
                ) {
                    styled(node.children, look, onOpenLink, struck, linked = true)
                }
            }
        }
    }
}

@Composable
private fun Styled(children: List<Inline>, style: TextStyle, onOpenLink: (String) -> Unit) {
    val colors = MaterialTheme.colorScheme
    val look = Look(link = colors.primary, code = colors.secondaryContainer)
    val text =
        remember(children, look, onOpenLink) {
            buildAnnotatedString { styled(children, look, onOpenLink) }
        }
    Text(text, style = style)
}

private fun heading(level: Int, body: TextStyle) =
    when (level) {
        1 -> body.copy(fontSize = 17.sp, lineHeight = 24.sp, fontWeight = FontWeight.Bold)
        2 -> body.copy(fontSize = 16.sp, lineHeight = 23.sp, fontWeight = FontWeight.Bold)
        else -> body.copy(fontWeight = FontWeight.Bold)
    }

@Composable
private fun Blocks(
    blocks: List<Block>,
    onOpenLink: (String) -> Unit,
    depth: Int,
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val body = MaterialTheme.typography.bodyMedium.copy(lineHeight = 22.sp)
    Column(modifier) {
        blocks.forEachIndexed { index, block ->
            val hugs = index == 0 || (depth > 0 && block is Block.ItemList)
            Box(Modifier.padding(top = if (hugs) 0.dp else 8.dp)) {
                when (block) {
                    is Block.Paragraph -> Styled(block.children, body, onOpenLink)
                    is Block.Heading ->
                        Styled(block.children, heading(block.level, body), onOpenLink)
                    is Block.Code ->
                        Text(
                            block.text,
                            style =
                                body.copy(
                                    fontFamily = FontFamily.Monospace,
                                    fontSize = 13.sp,
                                    lineHeight = 20.sp,
                                ),
                            softWrap = false,
                            modifier =
                                Modifier.fillMaxWidth()
                                    .clip(RoundedCornerShape(10.dp))
                                    .background(colors.secondaryContainer)
                                    .horizontalScroll(rememberScrollState())
                                    .padding(horizontal = 12.dp, vertical = 8.dp),
                        )
                    is Block.Quote -> {
                        val guide = LocalGuideColor.current
                        CompositionLocalProvider(
                            LocalContentColor provides colors.onSurfaceVariant
                        ) {
                            Blocks(
                                block.children,
                                onOpenLink,
                                depth,
                                Modifier.drawBehind {
                                        drawRect(guide, size = Size(3.dp.toPx(), size.height))
                                    }
                                    .padding(start = 12.dp),
                            )
                        }
                    }
                    is Block.ItemList ->
                        Column {
                            block.items.forEachIndexed { at, item ->
                                Row {
                                    Text(
                                        when {
                                            block.start != null -> "${block.start + at}."
                                            depth == 0 -> "•"
                                            else -> "◦"
                                        },
                                        style = body,
                                        color = colors.onSurfaceVariant,
                                        modifier =
                                            Modifier.widthIn(min = 20.dp).padding(end = 6.dp),
                                    )
                                    Blocks(item, onOpenLink, depth + 1, Modifier.weight(1f))
                                }
                            }
                        }
                    Block.Rule -> HorizontalDivider(color = colors.outlineVariant)
                }
            }
        }
    }
}

@Composable
internal fun MarkdownText(text: String, onOpenLink: (String) -> Unit) {
    val blocks = remember(text) { markdown(text) }
    CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
        Blocks(blocks, onOpenLink, depth = 0)
    }
}
