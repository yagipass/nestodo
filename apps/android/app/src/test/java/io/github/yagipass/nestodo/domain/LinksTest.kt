package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LinksTest {
    private fun urls(text: String) = findLinks(text).map { it.url }

    @Test
    fun `a link ends at full-width punctuation and typographic quotes so text written right after a URL is not opened as part of it`() {
        assertEquals(
            listOf("https://example.com/project/pull/296"),
            urls("PR: https://example.com/project/pull/296（two commits）"),
        )
        assertEquals(
            listOf("http://localhost:6420"),
            urls("«preview: http://localhost:6420»: done"),
        )
        assertEquals(
            listOf("https://example.com/pull/26"),
            urls("„https://example.com/pull/26“ (`Closes #25`)"),
        )
    }

    @Test
    fun `a link ends at the first Chinese, Japanese or Korean character so no space is needed after a URL`() {
        assertEquals(
            listOf(Link(3, 23, "https://example.com/")),
            findLinks("詳細はhttps://example.com/を参照"),
        )
        assertEquals(listOf("https://example.com/"), urls("详情见https://example.com/。"))
        assertEquals(listOf("https://example.com/"), urls("자세한 내용은 https://example.com/에서 확인"))
    }

    @Test
    fun `trailing punctuation stays in the link because real URLs end with it`() {
        assertEquals(
            listOf("https://en.wikipedia.org/wiki/Washington,_D.C."),
            urls("https://en.wikipedia.org/wiki/Washington,_D.C."),
        )
    }

    @Test
    fun `brackets that pair up inside the URL stay but a closing bracket that wraps it does not`() {
        assertEquals(
            listOf("https://en.wikipedia.org/wiki/Mercury_(planet)"),
            urls("(https://en.wikipedia.org/wiki/Mercury_(planet))"),
        )
        assertEquals(listOf("https://example.com/a[1]"), urls("[https://example.com/a[1]]"))
    }

    @Test
    fun `a link ends at quotes, backticks and angle brackets, which cannot appear in a URL`() {
        assertEquals(
            listOf("https://example.com", "https://example.org"),
            urls("\"https://example.com\" and <https://example.org>"),
        )
        assertEquals(
            listOf("https://developer.mozilla.org/en-US/docs/Web/API/URL/"),
            urls("Spec: `https://developer.mozilla.org/en-US/docs/Web/API/URL/`"),
        )
    }

    @Test
    fun `a URL inside another URL's query is part of it`() {
        assertEquals(
            listOf("https://a.example/login?next=https://b.example/"),
            urls("https://a.example/login?next=https://b.example/"),
        )
    }

    @Test
    fun `a scheme with no host is not a link because it cannot be opened`() {
        assertEquals(emptyList<String>(), urls("https:// and https:///path"))
        assertEquals(emptyList<String>(), urls("https://\\evil.example and https://user@/"))
    }

    @Test
    fun `the scheme is found in any letter case`() {
        assertEquals(listOf("HTTPS://Example.com"), urls("HTTPS://Example.com"))
    }

    @Test
    fun `the label is only the host so a long URL does not push the rest of the title out of the row`() {
        assertEquals(
            "example.com:8080",
            Link(0, 0, "https://www.example.com:8080/very/long/path?q=1#top").label,
        )
        assertEquals("www.", Link(0, 0, "https://www.").label)
    }

    @Test
    fun `the label is the host the browser opens so user info or a backslash cannot make it show another site`() {
        assertEquals(
            listOf("evil.example", "evil.example", "evil.example"),
            findLinks(
                    "https://good.example@evil.example " +
                        "https://evil.example\\@good.example/ " +
                        "https://good.example:443@www.evil.example/login"
                )
                .map { it.label },
        )
    }

    @Test
    fun `the label is lowercase like the host the browser opens so a capital I cannot pass for an l`() {
        assertEquals("paypai.example", Link(0, 0, "https://paypaI.example/").label)
    }

    @Test
    fun `a pasted URL with letters outside ASCII is percent-encoded so it stays one link even when it ends with one, such as 。`() {
        val pasted =
            mapOf(
                "https://example.com/ありがとう。" to
                    "https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82",
                "https://zh.wikipedia.org/wiki/北京" to
                    "https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC",
                "https://ko.wikipedia.org/wiki/서울" to
                    "https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8",
                "https://de.wikipedia.org/wiki/Fuß" to "https://de.wikipedia.org/wiki/Fu%C3%9F",
            )
        for ((raw, encoded) in pasted) {
            assertEquals(encoded, encodePastedUrl(raw))
            assertEquals(listOf(encoded), urls(encoded))
        }
    }

    @Test
    fun `an already encoded URL is left as it is so pasting it twice does not double-encode it`() {
        val encoded = "https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8"
        assertEquals(encoded, encodePastedUrl(encoded))
    }

    @Test
    fun `a domain outside ASCII becomes punycode and keeps its port`() {
        assertEquals(
            "https://xn--wgv71a119e.jp:8080/%E3%83%91%E3%82%B9",
            encodePastedUrl("https://日本語.jp:8080/パス"),
        )
        assertEquals(
            "https://xn--mnchen-3ya.de/stra%C3%9Fe",
            encodePastedUrl("https://münchen.de/straße"),
        )
        assertEquals("https://xn--3e0b707e.kr/", encodePastedUrl("https://한국.kr/"))
    }

    @Test
    fun `the whitespace a copied URL often carries is dropped`() {
        assertEquals(
            "https://example.com/%E5%8C%97%E4%BA%AC",
            encodePastedUrl("  https://example.com/北京\n"),
        )
    }

    @Test
    fun `characters that would end the link early are encoded so the whole pasted URL opens`() {
        for (pasted in
            listOf(
                "https://example.com/a)b",
                "https://example.com/a))(b)",
                "https://example.com/\"x\"",
                "https://example.com/😀",
            )) {
            val url = encodePastedUrl(pasted)!!
            assertEquals(listOf(url), urls(url))
        }
        assertEquals(
            "https://example.com/a%29%29(b)",
            encodePastedUrl("https://example.com/a))(b)"),
        )
        assertEquals("https://example.com/%F0%9F%98%80", encodePastedUrl("https://example.com/😀"))
    }

    @Test
    fun `anything but a single URL is left alone because its extent cannot be known`() {
        for (pasted in
            listOf(
                "München https://example.com/",
                "https://example.com/a b",
                "ftp://example.com/",
                "https://",
                "plain text",
            )) {
            assertNull(encodePastedUrl(pasted))
        }
    }

    @Test
    fun `a URL pasted into a title is encoded in place and the caret lands right after it`() {
        val before = "Read  later"
        val after = "Read https://zh.wikipedia.org/wiki/北京 later"
        assertEquals(
            Pasted(
                "Read https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC later",
                5 + "https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC".length,
            ),
            encodePaste(before, 5, 5, after),
        )
    }

    @Test
    fun `a URL pasted over a selection replaces only the selection`() {
        assertEquals(
            Pasted("a https://example.com/%C3%A9 b", 28),
            encodePaste("a XX b", 2, 4, "a https://example.com/é b"),
        )
    }

    @Test
    fun `typing a URL one character at a time is not rewritten under the user's fingers`() {
        assertNull(
            encodePaste(
                "https://ko.wikipedia.org/wiki/",
                30,
                30,
                "https://ko.wikipedia.org/wiki/서",
            )
        )
    }

    @Test
    fun `selecting a decoded URL without changing the text does not rewrite it`() {
        val title = "https://ja.wikipedia.org/wiki/東京"
        assertNull(encodePaste(title, 0, title.length, title))
    }
}
