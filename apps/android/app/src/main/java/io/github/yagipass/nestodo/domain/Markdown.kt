package io.github.yagipass.nestodo.domain

import org.commonmark.ext.gfm.strikethrough.Strikethrough
import org.commonmark.ext.gfm.strikethrough.StrikethroughExtension
import org.commonmark.node.BlockQuote
import org.commonmark.node.Code
import org.commonmark.node.Emphasis
import org.commonmark.node.FencedCodeBlock
import org.commonmark.node.HardLineBreak
import org.commonmark.node.Heading
import org.commonmark.node.HtmlBlock
import org.commonmark.node.HtmlInline
import org.commonmark.node.Image
import org.commonmark.node.IndentedCodeBlock
import org.commonmark.node.Link
import org.commonmark.node.LinkReferenceDefinition
import org.commonmark.node.ListBlock
import org.commonmark.node.Node
import org.commonmark.node.OrderedList
import org.commonmark.node.Paragraph
import org.commonmark.node.SoftLineBreak
import org.commonmark.node.StrongEmphasis
import org.commonmark.node.Text
import org.commonmark.node.ThematicBreak
import org.commonmark.parser.IncludeSourceSpans
import org.commonmark.parser.Parser

sealed interface Inline {
    data class Text(val text: String) : Inline

    data class Code(val text: String) : Inline

    data class Strong(val children: List<Inline>) : Inline

    data class Em(val children: List<Inline>) : Inline

    data class Del(val children: List<Inline>) : Inline

    data class Link(val href: String, val children: List<Inline>) : Inline
}

sealed interface Block {
    data class Paragraph(val children: List<Inline>) : Block

    data class Heading(val level: Int, val children: List<Inline>) : Block

    data class Code(val text: String) : Block

    data class Quote(val children: List<Block>) : Block

    data class ItemList(val start: Int?, val items: List<List<Block>>) : Block

    data object Rule : Block
}

private val parser: Parser =
    Parser.builder()
        .extensions(listOf(StrikethroughExtension.builder().requireTwoTildes(true).build()))
        .enabledBlockTypes(
            setOf(
                Heading::class.java,
                FencedCodeBlock::class.java,
                IndentedCodeBlock::class.java,
                BlockQuote::class.java,
                ListBlock::class.java,
                ThematicBreak::class.java,
            )
        )
        .includeSourceSpans(IncludeSourceSpans.BLOCKS_AND_INLINES)
        .build()

private fun opens(url: String) = findLinks(url).firstOrNull()?.start == 0

private fun linked(url: String, label: List<Inline>) =
    Inline.Link(url, label.ifEmpty { listOf(Inline.Text(url)) })

private fun withBareLinks(nodes: List<Inline>): List<Inline> = nodes.flatMap { node ->
    if (node !is Inline.Text) return@flatMap listOf(node)
    buildList {
        var at = 0
        for (found in findLinks(node.text)) {
            if (found.start > at) add(Inline.Text(node.text.substring(at, found.start)))
            add(linked(found.url, emptyList()))
            at = found.end
        }
        if (at < node.text.length) add(Inline.Text(node.text.substring(at)))
    }
}

private fun asWritten(text: String): Block =
    Block.Paragraph(withBareLinks(listOf(Inline.Text(text))))

private fun plain(parent: Node): String = buildString {
    var child = parent.firstChild
    while (child != null) {
        when (child) {
            is Text -> append(child.literal)
            is Code -> append(child.literal)
            else -> append(plain(child))
        }
        child = child.next
    }
}

private class Reading(private val source: String) {
    private fun written(node: Node): String {
        val spans = node.sourceSpans
        if (spans.isEmpty()) return ""
        val last = spans.last()
        return source.substring(spans.first().inputIndex, last.inputIndex + last.length)
    }

    private fun inlines(parent: Node, inLink: Boolean = false): List<Inline> {
        val out = mutableListOf<Inline>()
        fun add(node: Inline) {
            val last = out.lastOrNull()
            when {
                node !is Inline.Text -> out += node
                last is Inline.Text -> out[out.lastIndex] = Inline.Text(last.text + node.text)
                node.text.isNotEmpty() -> out += node
            }
        }
        fun link(from: Node, url: String, label: List<Inline>) {
            when {
                inLink || url.isEmpty() -> label.forEach(::add)
                opens(url) -> add(linked(url, label))
                else -> add(Inline.Text(written(from)))
            }
        }
        var child = parent.firstChild
        while (child != null) {
            when (child) {
                is Text -> add(Inline.Text(child.literal))
                is HtmlInline -> add(Inline.Text(child.literal))
                is SoftLineBreak,
                is HardLineBreak -> add(Inline.Text("\n"))
                is Code -> add(Inline.Code(child.literal))
                is StrongEmphasis -> add(Inline.Strong(inlines(child, inLink)))
                is Emphasis -> add(Inline.Em(inlines(child, inLink)))
                is Strikethrough -> add(Inline.Del(inlines(child, inLink)))
                is Link -> link(child, child.destination, inlines(child, true))
                is Image ->
                    link(
                        child,
                        child.destination,
                        plain(child).let {
                            if (it.isEmpty()) emptyList() else listOf(Inline.Text(it))
                        },
                    )
                else -> add(Inline.Text(written(child)))
            }
            child = child.next
        }
        return if (inLink) out else withBareLinks(out)
    }

    private fun items(list: Node): List<List<Block>> = buildList {
        var item = list.firstChild
        while (item != null) {
            add(blocks(item))
            item = item.next
        }
    }

    fun blocks(parent: Node): List<Block> {
        val out = mutableListOf<Block>()
        var child = parent.firstChild
        while (child != null) {
            when (child) {
                is Paragraph -> out += Block.Paragraph(inlines(child))
                is Heading ->
                    out +=
                        if (child.firstChild == null) asWritten(written(child))
                        else Block.Heading(child.level, inlines(child))
                is FencedCodeBlock -> out += Block.Code(child.literal.removeSuffix("\n"))
                is IndentedCodeBlock -> out += Block.Code(child.literal.removeSuffix("\n"))
                is BlockQuote -> out += Block.Quote(blocks(child))
                is ListBlock ->
                    out +=
                        Block.ItemList(
                            (child as? OrderedList)?.let { it.markerStartNumber ?: 1 },
                            items(child),
                        )
                is ThematicBreak -> out += Block.Rule
                is LinkReferenceDefinition -> Unit
                is HtmlBlock -> out += asWritten(child.literal)
                else -> out += asWritten(written(child))
            }
            child = child.next
        }
        return out
    }
}

fun markdown(source: String): List<Block> =
    Reading(source).blocks(parser.parse(source)).ifEmpty { listOf(asWritten(source.trim())) }
