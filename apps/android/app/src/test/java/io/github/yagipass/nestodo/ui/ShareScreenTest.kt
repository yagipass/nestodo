package io.github.yagipass.nestodo.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import org.junit.Assert.assertEquals
import org.junit.Test

class ShareScreenTest {
    private fun marks(title: String, match: String) =
        marked(AnnotatedString(title), match, Color.Yellow).spanStyles.map { it.start to it.end }

    @Test
    fun `every place a title matches the search is marked whatever its case, so it is clear why the todo is listed`() {
        assertEquals(listOf(0 to 4, 14 to 18), marks("Trip prep and TRIP list", "trip"))
    }

    @Test
    fun `a title is left unmarked while nothing is searched for, so browsing shows the todos as the main list does`() {
        assertEquals(emptyList<Pair<Int, Int>>(), marks("Trip prep", ""))
    }
}
