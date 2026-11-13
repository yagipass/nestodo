package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EditingTest {
    private val groceries = todo(1, "Groceries")
    private val milk = todo(2, "Milk", parent = 1)
    private val eggs = todo(3, "Eggs", parent = 1, position = 1)
    private val dentist = todo(4, "Dentist", position = 1)
    private val outline = outlineOf(listOf(groceries, milk, eggs, dentist))
    private val all = Viewport()

    private fun with(vararg todos: Todo) = outlineOf(todos.toList())

    private fun chain(depth: Int) =
        (1L..depth).map { id -> todo(id, "Level $id", parent = (id - 1).takeIf { it > 0 }) }

    private val tooDeep =
        outlineOf(
            chain(MAX_DEPTH) + todo(100, "Branch", position = 1) + todo(101, "Leaf", parent = 100)
        )

    @Test
    fun `Enter at the end of a row adds an empty sibling right below it and moves the cursor there`() {
        assertEquals(
            Edit(listOf(Change.Create(-1, 1, 1, "")), Caret(-1, 0)),
            outline.enter(all, 2, 4, newKey = -1),
        )
    }

    @Test
    fun `Enter at the end of an expanded parent adds its first child so typing continues inside the list`() {
        assertEquals(
            Edit(listOf(Change.Create(-1, 1, 0, "")), Caret(-1, 0)),
            outline.enter(all, 1, 9, newKey = -1),
        )
    }

    @Test
    fun `Enter at the end of a collapsed parent adds a sibling because its children are out of sight`() {
        val folded = with(groceries.copy(collapsed = true), milk, eggs, dentist)
        assertEquals(
            Edit(listOf(Change.Create(-1, null, 1, "")), Caret(-1, 0)),
            folded.enter(all, 1, 9, newKey = -1),
        )
    }

    @Test
    fun `Enter in the middle of a row splits it and continues at the start of the second half`() {
        assertEquals(
            Edit(
                listOf(Change.Create(-1, null, 2, "ist"), Change.Rename(4, "Dent")),
                Caret(-1, 0),
            ),
            outline.enter(all, 4, 4, newKey = -1),
        )
    }

    @Test
    fun `Enter at the start of a row inserts an empty row above and keeps the cursor where it was`() {
        assertEquals(
            Edit(listOf(Change.Create(-1, 1, 1, "")), Caret(3, 0)),
            outline.enter(all, 3, 0, newKey = -1),
        )
    }

    @Test
    fun `Enter on an empty indented row outdents it, which is how a nested list is ended`() {
        val trailing = with(groceries, milk, eggs, todo(5, "", parent = 1, position = 2), dentist)
        assertEquals(
            Edit(listOf(Change.Move(5, null, 1)), Caret(5, 0)),
            trailing.enter(all, 5, 0, newKey = -1),
        )
    }

    @Test
    fun `Enter on an empty indented row that has children starts its first child instead of moving them out`() {
        val parent =
            with(groceries, todo(5, "", parent = 1), todo(6, "Butter", parent = 5), dentist)
        assertEquals(
            Edit(listOf(Change.Create(-1, 5, 0, "")), Caret(-1, 0)),
            parent.enter(all, 5, 0, newKey = -1),
        )
    }

    @Test
    fun `Backspace on an empty row deletes it and continues at the end of the row above`() {
        val trailing = with(groceries, milk, eggs, todo(5, "", parent = 1, position = 2), dentist)
        assertEquals(
            Edit(listOf(Change.Delete(5)), Caret(3, 4)),
            trailing.deleteBackward(all, 5),
        )
    }

    @Test
    fun `Backspace at the start of a row joins its text to the row above`() {
        assertEquals(
            Edit(listOf(Change.Rename(2, "MilkEggs"), Change.Delete(3)), Caret(2, 4)),
            outline.deleteBackward(all, 3),
        )
    }

    @Test
    fun `Backspace never joins into a deeper row because the text would land inside another list`() {
        assertNull(outline.deleteBackward(all, 4))
    }

    @Test
    fun `Backspace never deletes or joins a row with children so a subtree is never lost`() {
        val empty = with(todo(1, ""), milk)
        assertNull(empty.deleteBackward(all, 1))
        val belowAnother = with(todo(4, "Dentist"), groceries.copy(position = 1), milk)
        assertNull(belowAnother.deleteBackward(all, 1))
    }

    @Test
    fun `Backspace never deletes or joins a row with a note so what was written under it is never lost`() {
        val noted = with(groceries, milk, eggs.copy(note = "free-range"), dentist)
        assertNull(noted.deleteBackward(all, 3))
        val untitled = todo(5, "", parent = 1, position = 2).copy(note = "for the cake")
        assertNull(with(groceries, milk, eggs, untitled, dentist).deleteBackward(all, 5))
    }

    @Test
    fun `Backspace does not join completed rows so a finished item is not reopened by accident`() {
        val done = with(groceries, milk, eggs.copy(completedAt = "2026-10-23 09:00:00"), dentist)
        assertNull(done.deleteBackward(all, 3))
    }

    @Test
    fun `Backspace does not join rows whose text would exceed what the server accepts`() {
        val long =
            with(todo(1, "a".repeat(MAX_TITLE_LENGTH - 50)), todo(2, "b".repeat(51), position = 1))
        assertNull(long.deleteBackward(all, 2))
    }

    @Test
    fun `deleting a row continues at the end of the row above, like Backspace in an empty row`() {
        assertEquals(Edit(listOf(Change.Delete(3)), Caret(2, 4)), outline.delete(all, 3))
    }

    @Test
    fun `deleting the first row continues on the next row at its level, never on a row deleted with it`() {
        assertEquals(
            Edit(listOf(Change.Delete(1, recursive = true)), Caret(4, 7)),
            outline.delete(all, 1),
        )
    }

    @Test
    fun `deleting a row whose todos are all hidden still takes them along, since the server refuses otherwise`() {
        val done = with(groceries, milk.copy(completedAt = "2026-10-23 09:00:00"), dentist)
        assertEquals(
            listOf(Change.Delete(1, recursive = true)),
            done.delete(Viewport(showCompleted = false), 1)?.changes,
        )
    }

    @Test
    fun `indent moves the row to the end of the previous sibling and opens it so the row stays in view`() {
        val folded = with(groceries.copy(collapsed = true), milk, eggs, dentist)
        assertEquals(
            Edit(listOf(Change.Move(4, 1, 2), Change.Collapse(1, false)), Caret(4, 3)),
            folded.indent(all, 4, 3),
        )
    }

    @Test
    fun `indent skips a hidden completed sibling so the row does not vanish into it`() {
        val hiddenDone = todo(5, "Bank", position = 1, done = true)
        val outline = with(groceries, milk, eggs, hiddenDone, dentist.copy(position = 2))
        assertEquals(
            Edit(listOf(Change.Move(4, 1, 2)), Caret(4, 0)),
            outline.indent(Viewport(showCompleted = false), 4, 0),
        )
        assertEquals(
            Edit(listOf(Change.Move(4, 5, 0)), Caret(4, 0)),
            outline.indent(all, 4, 0),
        )
    }

    @Test
    fun `the first row of a list cannot be indented because it has nothing to go under`() {
        assertNull(outline.indent(all, 2, 0))
    }

    @Test
    fun `indent does nothing when the row or a row under it would end up deeper than the server allows`() {
        val parent = MAX_DEPTH - 2L
        val deep =
            outlineOf(
                chain(MAX_DEPTH - 2) +
                    todo(100, "A", parent = parent) +
                    todo(101, "B", parent = parent, position = 1) +
                    todo(102, "Under B", parent = 101) +
                    todo(103, "C", parent = parent, position = 2)
            )
        assertNull(deep.indent(all, 101, 0))
        assertEquals(
            Edit(listOf(Change.Move(103, 101, 1)), Caret(103, 0)),
            deep.indent(all, 103, 0),
        )
    }

    @Test
    fun `outdent places the row right after its old parent`() {
        assertEquals(
            Edit(listOf(Change.Move(2, null, 1)), Caret(2, 1)),
            outline.outdent(2, 1),
        )
    }

    @Test
    fun `outdent does nothing for a top-level row because it has no parent to move out of`() {
        assertNull(outline.outdent(1, 0))
    }

    @Test
    fun `moving swaps a row with its neighbour and carries its subtree, never leaving its parent`() {
        assertEquals(
            Edit(listOf(Change.Move(1, null, 1)), Caret(1, 0)),
            outline.moveDown(all, 1, 0),
        )
        assertEquals(
            Edit(listOf(Change.Move(3, 1, 0)), Caret(3, 2)),
            outline.moveUp(all, 3, 2),
        )
        assertNull(outline.moveUp(all, 2, 0))
        assertNull(outline.moveDown(all, 3, 0))
    }

    @Test
    fun `moving skips hidden completed siblings so every press visibly moves the row`() {
        val outline =
            with(
                todo(1, "A"),
                todo(2, "Done", position = 1, done = true),
                todo(3, "B", position = 2),
            )
        val hidden = Viewport(showCompleted = false)
        assertEquals(
            Edit(listOf(Change.Move(1, null, 2)), Caret(1, 0)),
            outline.moveDown(hidden, 1, 0),
        )
        assertEquals(
            Edit(listOf(Change.Move(3, null, 0)), Caret(3, 0)),
            outline.moveUp(hidden, 3, 0),
        )
    }

    @Test
    fun `moving to another todo puts the row at the end of its children and leaves the cursor where the row was`() {
        assertEquals(
            Edit(listOf(Change.Move(2, 4, 0)), Caret(3, 4)),
            outline.moveTo(all, 2, 4),
        )
    }

    @Test
    fun `moving a row to the end of its own parent sends its final position, which is what the server expects`() {
        assertEquals(
            Edit(listOf(Change.Move(2, 1, 1)), Caret(3, 4)),
            outline.moveTo(all, 2, 1),
        )
    }

    @Test
    fun `moving the last row leaves the cursor on the row above, so typing can go on nearby`() {
        assertEquals(
            Edit(listOf(Change.Move(4, 1, 2)), Caret(3, 4)),
            outline.moveTo(all, 4, 1),
        )
    }

    @Test
    fun `moving leaves a collapsed todo closed so the rows being worked among do not jump around`() {
        val folded = with(groceries.copy(collapsed = true), milk, eggs, dentist)
        assertEquals(listOf(Change.Move(4, 1, 2)), folded.moveTo(all, 4, 1)?.changes)
    }

    @Test
    fun `a row cannot be moved into itself or a todo under it, which the server refuses`() {
        assertNull(outline.moveTo(all, 1, 1))
        assertNull(outline.moveTo(all, 1, 2))
    }

    @Test
    fun `a row cannot be moved where it or a row under it would end up deeper than the server allows`() {
        assertNull(tooDeep.moveTo(all, 100, MAX_DEPTH - 1L))
        assertEquals(
            Change.Move(100, MAX_DEPTH - 2L, 1),
            tooDeep.moveTo(all, 100, MAX_DEPTH - 2L)?.changes?.single(),
        )
    }

    @Test
    fun `destinations list every other todo in outline order with the titles above it, so same names can be told apart`() {
        assertEquals(
            listOf(
                Destination(1, "Groceries", emptyList()),
                Destination(2, "Milk", listOf("Groceries")),
                Destination(3, "Eggs", listOf("Groceries")),
            ),
            outline.destinations(all, 4, ""),
        )
    }

    @Test
    fun `destinations for a shared todo offer every todo because nothing is being moved`() {
        assertEquals(listOf(1L, 2L, 3L, 4L), outline.destinations(all, null, "").map { it.key })
    }

    @Test
    fun `destinations never offer the row itself or anything under it`() {
        assertEquals(listOf(4L), outline.destinations(all, 1, "").map { it.key })
    }

    @Test
    fun `destinations match the title regardless of case, even inside collapsed todos`() {
        val folded = with(groceries.copy(collapsed = true), milk, eggs, dentist)
        assertEquals(listOf(3L), folded.destinations(all, 4, " EGG ").map { it.key })
    }

    @Test
    fun `destinations leave out completed todos and everything under them while completed todos are hidden`() {
        val done = with(groceries.copy(completedAt = "2026-10-23 09:00:00"), milk, eggs, dentist)
        assertEquals(
            listOf(4L),
            done.destinations(Viewport(showCompleted = false), 2, "").map { it.key },
        )
        assertEquals(listOf(1L, 3L, 4L), done.destinations(all, 2, "").map { it.key })
    }

    @Test
    fun `destinations leave out todos where the moved or shared row would end up deeper than the server allows`() {
        fun levels(count: Int) = (1L..count).toList()
        assertEquals(levels(MAX_DEPTH - 2), tooDeep.destinations(all, 100, "").map { it.key })
        assertEquals(
            levels(MAX_DEPTH - 1) + 100L,
            tooDeep.destinations(all, 101, "").map { it.key },
        )
        assertEquals(
            levels(MAX_DEPTH - 1) + listOf(100L, 101L),
            tooDeep.destinations(all, null, "").map { it.key },
        )
    }

    @Test
    fun `destinations leave out todos without a title, which could not be told apart`() {
        val blank = with(groceries.copy(title = " "), milk, eggs, dentist)
        assertEquals(listOf(2L, 3L), blank.destinations(all, 4, "").map { it.key })
    }

    @Test
    fun `completing a row while completed rows are hidden continues on the next row at its level or above, never on one that disappears with it`() {
        val hidden = Viewport(showCompleted = false)
        assertEquals(
            Edit(listOf(Change.Complete(1, true)), Caret(4, 7)),
            outline.toggleCompleted(hidden, 1, 3),
        )
        assertEquals(
            Edit(listOf(Change.Complete(3, true)), Caret(4, 7)),
            outline.toggleCompleted(hidden, 3, 0),
        )
    }

    @Test
    fun `completing the last row while completed rows are hidden continues at the end of the row above`() {
        assertEquals(
            Edit(listOf(Change.Complete(4, true)), Caret(3, 4)),
            outline.toggleCompleted(Viewport(showCompleted = false), 4, 0),
        )
    }

    @Test
    fun `Enter in a done list adds an open row and reopens the todos above it, as the server does`() {
        val done =
            with(
                todo(1, "Groceries", done = true),
                todo(2, "Milk", parent = 1, done = true),
                todo(3, "Eggs", parent = 1, position = 1, done = true),
                dentist,
            )
        val edit = done.enter(all, 2, 4, newKey = -1)!!
        assertEquals(setOf(2L, 3L), edit.changes.fold(done, Outline::apply).done())
    }

    @Test
    fun `indenting an open row under a done todo reopens it, and Undo moves the row back and completes that todo again`() {
        val done =
            with(
                todo(1, "Groceries", done = true),
                todo(2, "Milk", parent = 1, done = true),
                todo(4, "Dentist", position = 1),
            )
        val indent = done.indent(all, 4, 0)!!.changes
        val indented = indent.fold(done, Outline::apply)
        assertEquals(setOf(2L), indented.done())

        val undo = done.undoFor(indent)!!

        assertEquals(listOf(Change.Move(4, null, 1), Change.Complete(1, true)), undo)
        assertEquals(done, undo.fold(indented, Outline::apply))
    }

    @Test
    fun `collapsing is refused on a row without children because there is nothing to hide`() {
        assertNull(outline.toggleCollapsed(2, 0))
        assertEquals(
            Edit(listOf(Change.Collapse(1, true)), Caret(1, 0)),
            outline.toggleCollapsed(1, 0),
        )
    }

    @Test
    fun `a row added from the button goes to the end of the list`() {
        assertEquals(
            Edit(listOf(Change.Create(-1, null, 2, "")), Caret(-1, 0)),
            outline.append(null, -1),
        )
    }

    @Test
    fun `a caret kept from before the title changed elsewhere never lands inside an emoji, so the next keystroke cannot save it as a question mark`() {
        val renamed = "ab\uD83D\uDE00 Dentist"
        assertEquals(4, caretIn(renamed, 3))
        assertEquals(2, caretIn(renamed, 2))
        assertEquals(4, caretIn(renamed, 4))
        assertEquals(renamed.length, caretIn(renamed, 30))
    }
}
