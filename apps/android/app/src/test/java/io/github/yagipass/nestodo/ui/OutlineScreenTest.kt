package io.github.yagipass.nestodo.ui

import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.TextFieldValue
import io.github.yagipass.nestodo.domain.Caret
import org.junit.Assert.assertEquals
import org.junit.Test

class OutlineScreenTest {
    @Test
    fun `a pasted CRLF, CR or LF becomes one space each, as the server saves it, so a title pasted from Windows never gets two spaces`() {
        val pasted = TextFieldValue("Dentist\r\nat 9am\rsharp\nplease", TextRange(28))

        assertEquals(
            TextFieldValue("Dentist at 9am sharp please", TextRange(27)),
            pasted.toSingleLine(),
        )
    }

    @Test
    fun `the caret and the word being composed after a pasted CRLF stay on the same text, so the next keystroke and Enter act where the user is`() {
        val pasted =
            TextFieldValue(
                "Dentist\r\nat 9am\rsharp tomorrow",
                selection = TextRange(21),
                composition = TextRange(22, 30),
            )

        assertEquals(
            TextFieldValue(
                "Dentist at 9am sharp tomorrow",
                selection = TextRange(20),
                composition = TextRange(21, 29),
            ),
            pasted.toSingleLine(),
        )
    }

    @Test
    fun `blank lines remembered from before the note was cleared give way to the empty note, so the reopened note neither shows them nor saves them with the next text typed`() {
        val remembered = Draft(4, TextFieldValue("\n\n", TextRange(2)))

        assertEquals(TextFieldValue("", TextRange(0)), remembered.resolve(5, ""))
    }

    @Test
    fun `a title changed elsewhere replaces the remembered draft with the caret at its end, so the next keystroke goes after the new text instead of into it`() {
        val remembered = Draft(4, TextFieldValue("Dentist", TextRange(3)))

        assertEquals(
            TextFieldValue("Dentist at 9am", TextRange(14)),
            remembered.resolve(5, "Dentist at 9am"),
        )
    }

    @Test
    fun `a draft typed in the current revision is kept while the outline still holds the text before it, so a keystroke is not undone before the ViewModel catches up`() {
        val typed = Draft(5, TextFieldValue("Dentistx", TextRange(8)))

        assertEquals(TextFieldValue("Dentistx", TextRange(8)), typed.resolve(5, "Dentist"))
    }

    @Test
    fun `a draft from an older revision keeps its caret when the title is unchanged, so a reload that leaves the title alone does not move the caret`() {
        val remembered = Draft(4, TextFieldValue("Dentist at 9am", TextRange(7)))

        assertEquals(
            TextFieldValue("Dentist at 9am", TextRange(7)),
            remembered.resolve(5, "Dentist at 9am"),
        )
    }

    @Test
    fun `a caret put back after the title gained an emoji elsewhere lands after the emoji, not inside it, so the next keystroke cannot save the emoji as a question mark`() {
        val renamed = "ab\uD83D\uDE00 Dentist"

        assertEquals(
            Draft(5, TextFieldValue(renamed, TextRange(4))),
            Caret(4, 3).placedIn(renamed, 5),
        )
    }
}
