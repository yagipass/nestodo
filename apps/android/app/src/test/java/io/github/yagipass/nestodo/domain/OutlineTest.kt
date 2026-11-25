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

private const val CART = "\uD83D\uDED2"

fun Outline.render(viewport: Viewport = Viewport()): List<String> =
    rows(viewport).map { "  ".repeat(it.depth) + it.title }

fun Outline.done(): Set<Long> = nodes.values.filter { it.completed }.map { it.key }.toSet()

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
    fun `rekeying renames a row everywhere it is referred to, so its parent and the todos under it still find it`() {
        val ids = mapOf(1L to 100L, 2L to 200L)

        val rekeyed = outline.rekeyed { ids[it] ?: it }

        assertEquals(listOf("Groceries", "  Milk", "  Eggs", "Dentist"), rekeyed.render())
        assertEquals(listOf(100L, 4L), rekeyed.roots)
        assertEquals(listOf(200L, 3L), rekeyed[100]?.children)
        assertEquals(100L, rekeyed[200]?.parent)
        assertEquals(100L, rekeyed[3]?.parent)
    }

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

    private val project =
        outlineOf(
            listOf(
                todo(1, "Moving"),
                todo(2, "Boxes", parent = 1),
                todo(3, "Quotes", parent = 2),
                todo(4, "Address", parent = 1, position = 1, done = true),
                todo(5, "Tax", position = 1),
            )
        )

    private val finished =
        outlineOf(
            listOf(
                todo(1, "Moving", done = true),
                todo(2, "Boxes", parent = 1, done = true),
                todo(3, "Quotes", parent = 2, done = true),
                todo(4, "Address", parent = 1, position = 1, done = true),
                todo(5, "Tax", position = 1, done = true),
            )
        )

    @Test
    fun `completing a todo completes every open todo under it at any depth, as the server does, so a done todo never hides open work`() {
        assertEquals(setOf(1L, 2L, 3L, 4L), project.apply(Change.Complete(1, true)).done())
    }

    @Test
    fun `reopening a todo reopens the done todos above it, which are no longer finished, but leaves the ones under it done`() {
        assertEquals(setOf(3L, 4L, 5L), finished.apply(Change.Complete(2, false)).done())
    }

    @Test
    fun `adding a todo under a done todo reopens it and every todo above it, as the server does, since they now have open work under them`() {
        assertEquals(setOf(4L, 5L), finished.apply(Change.Create(-1, 3, 0, "Tape")).done())
    }

    @Test
    fun `moving an open todo under a done one reopens it and the todos above it, while moving a done one keeps them done`() {
        val mixed =
            outlineOf(
                listOf(
                    todo(1, "Moving", done = true),
                    todo(2, "Boxes", parent = 1, done = true),
                    todo(5, "Tax", position = 1),
                    todo(6, "Receipts", parent = 5, done = true),
                    todo(7, "Bank", position = 2, done = true),
                )
            )
        assertEquals(setOf(6L, 7L), mixed.apply(Change.Move(5, 2, 0)).done())
        assertEquals(setOf(1L, 2L, 6L, 7L), mixed.apply(Change.Move(7, 2, 0)).done())
    }

    @Test
    fun `undoing a completion reopens exactly the todos it completed, so a todo done before stays done`() {
        val complete = listOf(Change.Complete(1, true))
        val undo = project.undoFor(complete)!!
        assertEquals(
            listOf(Change.Complete(1, false), Change.Complete(2, false), Change.Complete(3, false)),
            undo,
        )
        assertEquals(project, undo.fold(complete.fold(project, Outline::apply), Outline::apply))
    }

    @Test
    fun `undoing a reopen completes again the todos above it that the reopen took along`() {
        val reopen = listOf(Change.Complete(3, false))
        val undo = finished.undoFor(reopen)!!
        assertEquals(listOf(Change.Complete(1, true)), undo)
        assertEquals(finished, undo.fold(reopen.fold(finished, Outline::apply), Outline::apply))
    }

    @Test
    fun `undoing an add under a done todo deletes the new todo before completing the todos above it again, so the server never completes it on its way out`() {
        val add = listOf(Change.Create(-1, 3, 0, "Tape"))
        val undo = finished.undoFor(add)!!
        assertEquals(listOf(Change.Delete(-1), Change.Complete(1, true)), undo)
        assertEquals(finished, undo.fold(add.fold(finished, Outline::apply), Outline::apply))
    }

    @Test
    fun `undoing a delete inside a done todo brings the todos back done and leaves the todo above done, as the server's restore does`() {
        val delete = listOf(Change.Delete(2, recursive = true))
        val undo = finished.undoFor(delete)!!
        assertEquals(listOf(Change.Restore(2, 1, 0, listOf(finished[2]!!, finished[3]!!))), undo)
        assertEquals(finished, undo.fold(delete.fold(finished, Outline::apply), Outline::apply))
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
    fun `undoing a deletion restores the row under the same key and id so older undo steps and its comments still find it`() {
        val done = outline.apply(Change.Complete(3, true))
        val undo = done.undoFor(listOf(Change.Delete(3)))!!
        assertEquals(listOf(Change.Restore(3, 1, 1, listOf(done[3]!!))), undo)
        assertEquals(done, undo.fold(done.apply(Change.Delete(3)), Outline::apply))
    }

    @Test
    fun `undoing a deletion brings the comment count back, since the server restores the comments with the row`() {
        val logged =
            outlineOf(
                listOf(todo(1, "Groceries"), todo(2, "Milk", parent = 1).copy(commentCount = 3))
            )
        val undo = logged.undoFor(listOf(Change.Delete(2)))!!
        assertEquals(3, undo.fold(logged.apply(Change.Delete(2)), Outline::apply)[2]!!.comments)
    }

    @Test
    fun `undoing a deletion brings the icon back, since the server restores the row with it`() {
        val marked =
            outlineOf(listOf(todo(1, "Groceries"), todo(2, "Milk", parent = 1).copy(icon = CART)))
        val undo = marked.undoFor(listOf(Change.Delete(2)))!!
        assertEquals(CART, undo.fold(marked.apply(Change.Delete(2)), Outline::apply)[2]!!.icon)
    }

    @Test
    fun `undoing an icon change puts back the icon the row had, or none when it had none`() {
        val marked = outline.apply(Change.SetIcon(4, CART))
        assertEquals(
            listOf(Change.SetIcon(4, null)),
            outline.undoFor(listOf(Change.SetIcon(4, CART))),
        )
        assertEquals(
            listOf(Change.SetIcon(4, CART)),
            marked.undoFor(listOf(Change.SetIcon(4, "\uD83E\uDDB7"))),
        )
        assertEquals(
            listOf(Change.SetIcon(4, CART)),
            marked.undoFor(listOf(Change.SetIcon(4, null))),
        )
    }

    @Test
    fun `the server's icon reaches the row as it is, so a row knows it has an icon even of a kind it cannot draw`() {
        val marked =
            outlineOf(
                listOf(
                    todo(1, "Groceries").copy(icon = CART),
                    todo(2, "Bank").copy(icon = "line:bank"),
                )
            )
        assertEquals(listOf(CART, "line:bank"), marked.rows(Viewport()).map { it.icon })
    }

    @Test
    fun `undoing a deletion restores the whole subtree in one step with each completion and fold, as the server restores it by the deleted id`() {
        val held =
            listOf(Change.Collapse(1, true), Change.Complete(3, true)).fold(outline, Outline::apply)
        val delete = listOf(Change.Delete(1, recursive = true))
        val undo = held.undoFor(delete)!!
        assertEquals(
            listOf(Change.Restore(1, null, 0, listOf(held[1]!!, held[2]!!, held[3]!!))),
            undo,
        )
        assertEquals(held, undo.fold(delete.fold(held, Outline::apply), Outline::apply))
    }

    @Test
    fun `an open row restored under a completed todo reopens it, as the server does, so the next reload does not end Undo`() {
        val milk = outline[2]!!
        val restored =
            outline
                .apply(Change.Delete(2))
                .apply(Change.Complete(1, true))
                .apply(Change.Restore(2, 1, 0, listOf(milk)))
        assertFalse(restored[1]!!.completed)
        assertEquals(outline.render(), restored.render())
    }

    @Test
    fun `a row cannot be restored over one that is still there, so Undo never shows a todo twice`() {
        assertFalse(outline.accepts(Change.Restore(2, 1, 0, listOf(outline[2]!!))))
        assertTrue(
            outline.apply(Change.Delete(2)).accepts(Change.Restore(2, 1, 0, listOf(outline[2]!!)))
        )
    }

    @Test
    fun `a row cannot be moved under its own descendant, which would cut it out of the outline`() {
        assertFalse(outline.accepts(Change.Move(1, 2, 0)))
        assertTrue(outline.accepts(Change.Move(4, 2, 0)))
        assertFalse(outline.accepts(listOf(Change.Delete(2), Change.Rename(2, "gone"))))
    }

    private val moving =
        outlineOf(
            listOf(
                todo(1, "Moving"),
                todo(2, "Boxes", parent = 1),
                todo(3, "Quotes", parent = 2),
                todo(4, "Address", parent = 1, position = 1),
                todo(5, "Bank", parent = 4),
                todo(6, "Tax", position = 1),
                todo(7, "Done step", parent = 6, done = true),
            )
        )

    @Test
    fun `each guide line runs from a parent's bullet down to its last child and no further`() {
        assertEquals(
            listOf(
                Guide(emptyList(), stem = true),
                Guide(listOf(true), stem = true),
                Guide(listOf(true, false), stem = false),
                Guide(listOf(false), stem = true),
                Guide(listOf(false, false), stem = false),
                Guide(emptyList(), stem = true),
                Guide(listOf(false), stem = false),
            ),
            guides(moving.rows(Viewport())),
        )
    }

    @Test
    fun `a parent whose children are all hidden gets no stem so no line leads to nothing`() {
        val rows = moving.rows(Viewport(showCompleted = false))
        assertEquals(
            listOf(
                "Moving" to true,
                "Boxes" to true,
                "Quotes" to false,
                "Address" to true,
                "Bank" to false,
                "Tax" to false,
            ),
            rows.zip(guides(rows)) { row, guide -> row.title to guide.stem },
        )
    }
}
