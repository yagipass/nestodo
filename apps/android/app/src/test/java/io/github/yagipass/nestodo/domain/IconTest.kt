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

    private val cat =
        ImageIcon(
            "3f2a9c1e-0000-4000-8000-000000000001",
            "ミケ",
            "/api/v1/icons/3f2a9c1e-0000-4000-8000-000000000001/image",
        )

    @Test
    fun `an icon of a kind from a later release is not drawn, so the row never shows text like line colon cart in place of an icon`() {
        assertNull(shownIcon("line:cart:green", listOf(cat)))
        assertNull(shownIcon("line:${cat.id}", listOf(cat)))
        assertNull(shownIcon(null, listOf(cat)))
    }

    @Test
    fun `any other icon is drawn as the text it holds, even one this phone's check would refuse, so an emoji newer than the phone's Unicode tables is not hidden`() {
        assertEquals(ShownIcon.Emoji("🛒"), shownIcon("🛒", emptyList()))
        assertEquals(ShownIcon.Emoji("❤"), shownIcon("❤", emptyList()))
    }

    @Test
    fun `an image icon is drawn only while it is in the member's list, so one deleted elsewhere or not loaded yet shows nothing instead of its id`() {
        assertEquals(ShownIcon.Image(cat), shownIcon(cat.value, listOf(cat)))
        assertNull(shownIcon(cat.value, emptyList()))
        assertNull(shownIcon("image:3f2a9c1e-0000-4000-8000-000000000002", listOf(cat)))
    }

    @Test
    fun `the name offered for a new image is the file name without its extension, so a photo called Mike dot jpg is suggested as Mike`() {
        assertEquals("Mike", iconName("Mike.jpg"))
        assertEquals("school.crest", iconName("school.crest.png"))
        assertEquals("ミケ", iconName("  ミケ .webp"))
        assertEquals("", iconName(".png"))
        assertEquals("", iconName(""))
    }

    @Test
    fun `a long file name is cut to the 30 characters the server keeps without splitting an emoji, so the suggestion is never refused for its length`() {
        assertEquals("a".repeat(30), iconName("a".repeat(40) + ".jpg"))
        val cut = iconName("a".repeat(29) + "🐈" + ".png")
        assertEquals("a".repeat(29), cut)
    }

    @Test
    fun `a decomposed file name is composed first, as the server stores it, so the suggestion matches what is saved`() {
        assertEquals("\u30AC", iconName("\u30AB\u3099.jpg"))
    }

    @Test
    fun `deleting an image clears it from every todo using it and counts them first, so the warning names how many lose their icon`() {
        val outline =
            outlineOf(
                listOf(
                    Todo(1, null, 0, "Vet", icon = cat.value),
                    Todo(2, 1, 0, "Food", icon = cat.value, completedAt = "2026-11-13 09:00:00"),
                    Todo(3, null, 1, "Dentist", icon = "🦷"),
                )
            )

        assertEquals(2, outline.usesOf(cat.value))
        val cleared = outline.withoutIcon(cat.value)
        assertEquals(listOf(null, null, "🦷"), listOf(1L, 2L, 3L).map { cleared[it]!!.icon })
        assertEquals(0, cleared.usesOf(cat.value))
    }
}
