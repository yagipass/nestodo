package io.github.yagipass.nestodo.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class IconPickerTest {
    @Test
    fun `the delete warning says how many todos lose the icon, so the member knows what else changes before deleting`() {
        assertEquals("Deleting it cannot be undone.", deleteWarning(0))
        assertEquals(
            "It is the icon of 1 todo. Deleting it removes the icon from that todo and cannot be undone.",
            deleteWarning(1),
        )
        assertEquals(
            "It is the icon of 3 todos. Deleting it removes the icon from them and cannot be undone.",
            deleteWarning(3),
        )
    }
}
