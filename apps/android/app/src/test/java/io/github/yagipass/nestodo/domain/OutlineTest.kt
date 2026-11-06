package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

fun todo(
    id: Long,
    title: String,
    parent: Long? = null,
    position: Int = 0,
    done: Boolean = false,
    collapsed: Boolean = false,
) = Todo(id, parent, position, title, if (done) "2026-10-23 09:00:00" else null, collapsed)

fun Outline.render(viewport: Viewport = Viewport()): List<String> =
    rows(viewport).map { "  ".repeat(it.depth) + it.title }

class OutlineTest {
    private val outline =
        outlineOf(
            listOf(
                todo(1, "Groceries"),
                todo(2, "Milk", parent = 1),
                todo(3, "Eggs", parent = 1, position = 1),
                todo(4, "Dentist", position = 1),
            )
        )

    @Test
    fun `the server's rows become a tree ordered by position so siblings keep the order the user gave them`() {
        val shuffled =
            outlineOf(
                listOf(
                    todo(3, "Eggs", parent = 1, position = 1),
                    todo(4, "Dentist", position = 1),
                    todo(2, "Milk", parent = 1),
                    todo(1, "Groceries"),
                )
            )
        assertEquals(listOf("Groceries", "  Milk", "  Eggs", "Dentist"), shuffled.render())
        assertEquals(outline, shuffled)
        val movedUp = outlineOf(listOf(todo(1, "older", position = 1), todo(2, "moved above it")))
        assertEquals(listOf("moved above it", "older"), movedUp.render())
    }

    @Test
    fun `a todo whose parent is missing stays at the top level instead of disappearing`() {
        val orphaned = outlineOf(listOf(todo(5, "Orphan", parent = 99)))
        assertEquals(listOf("Orphan"), orphaned.render())
    }

    @Test
    fun `hiding completed rows hides everything under them so a finished project leaves no strays`() {
        val done =
            outlineOf(
                listOf(
                    todo(1, "Groceries", done = true),
                    todo(2, "Milk", parent = 1),
                    todo(4, "Dentist", position = 1),
                )
            )
        assertEquals(listOf("Dentist"), done.render(Viewport(showCompleted = false)))
    }

    @Test
    fun `rows under a completed row are dimmed but stay open because completion does not cascade`() {
        val done = outline.apply(Change.Complete(1, true))
        val milk = done.rows(Viewport()).single { it.key == 2L }
        assertTrue(milk.dimmed)
        assertFalse(milk.completed)
    }

    @Test
    fun `a collapsed row hides its children but still shows itself and that it has some`() {
        val folded = outline.apply(Change.Collapse(1, true))
        assertEquals(listOf("Groceries", "Dentist"), folded.render())
        val groceries = folded.rows(Viewport()).first()
        assertTrue(groceries.collapsed && groceries.hasChildren)
    }

    @Test
    fun `a move lands at the final index among the new siblings like the server's PATCH`() {
        val moved = outline.apply(Change.Move(4, 1, 1))
        assertEquals(listOf("Groceries", "  Milk", "  Dentist", "  Eggs"), moved.render())
        val down = outline.apply(Change.Move(2, 1, 1))
        assertEquals(listOf("Groceries", "  Eggs", "  Milk", "Dentist"), down.render())
    }

    @Test
    fun `deleting a row removes its whole subtree like the server does`() {
        val deleted = outline.apply(Change.Delete(1))
        assertEquals(listOf("Dentist"), deleted.render())
        assertEquals(setOf(4L), deleted.nodes.keys)
    }

    @Test
    fun `undoing a group of changes restores the exact outline including sibling positions`() {
        val changes =
            listOf(
                Change.Move(4, 1, 0),
                Change.Collapse(1, true),
                Change.Rename(2, "Oat milk"),
                Change.Complete(3, true),
                Change.Create(-1, 4, 0, "Call"),
            )
        val edited = changes.fold(outline, Outline::apply)
        val undo = outline.undoFor(changes)!!
        assertEquals(outline, undo.fold(edited, Outline::apply))
    }

    @Test
    fun `undoing a deletion recreates the row under the same key so older undo steps still find it`() {
        val done = outline.apply(Change.Complete(3, true))
        val undo = done.undoFor(listOf(Change.Delete(3)))!!
        assertEquals(listOf(Change.Create(3, 1, 1, "Eggs"), Change.Complete(3, true)), undo)
    }

    @Test
    fun `undoing a deletion brings the note back too so Undo restores everything the row held`() {
        val noted = outline.apply(Change.Annotate(3, "free-range"))
        val undo = noted.undoFor(listOf(Change.Delete(3)))!!
        assertEquals(listOf(Change.Create(3, 1, 1, "Eggs", "free-range")), undo)
        assertEquals(noted, undo.fold(noted.apply(Change.Delete(3)), Outline::apply))
    }

    @Test
    fun `undoing a deletion recreates every row under it, parents first, with each note, completion and fold`() {
        val held =
            listOf(Change.Collapse(1, true), Change.Annotate(2, "oat"), Change.Complete(3, true))
                .fold(outline, Outline::apply)
        val delete = listOf(Change.Delete(1, recursive = true))
        val undo = held.undoFor(delete)!!
        assertEquals(
            listOf(
                Change.Create(1, null, 0, "Groceries"),
                Change.Collapse(1, true),
                Change.Create(2, 1, 0, "Milk", "oat"),
                Change.Create(3, 1, 1, "Eggs"),
                Change.Complete(3, true),
            ),
            undo,
        )
        assertEquals(held, undo.fold(delete.fold(held, Outline::apply), Outline::apply))
    }

    @Test
    fun `a row cannot be moved under its own descendant, which would cut it out of the outline`() {
        assertFalse(outline.accepts(Change.Move(1, 2, 0)))
        assertTrue(outline.accepts(Change.Move(4, 2, 0)))
        assertFalse(outline.accepts(listOf(Change.Delete(2), Change.Rename(2, "gone"))))
    }
}
