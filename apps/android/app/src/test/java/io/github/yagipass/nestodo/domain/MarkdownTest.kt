package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Test

class MarkdownTest {
    private fun text(value: String) = Inline.Text(value)

    private fun code(value: String) = Inline.Code(value)

    private fun strong(vararg children: Inline) = Inline.Strong(children.toList())

    private fun link(href: String, vararg children: Inline) =
        Inline.Link(href, if (children.isEmpty()) listOf(text(href)) else children.toList())

    private fun p(vararg children: Inline) = Block.Paragraph(children.toList())

    private fun list(start: Int?, vararg items: List<Block>) = Block.ItemList(start, items.toList())

    @Test
    fun `a line break and a run of spaces inside a paragraph stay as written, so a log pasted without Markdown in mind reads the same`() {
        assertEquals(
            listOf(p(text("built  ok\ntests   2 failed"))),
            markdown("built  ok\ntests   2 failed"),
        )
    }

    @Test
    fun `bold, italic, struck and code text are formatted, the marks an assistant writes a summary with`() {
        assertEquals(
            listOf(
                p(
                    strong(text("done")),
                    text(" and "),
                    Inline.Em(listOf(text("maybe"))),
                    text(", "),
                    Inline.Del(listOf(text("dropped"))),
                    text(", run "),
                    code("vp test"),
                )
            ),
            markdown("**done** and *maybe*, ~~dropped~~, run `vp test`"),
        )
    }

    @Test
    fun `marks that touch Japanese text are formatted, since Japanese has no spaces to set them apart`() {
        assertEquals(
            listOf(p(text("これは"), strong(text("太字")), text("です"))),
            markdown("これは**太字**です"),
        )
    }

    @Test
    fun `a single tilde is left alone, so a range such as 10~20 is not struck through`() {
        assertEquals(
            listOf(p(text("10~20 items, 30~40 at most"))),
            markdown("10~20 items, 30~40 at most"),
        )
    }

    @Test
    fun `lists nest and a numbered list starts at the number written, so steps keep their order when a comment continues an earlier one`() {
        assertEquals(
            listOf(
                list(
                    3,
                    listOf(
                        p(text("build")),
                        list(null, listOf(p(text("web"))), listOf(p(text("cli")))),
                    ),
                    listOf(p(text("release"))),
                )
            ),
            markdown("3. build\n   - web\n   - cli\n4. release"),
        )
    }

    @Test
    fun `a fenced block is shown exactly as written, so pasted output keeps its indentation and its asterisks`() {
        assertEquals(
            listOf(Block.Code("$ ls  *.ts\n  a.ts  **b.ts**")),
            markdown("```sh\n$ ls  *.ts\n  a.ts  **b.ts**\n```"),
        )
    }

    @Test
    fun `headings, quotes and rules are read, so a long comment can be divided into parts`() {
        assertEquals(
            listOf(
                Block.Heading(2, listOf(text("Plan"))),
                Block.Quote(listOf(p(text("wait for the review")))),
                Block.Rule,
            ),
            markdown("## Plan\n\n> wait for the review\n\n---"),
        )
    }

    @Test
    fun `a link shows its label with the label's own formatting and opens the address behind it`() {
        assertEquals(
            listOf(
                p(
                    text("see "),
                    link(
                        "https://example.com/pull/7",
                        text("the "),
                        strong(text("pull")),
                        text(" request"),
                    ),
                    text(" first"),
                )
            ),
            markdown("see [the **pull** request](https://example.com/pull/7) first"),
        )
    }

    @Test
    fun `a URL written on its own is a link that shows it as written, so a comment that is only a link can still be opened`() {
        assertEquals(
            listOf(p(link("https://example.com/a_b_c"))),
            markdown("https://example.com/a_b_c"),
        )
        assertEquals(
            listOf(
                p(
                    link("https://example.org"),
                    text(" and "),
                    strong(link("https://example.net/x")),
                )
            ),
            markdown("<https://example.org> and **https://example.net/x**"),
        )
    }

    @Test
    fun `a URL written on its own ends where Japanese text starts, so a sentence that goes on after it is not opened as part of the address`() {
        assertEquals(
            listOf(p(text("詳細は"), link("https://example.com/"), text("を参照"))),
            markdown("詳細はhttps://example.com/を参照"),
        )
    }

    @Test
    fun `a URL inside code or inside a link's label stays text, so code reads as written and one link is never nested in another`() {
        assertEquals(
            listOf(
                p(
                    code("curl https://example.com/api"),
                    text(" or "),
                    link("https://example.com/b", text("see https://example.com/a")),
                )
            ),
            markdown(
                "`curl https://example.com/api` or [see https://example.com/a](https://example.com/b)"
            ),
        )
    }

    @Test
    fun `a picture inside a link is kept as its description, so one link is never nested in another`() {
        assertEquals(
            listOf(p(link("https://example.com", text("logo home")))),
            markdown("[![logo](https://example.com/logo.png) home](https://example.com)"),
        )
    }

    @Test
    fun `a link written with character references opens the address it names`() {
        assertEquals(
            listOf(p(text("AT&T "), link("https://example.com/?q=1&page=2", text("search")))),
            markdown("AT&amp;T [search](https://example.com/?q=1&amp;page=2)"),
        )
    }

    @Test
    fun `a link that is not http or https is left as written, so nothing in a comment runs a script or opens another app`() {
        val written =
            "[run](javascript:alert(1)), [mail](mailto:a@example.com), [here](/settings) and <a@example.com>"
        assertEquals(listOf(p(text(written))), markdown(written))
    }

    @Test
    fun `a link with no address shows only its label, so nothing that looks like a link opens nowhere`() {
        assertEquals(listOf(p(text("later and chart"))), markdown("[later]() and ![chart]()"))
    }

    @Test
    fun `a link with no label shows its address, so no link is too small to see or tap`() {
        assertEquals(
            listOf(
                p(
                    link("https://example.com/plain.png"),
                    text(" and "),
                    link("https://example.net/b"),
                )
            ),
            markdown("![](https://example.com/plain.png) and [](https://example.net/b)"),
        )
    }

    @Test
    fun `HTML, tables and task boxes are shown as written, since they are not drawn`() {
        assertEquals(
            listOf(
                p(text("<b>bold</b>")),
                p(text("<script>alert(1)</script>")),
                p(text("| a | b |\n|---|---|\n| 1 | 2 |")),
                list(null, listOf(p(text("[ ] later")))),
            ),
            markdown(
                "<b>bold</b>\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] later"
            ),
        )
    }

    @Test
    fun `a picture becomes a link to it labelled with its description, since comments do not load pictures`() {
        assertEquals(
            listOf(p(link("https://example.com/chart.png", text("the chart")))),
            markdown("![the chart](https://example.com/chart.png)"),
        )
    }

    @Test
    fun `a reference is followed to the address defined for it elsewhere in the comment`() {
        assertEquals(
            listOf(
                p(text("see "), link("https://example.com/doc", text("the doc")), text(" today"))
            ),
            markdown("see [the doc][1] today\n\n[1]: https://example.com/doc"),
        )
    }

    @Test
    fun `a comment that is only a link definition or an empty heading is shown as written, so no comment is ever blank`() {
        assertEquals(
            listOf(p(text("[1]: "), link("https://example.com/doc"))),
            markdown("[1]: https://example.com/doc"),
        )
        assertEquals(listOf(p(text("#"))), markdown("#"))
    }
}
