package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.ui.text.TextRange
import io.github.yagipass.nestodo.domain.MAX_NOTE_LENGTH
import io.github.yagipass.nestodo.domain.MAX_TITLE_LENGTH
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class OutlineScreenTest {
    private var revision = 4L

    private fun title(text: String) =
        Field(text, { revision }, singleLine = true, maxLength = MAX_TITLE_LENGTH)

    private fun note(text: String) =
        Field(text, { revision }, singleLine = false, maxLength = MAX_NOTE_LENGTH)

    private fun Field.type(edit: TextFieldBuffer.() -> Unit) {
        state.edit {
            edit()
            with(input) { transformInput() }
        }
    }

    @Test
    fun `a pasted CRLF, CR or LF becomes one space each, as the server saves it, so a title pasted from Windows never gets two spaces`() {
        val field = title("")

        field.type { replace(0, 0, "Dentist\r\nat 9am\rsharp\nplease") }

        assertEquals(Typed("Dentist at 9am sharp please", TextRange(27), null), field.typed())
    }

    @Test
    fun `the caret after a pasted line break stays before the text that followed it, so the next keystroke goes where the user is`() {
        val field = title("tomorrow")
        field.type { selection = TextRange(0) }

        field.type { replace(0, 0, "Dentist\r\nat 9am ") }

        assertEquals(Typed("Dentist at 9am tomorrow", TextRange(15), null), field.typed())
    }

    @Test
    fun `a URL pasted at the caret is encoded in place, so the link the field shows is the one that gets saved and opened`() {
        val field = title("Read  later")
        field.type { selection = TextRange(5) }

        field.type { replace(5, 5, "https://zh.wikipedia.org/wiki/北京") }

        val url = "https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC"
        assertEquals(Typed("Read $url later", TextRange(5 + url.length), null), field.typed())
    }

    @Test
    fun `an edit that would make the title longer than the server keeps is refused whole, so the field never shows text that would be cut off when saved`() {
        val full = "a".repeat(MAX_TITLE_LENGTH - 1)
        val field = title(full)

        field.type { replace(length, length, "bc") }

        assertEquals(Typed(full, TextRange(full.length), null), field.typed())
    }

    @Test
    fun `a note keeps the line breaks typed into it, so a list written in the note stays a list`() {
        val field = note("")

        field.type { replace(0, 0, "Buy\nmilk") }

        assertEquals("Buy\nmilk", field.typed().text)
    }

    @Test
    fun `blank lines remembered from before the note was cleared give way to the empty note, so the reopened note neither shows them nor saves them with the next text typed`() {
        val field = note("")
        field.type { replace(0, 0, "\n\n") }
        revision = 5

        field.follow("")

        assertEquals(Typed("", TextRange(0), null), field.typed())
    }

    @Test
    fun `a title changed elsewhere replaces what the field holds with the caret at its end, so the next keystroke goes after the new text instead of into it`() {
        val field = title("Dentist")
        field.type { selection = TextRange(3) }
        revision = 5

        field.follow("Dentist at 9am")

        assertEquals(Typed("Dentist at 9am", TextRange(14), null), field.typed())
    }

    @Test
    fun `text typed in the current revision is kept while the outline still holds the text before it, so a keystroke is not undone before the ViewModel catches up`() {
        revision = 5
        val field = title("Dentist")
        field.type { replace(7, 7, "x") }

        field.follow("Dentist")

        assertEquals(Typed("Dentistx", TextRange(8), null), field.typed())
    }

    @Test
    fun `a field from an older revision keeps its caret when the title is unchanged, so a reload that leaves the title alone does not move the caret`() {
        val field = title("Dentist at 9am")
        field.type { selection = TextRange(7) }
        revision = 5

        field.follow("Dentist at 9am")

        assertEquals(TextRange(7), field.typed().selection)
    }

    @Test
    fun `a caret put back after the title gained an emoji elsewhere lands after the emoji, not inside it, so the next keystroke cannot save the emoji as a question mark`() {
        val field = title("ab😀 Dentist")

        field.place(3)

        assertEquals(TextRange(4), field.typed().selection)
    }

    @Test
    fun `what the user types is handed to the ViewModel once, so each keystroke is saved and then not sent again`() {
        val field = title("Dentist")
        field.type { replace(7, 7, "x") }

        assertEquals(Typed("Dentistx", TextRange(8), null), field.take())
        assertNull(field.take())
    }

    @Test
    fun `a title put in from elsewhere is not handed back to the ViewModel, so it cannot overwrite a newer title that arrived in the meantime`() {
        val field = title("Dentist")
        revision = 5

        field.follow("Dentist at 9am")

        assertNull(field.take())
    }

    @Test
    fun `putting the caret back after an outline change does not drop letters typed just before it, so they still reach the ViewModel`() {
        val field = title("Dentist")
        field.type { replace(7, 7, "x") }

        field.place(0)

        assertEquals(Typed("Dentistx", TextRange(0), null), field.take())
    }
}
