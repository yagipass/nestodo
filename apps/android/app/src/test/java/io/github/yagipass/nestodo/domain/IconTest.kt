package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class IconTest {
    @Test
    fun `one emoji is accepted however it is built, so whatever the emoji keyboard inserts can be set`() {
        val accepted =
            listOf(
                "🛒",
                "🗑️",
                "❤️",
                "1️⃣",
                "☝🏻",
                "🇯🇵",
                "👩‍👩‍👧",
                "🏴󠁧󠁢󠁥󠁮󠁧󠁿",
            )
        assertEquals(accepted, accepted.filter(::isIcon))
    }

    @Test
    fun `text, a symbol drawn as text, or more than one emoji is refused, as the server refuses it, so nothing is sent that would come back as an error`() {
        val refused =
            listOf(
                "",
                "a",
                "1",
                "あ",
                "★",
                "❤",
                "🛒🛒",
                "🛒 ",
                "line:cart",
            )
        assertEquals(emptyList<String>(), refused.filter(::isIcon))
    }

    @Test
    fun `an emoji longer than 32 UTF-16 units is refused even as one cluster, because the server stores no more`() {
        val woman = "👩"
        val longest = woman + "‍$woman".repeat(10)
        val tooLong = woman + "‍$woman".repeat(11)
        assertEquals(32, longest.length)
        assertTrue(isIcon(longest))
        assertFalse(isIcon(tooLong))
    }

    @Test
    fun `the picker offers the 40 agreed emoji in order with U+FE0F kept where the shared list has it, so every client shows the same grid`() {
        assertEquals(40, IconChoices.size)
        assertEquals("🏠", IconChoices.first())
        assertEquals("⚠️", IconChoices.last())
        assertEquals(
            listOf(
                "🗑️",
                "🛍️",
                "✂️",
                "✈️",
                "✏️",
                "🏛️",
                "❤️",
                "⚠️",
            ),
            IconChoices.filter { '️' in it },
        )
    }

    @Test
    fun `every emoji in the picker passes the check the server makes, so a choice from the grid is never refused`() {
        assertEquals(emptyList<String>(), IconChoices.filterNot(::isIcon))
    }

    @Test
    fun `an icon of a kind from a later release is not drawn, so the row never shows text like line colon cart in place of an icon`() {
        assertNull(shownIcon("line:cart:green"))
        assertNull(shownIcon("image:3f2a9c1e-0000-4000-8000-000000000000"))
        assertNull(shownIcon(null))
    }

    @Test
    fun `any other icon is drawn as the text it holds, even one this phone's check would refuse, so an emoji newer than the phone's Unicode tables is not hidden`() {
        assertEquals("🛒", shownIcon("🛒"))
        assertEquals("❤", shownIcon("❤"))
    }
}
