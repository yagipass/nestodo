package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ShareTest {
    @Test
    fun `a page shared from a browser keeps its title before the link so the row reads like the page`() {
        assertEquals(
            Shared("Pancake recipe https://example.com/pancakes", ""),
            sharedTodo("Pancake recipe", "https://example.com/pancakes"),
        )
    }

    @Test
    fun `a subject the text already contains is not repeated`() {
        assertEquals(
            Shared("Pancake recipe https://example.com/pancakes", ""),
            sharedTodo("Pancake recipe", "Pancake recipe https://example.com/pancakes"),
        )
    }

    @Test
    fun `the first line becomes the title and the rest the note so a long memo stays one row in the list`() {
        assertEquals(
            Shared("Shopping", "Milk\n  Eggs\n\nButter"),
            sharedTodo(null, "\n\n  Shopping  \n\nMilk\n  Eggs\n\nButter\n\n"),
        )
    }

    @Test
    fun `a shared URL with letters outside ASCII is encoded like a pasted one so the whole URL opens as one link`() {
        assertEquals(
            Shared("서울 https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8", ""),
            sharedTodo("서울", "https://ko.wikipedia.org/wiki/서울"),
        )
    }

    @Test
    fun `a subject alone is enough for a todo and its line breaks become spaces like a typed title`() {
        assertEquals(Shared("Call the dentist", ""), sharedTodo("Call\nthe dentist", null))
    }

    @Test
    fun `a share without any text has nothing to add`() {
        assertNull(sharedTodo(null, " \n "))
        assertNull(sharedTodo(" ", null))
    }

    @Test
    fun `text longer than a todo allows is cut so the server does not refuse the share`() {
        val shared = sharedTodo(null, "a".repeat(2500) + "\n" + "b".repeat(12000))!!
        assertEquals(MAX_TITLE_LENGTH, shared.title.length)
        assertEquals(MAX_NOTE_LENGTH, shared.note.length)
    }
}
