package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ShareTest {
    @Test
    fun `a page shared from a browser keeps its title and moves the link to the note so the row reads like the page`() {
        assertEquals(
            Shared("Pancake recipe", "https://example.com/pancakes"),
            sharedTodo("Pancake recipe", "https://example.com/pancakes"),
        )
    }

    @Test
    fun `a subject the text already contains is not repeated`() {
        assertEquals(
            Shared("Pancake recipe", "https://example.com/pancakes"),
            sharedTodo("Pancake recipe", "Pancake recipe https://example.com/pancakes"),
        )
    }

    @Test
    fun `a link that starts the text goes to the note too so the title starts with words`() {
        assertEquals(
            Shared("Make these on Sunday", "https://example.com/pancakes"),
            sharedTodo(null, "https://example.com/pancakes Make these on Sunday"),
        )
    }

    @Test
    fun `a link inside a sentence stays in the title because the sentence would not read without it`() {
        assertEquals(
            Shared("Compare https://example.com/pancakes with grandma's recipe", ""),
            sharedTodo(null, "Compare https://example.com/pancakes with grandma's recipe"),
        )
    }

    @Test
    fun `the text before a link is kept as shared, separators included, since the title can be edited before adding`() {
        assertEquals(
            Shared("Pancake recipe -", "https://example.com/pancakes"),
            sharedTodo(null, "Pancake recipe - https://example.com/pancakes"),
        )
    }

    @Test
    fun `a link shared alone goes to the note and leaves the title for the user to type`() {
        assertEquals(
            Shared("", "https://example.com/pancakes"),
            sharedTodo(null, "https://example.com/pancakes"),
        )
    }

    @Test
    fun `links at both ends go to the note in the order they were shared, above the rest of the text`() {
        assertEquals(
            Shared("Pancakes", "https://example.com/a\nhttps://example.com/b\nServes 4"),
            sharedTodo(null, "https://example.com/a Pancakes https://example.com/b\n\nServes 4"),
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
            Shared("서울", "https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8"),
            sharedTodo("서울", "https://ko.wikipedia.org/wiki/서울"),
        )
    }

    @Test
    fun `a link with letters outside ASCII after some words is moved whole and encoded so it opens as one link`() {
        assertEquals(
            Shared("東京", "https://ja.wikipedia.org/wiki/%E6%9D%B1%E4%BA%AC"),
            sharedTodo(null, "東京 https://ja.wikipedia.org/wiki/東京"),
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
