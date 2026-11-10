package io.github.yagipass.nestodo

import android.content.ActivityNotFoundException
import io.github.yagipass.nestodo.access.DeviceLockedException
import io.github.yagipass.nestodo.data.ApiException
import io.github.yagipass.nestodo.domain.Caret
import io.github.yagipass.nestodo.domain.Todo
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class TodoViewModelTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val groceries = Todo(1, null, 0, "Groceries")
    private val milk = Todo(2, 1, 0, "Milk")
    private val eggs = Todo(3, 1, 1, "Eggs")
    private val dentist = Todo(4, null, 1, "Dentist")
    private val repository = FakeRepository(listOf(groceries, milk, eggs, dentist))
    private val original = listOf("Groceries", "  Milk", "  Eggs", "Dentist")
    private val settings = FakeSettings()
    private val backend = FakeBackend(repository)
    private val opened = mutableListOf<HttpUrl>()

    private fun backendAt(base: HttpUrl) = backend.also { opened += base }

    private fun viewModel() =
        TodoViewModel(
                ::backendAt,
                settings,
                timeZone = { "Asia/Tokyo" },
                timeSource = dispatcher.scheduler.timeSource,
            )
            .also { repository.calls.clear() }

    private fun TodoViewModel.outline(): List<String> =
        state.value.rows.map { "  ".repeat(it.depth) + it.title }

    private fun TodoViewModel.caretKey(): Long = state.value.caret!!.key

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `a saved login opens straight to the outline in the order the server keeps`() {
        val state = TodoViewModel(::backendAt, settings).state.value
        assertEquals(AuthState.SignedIn, state.auth)
        assertTrue(state.loaded)
        assertEquals(listOf(0, 1, 1, 0), state.rows.map { it.depth })
        assertEquals(listOf(1L, 2L, 3L, 4L), state.rows.map { it.key })
    }

    @Test
    fun `Enter shows the new row and its cursor before the server answers so typing never waits`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.enter(2, 4)
        assertEquals(listOf("Groceries", "  Milk", "  ", "  Eggs", "Dentist"), viewModel.outline())
        assertEquals(Caret(viewModel.state.value.rows[2].key, 0), viewModel.state.value.caret)
        assertEquals(listOf("POST \"\" under 1 at 1"), repository.calls)
    }

    @Test
    fun `text typed into a new row before its create finishes is saved under the id the server assigned`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.enter(2, 4)
        val key = viewModel.caretKey()
        viewModel.select(key, 0)
        viewModel.setTitle(key, "Butter")
        dispatcher.scheduler.advanceTimeBy(1_000)
        assertEquals(listOf("POST \"\" under 1 at 1"), repository.calls)

        repository.gate!!.complete(Unit)

        assertEquals(
            listOf("POST \"\" under 1 at 1", "PATCH 100 title \"Butter\""),
            repository.calls,
        )
        assertEquals(key, viewModel.state.value.rows[2].key)
    }

    @Test
    fun `a reload keeps the user in a row created here and saves its text under the server's id`() {
        val viewModel = viewModel()
        viewModel.enter(4, 7)
        val key = viewModel.caretKey()
        viewModel.select(key, 0)
        repository.todos = listOf(groceries, milk, eggs, dentist, Todo(100, null, 2, ""))

        viewModel.refresh()
        assertEquals(key, viewModel.state.value.editing)
        viewModel.setTitle(key, "Call at 9am")
        viewModel.blur(key)

        assertEquals(original + "Call at 9am", viewModel.outline())
        assertEquals(
            listOf("POST \"\" under null at 2", "GET", "PATCH 100 title \"Call at 9am\""),
            repository.calls,
        )
    }

    @Test
    fun `typing is saved once after a pause instead of on every keystroke`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Milk,")
        viewModel.setTitle(2, "Milk, oat")
        dispatcher.scheduler.advanceTimeBy(499)
        assertEquals(emptyList<String>(), repository.calls)
        dispatcher.scheduler.advanceTimeBy(2)
        assertEquals(listOf("PATCH 2 title \"Milk, oat\""), repository.calls)
    }

    @Test
    fun `a note is saved once after a pause, on its own, instead of on every keystroke`() {
        val viewModel = viewModel()
        viewModel.selectNote(2, 0)
        viewModel.setNote(2, "oat")
        viewModel.setNote(2, "oat, not soy")
        dispatcher.scheduler.advanceTimeBy(499)
        assertEquals(emptyList<String>(), repository.calls)
        dispatcher.scheduler.advanceTimeBy(2)
        assertEquals(listOf("PATCH 2 note \"oat, not soy\""), repository.calls)
    }

    @Test
    fun `the Note button opens the row's note under the cursor and goes back to the end of its title`() {
        val viewModel = viewModel()
        viewModel.select(2, 2)

        viewModel.toggleNote()
        assertEquals(Caret(2, 0, inNote = true), viewModel.state.value.caret)
        assertEquals(2L, viewModel.state.value.noteOpen)
        viewModel.blur(2)
        viewModel.selectNote(2, 0)
        assertEquals(2L, viewModel.state.value.editing)
        assertTrue(viewModel.state.value.inNote)

        viewModel.toggleNote()
        assertEquals(Caret(2, 4), viewModel.state.value.caret)
        viewModel.blurNote(2)
        viewModel.select(2, 4)
        assertEquals(2L, viewModel.state.value.editing)
        assertFalse(viewModel.state.value.inNote)
        assertNull(viewModel.state.value.noteOpen)
    }

    @Test
    fun `a toolbar action taken while typing a note keeps the cursor in the note`() {
        val viewModel = viewModel()
        viewModel.selectNote(3, 0)
        viewModel.setNote(3, "free-range")
        viewModel.selectNote(3, 10)

        viewModel.indent()

        assertEquals(listOf("Groceries", "  Milk", "    Eggs", "Dentist"), viewModel.outline())
        assertEquals(Caret(3, 10, inNote = true), viewModel.state.value.caret)
        assertEquals(
            listOf("PATCH 3 note \"free-range\"", "PATCH 3 under 2 at 0"),
            repository.calls,
        )
    }

    @Test
    fun `leaving a note of only blank lines removes it so no empty note stays under the row`() {
        val viewModel = viewModel()
        viewModel.selectNote(4, 0)
        viewModel.setNote(4, "\n ")

        viewModel.blurNote(4)

        assertEquals("", viewModel.state.value.rows.last().note)
        assertNull(viewModel.state.value.editing)
        assertNull(viewModel.state.value.noteOpen)
        assertEquals(listOf("PATCH 4 note \"\""), repository.calls)
    }

    @Test
    fun `leaving a note of only blank lines counts as a new revision, so the screen replaces the blank lines it remembers with the cleared note`() {
        val viewModel = viewModel()
        viewModel.selectNote(4, 0)
        viewModel.setNote(4, "\n\n")
        val revision = viewModel.state.value.revision

        viewModel.blurNote(4)

        assertTrue(viewModel.state.value.revision > revision)
    }

    @Test
    fun `leaving a row saves its text at once so nothing typed is lost when the app goes away`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Oat milk")
        viewModel.blur(2)
        assertEquals(listOf("PATCH 2 title \"Oat milk\""), repository.calls)
        assertNull(viewModel.state.value.editing)
    }

    @Test
    fun `unsaved text goes to the server before a move so the server applies edits in the order made`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Oat milk")
        viewModel.select(2, 8)
        viewModel.moveDown()
        assertEquals(
            listOf("PATCH 2 title \"Oat milk\"", "PATCH 2 under 1 at 1"),
            repository.calls,
        )
        assertEquals(listOf("Groceries", "  Eggs", "  Oat milk", "Dentist"), viewModel.outline())
        assertEquals(Caret(2, 8), viewModel.state.value.caret)
    }

    @Test
    fun `splitting a row saves the tail as a new row before shortening the head, so a failure in between never loses text`() {
        val viewModel = viewModel()
        viewModel.enter(4, 4)
        assertEquals(
            listOf("POST \"ist\" under null at 2", "PATCH 4 title \"Dent\""),
            repository.calls,
        )
        assertEquals(original.dropLast(1) + listOf("Dent", "ist"), viewModel.outline())
    }

    @Test
    fun `Enter twice at the end of a nested list ends the list with a row back at the outer level`() {
        val viewModel = viewModel()
        viewModel.enter(3, 4)
        viewModel.enter(viewModel.caretKey(), 0)
        assertEquals(
            listOf("POST \"\" under 1 at 2", "PATCH 100 under null at 1"),
            repository.calls,
        )
        assertEquals(original.dropLast(1) + listOf("", "Dentist"), viewModel.outline())
    }

    @Test
    fun `indenting moves the row under the previous sibling and opens it so the row stays in view`() {
        repository.todos = listOf(groceries.copy(collapsed = true), milk, eggs, dentist)
        val viewModel = viewModel()
        viewModel.select(4, 0)
        viewModel.indent()
        assertEquals(listOf("PATCH 4 under 1 at 2", "PATCH 1 collapsed false"), repository.calls)
        assertEquals(listOf("Groceries", "  Milk", "  Eggs", "  Dentist"), viewModel.outline())
    }

    @Test
    fun `moving a row under another todo sends one move and leaves the cursor on the row that took its place`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.startMove()
        viewModel.blur(2)
        viewModel.moveTo(4)
        assertEquals(listOf("PATCH 2 under 4 at 0"), repository.calls)
        assertEquals(listOf("Groceries", "  Eggs", "Dentist", "  Milk"), viewModel.outline())
        assertEquals(Caret(3, 4), viewModel.state.value.caret)
        assertNull(viewModel.state.value.moving)
    }

    @Test
    fun `closing the move picker without choosing returns the cursor to where it was in the row`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        viewModel.startMove()
        viewModel.blur(4)
        viewModel.cancelMove()
        assertEquals(emptyList<String>(), repository.calls)
        assertEquals(Caret(4, 3), viewModel.state.value.caret)
        assertNull(viewModel.state.value.moving)
    }

    @Test
    fun `undoing a move puts the row back in its old place and the cursor back in it`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.startMove()
        viewModel.moveTo(4)
        viewModel.undo()
        assertEquals(listOf("PATCH 2 under 4 at 0", "PATCH 2 under 1 at 0"), repository.calls)
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(2, 4), viewModel.state.value.caret)
    }

    @Test
    fun `outdenting places the row right after its old parent`() {
        val viewModel = viewModel()
        viewModel.select(2, 0)
        viewModel.outdent()
        assertEquals(listOf("PATCH 2 under null at 1"), repository.calls)
        assertEquals(listOf("Groceries", "  Eggs", "Milk", "Dentist"), viewModel.outline())
    }

    @Test
    fun `tapping another row's bullet folds it without pulling the cursor and keyboard away from the row being typed in`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)

        viewModel.toggleCollapsed(1)

        assertEquals(listOf("PATCH 1 collapsed true"), repository.calls)
        assertEquals(listOf("Groceries", "Dentist"), viewModel.outline())
        assertEquals(4L, viewModel.state.value.editing)
        assertNull(viewModel.state.value.caret)

        viewModel.undo()

        assertEquals(listOf("PATCH 1 collapsed true", "PATCH 1 collapsed false"), repository.calls)
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(4, 3), viewModel.state.value.caret)
    }

    @Test
    fun `folding away the row being typed in saves its text first so nothing typed is lost`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Oat milk")

        viewModel.toggleCollapsed(1)

        assertEquals(
            listOf("PATCH 2 title \"Oat milk\"", "PATCH 1 collapsed true"),
            repository.calls,
        )
        assertEquals(listOf("Groceries", "Dentist"), viewModel.outline())
        assertNull(viewModel.state.value.editing)
    }

    @Test
    fun `a completed row is struck through at once and hidden with its subtree when completed rows are hidden`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.select(1, 0)
        viewModel.toggleCompleted()
        assertEquals(listOf("PATCH 1 completed true"), repository.calls)
        val rows = viewModel.state.value.rows
        assertTrue(rows[0].completed)
        assertTrue(rows[1].dimmed && !rows[1].completed)
        repository.gate!!.complete(Unit)

        viewModel.toggleShowCompleted()

        assertEquals(listOf("Dentist"), viewModel.outline())
    }

    @Test
    fun `completing rows one after another while completed rows are hidden keeps the cursor and toolbar on the next row`() {
        val viewModel = viewModel()
        viewModel.toggleShowCompleted()
        viewModel.select(2, 2)

        viewModel.toggleCompleted()

        assertEquals(listOf("Groceries", "  Eggs", "Dentist"), viewModel.outline())
        assertEquals(3L, viewModel.state.value.editing)
        assertEquals(Caret(3, 4), viewModel.state.value.caret)

        viewModel.toggleCompleted()

        assertEquals(listOf("PATCH 2 completed true", "PATCH 3 completed true"), repository.calls)
        assertEquals(listOf("Groceries", "Dentist"), viewModel.outline())
        assertEquals(4L, viewModel.state.value.editing)
        assertEquals(Caret(4, 7), viewModel.state.value.caret)
    }

    @Test
    fun `hidden completed rows stay hidden when the app is closed and opened again`() {
        repository.todos = listOf(groceries, milk, eggs, dentist.copy(completedAt = "2026-10-26"))
        viewModel().toggleShowCompleted()

        assertEquals(listOf("Groceries", "  Milk", "  Eggs"), viewModel().outline())
    }

    @Test
    fun `hidden completed rows stay hidden after signing out and back in`() {
        repository.todos = listOf(groceries, milk, eggs, dentist.copy(completedAt = "2026-10-26"))
        val viewModel = viewModel()
        viewModel.toggleShowCompleted()

        viewModel.signOut()
        viewModel.connect()

        assertEquals(listOf("Groceries", "  Milk", "  Eggs"), viewModel.outline())
    }

    @Test
    fun `hidden completed rows stay hidden after an expired login is renewed`() {
        repository.todos = listOf(groceries, milk, eggs, dentist.copy(completedAt = "2026-10-26"))
        val viewModel = viewModel()
        viewModel.toggleShowCompleted()
        repository.failure = ApiException.NotSignedIn()
        viewModel.select(2, 0)
        viewModel.toggleCompleted()

        viewModel.connect()

        assertEquals(listOf("Groceries", "  Milk", "  Eggs"), viewModel.outline())
    }

    @Test
    fun `Backspace at the start of a row merges it into the row above and keeps the cursor at the seam`() {
        val viewModel = viewModel()
        assertTrue(viewModel.deleteBackward(3))
        assertEquals(listOf("PATCH 2 title \"MilkEggs\"", "DELETE 3"), repository.calls)
        assertEquals(listOf("Groceries", "  MilkEggs", "Dentist"), viewModel.outline())
        assertEquals(Caret(2, 4), viewModel.state.value.caret)
    }

    @Test
    fun `a toolbar action after a merge keeps the cursor at the seam instead of where it was before`() {
        val viewModel = viewModel()
        viewModel.select(3, 0)
        viewModel.deleteBackward(3)
        viewModel.toggleCompleted()
        assertEquals(Caret(2, 4), viewModel.state.value.caret)
    }

    @Test
    fun `Backspace in an empty row deletes it and puts the cursor at the end of the row above`() {
        val viewModel = viewModel()
        viewModel.enter(2, 4)
        assertTrue(viewModel.deleteBackward(viewModel.caretKey()))
        assertEquals(listOf("POST \"\" under 1 at 1", "DELETE 100"), repository.calls)
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(2, 4), viewModel.state.value.caret)
    }

    @Test
    fun `Backspace that cannot merge leaves the outline alone so the keyboard handles the key`() {
        val viewModel = viewModel()
        assertFalse(viewModel.deleteBackward(1))
        assertFalse(viewModel.deleteBackward(4))
        assertEquals(emptyList<String>(), repository.calls)
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `the Delete button asks first and deletes nothing until the user confirms`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        viewModel.startDelete()
        viewModel.blur(4)
        assertEquals(emptyList<String>(), repository.calls)
        assertEquals(original, viewModel.outline())

        viewModel.confirmDelete()

        assertEquals(listOf("DELETE 4"), repository.calls)
        assertEquals(listOf("Groceries", "  Milk", "  Eggs"), viewModel.outline())
        assertEquals(Caret(3, 4), viewModel.state.value.caret)
        assertNull(viewModel.state.value.deleting)
    }

    @Test
    fun `cancelling a delete keeps the row and returns the cursor to where it was in the row`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        viewModel.startDelete()
        viewModel.blur(4)
        viewModel.cancelDelete()
        assertEquals(emptyList<String>(), repository.calls)
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(4, 3), viewModel.state.value.caret)
        assertNull(viewModel.state.value.deleting)
    }

    @Test
    fun `undoing a delete brings the row back with the cursor where it was`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        viewModel.startDelete()
        viewModel.confirmDelete()
        viewModel.undo()
        assertEquals(listOf("DELETE 4", "POST \"Dentist\" under null at 1"), repository.calls)
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(4, 3), viewModel.state.value.caret)
    }

    @Test
    fun `deleting a row with todos under it removes them all in one request that asks the server for the subtree`() {
        val viewModel = viewModel()
        viewModel.select(1, 0)
        viewModel.startDelete()
        viewModel.confirmDelete()
        assertEquals(listOf("DELETE 1 with its subtree"), repository.calls)
        assertEquals(listOf("Dentist"), viewModel.outline())
        assertEquals(Caret(4, 7), viewModel.state.value.caret)
    }

    @Test
    fun `undoing a delete brings back the todos under it too, each saved under its parent's new id`() {
        val viewModel = viewModel()
        viewModel.select(1, 3)
        viewModel.startDelete()
        viewModel.confirmDelete()

        viewModel.undo()

        assertEquals(
            listOf(
                "DELETE 1 with its subtree",
                "POST \"Groceries\" under null at 0",
                "POST \"Milk\" under 100 at 0",
                "POST \"Eggs\" under 100 at 1",
            ),
            repository.calls,
        )
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(1, 3), viewModel.state.value.caret)
    }

    @Test
    fun `undo reverses structural edits newest first by sending the inverse changes`() {
        val viewModel = viewModel()
        viewModel.select(4, 0)
        viewModel.indent()
        viewModel.moveUp()
        assertTrue(viewModel.state.value.canUndo)

        viewModel.undo()
        viewModel.undo()

        assertEquals(
            listOf(
                "PATCH 4 under 1 at 2",
                "PATCH 4 under 1 at 1",
                "PATCH 4 under 1 at 2",
                "PATCH 4 under null at 1",
            ),
            repository.calls,
        )
        assertEquals(original, viewModel.outline())
        assertFalse(viewModel.state.value.canUndo)
    }

    @Test
    fun `Undo takes back a burst of typing with pauses of up to a second in one step, and the next burst after a longer pause in another`() {
        val viewModel = viewModel()
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist ")
        dispatcher.scheduler.advanceTimeBy(1_000)
        viewModel.setTitle(4, "Dentist at")
        dispatcher.scheduler.advanceTimeBy(1_000)
        viewModel.setTitle(4, "Dentist at 9")
        dispatcher.scheduler.advanceTimeBy(1_001)
        viewModel.setTitle(4, "Dentist at 9am")

        viewModel.undo()
        assertEquals("Dentist at 9", viewModel.outline().last())
        assertEquals(Caret(4, 12), viewModel.state.value.caret)
        viewModel.undo()
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(4, 7), viewModel.state.value.caret)
        assertEquals(
            listOf(
                "PATCH 4 title \"Dentist \"",
                "PATCH 4 title \"Dentist at\"",
                "PATCH 4 title \"Dentist at 9\"",
                "PATCH 4 title \"Dentist at 9\"",
                "PATCH 4 title \"Dentist\"",
            ),
            repository.calls,
        )
    }

    @Test
    fun `an IME conversion stays one Undo step even while the user stops to pick a candidate`() {
        val viewModel = viewModel()
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist はいしゃ", composing = true)
        dispatcher.scheduler.advanceTimeBy(3_000)
        viewModel.setTitle(4, "Dentist 歯医者", composing = true)
        viewModel.setTitle(4, "Dentist 歯医者", composing = false)
        dispatcher.scheduler.advanceTimeBy(2_000)
        viewModel.setTitle(4, "Dentist 歯医者に", composing = true)

        viewModel.undo()
        assertEquals("Dentist 歯医者", viewModel.outline().last())
        viewModel.undo()
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `leaving the row or making another edit starts a new Undo step, so one Undo never takes back more`() {
        val viewModel = viewModel()
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist!")
        viewModel.blur(4)
        viewModel.select(4, 8)
        viewModel.setTitle(4, "Dentist!!")
        viewModel.indent()
        viewModel.setTitle(4, "Dentist!!!")

        val titles =
            List(4) {
                viewModel.undo()
                viewModel.outline().last().trim()
            }
        assertEquals(listOf("Dentist!!", "Dentist!!", "Dentist!", "Dentist"), titles)
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `typing right after undoing another edit starts a new Undo step, so one Undo never also takes back what was typed before that edit`() {
        val viewModel = viewModel()
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist!")
        viewModel.indent()
        viewModel.undo()
        viewModel.setTitle(4, "Dentist!!")

        val titles =
            List(2) {
                viewModel.undo()
                viewModel.outline().last()
            }
        assertEquals(listOf("Dentist!", "Dentist"), titles)
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `undoing typing puts the cursor where the text was typed, not at the end of the title`() {
        val viewModel = viewModel()
        viewModel.select(2, 0)
        viewModel.setTitle(2, "Oat Milk")

        viewModel.undo()

        assertEquals(original, viewModel.outline())
        assertEquals(Caret(2, 0), viewModel.state.value.caret)
    }

    @Test
    fun `undoing an emoji typed before another emoji leaves the cursor outside both, so the next Enter can't split one`() {
        val viewModel = viewModel()
        viewModel.select(4, 0)
        viewModel.setTitle(4, "\uD83D\uDE00Dentist")
        dispatcher.scheduler.advanceTimeBy(1_500)
        viewModel.setTitle(4, "\uD83D\uDC4D\uD83D\uDE00Dentist")

        viewModel.undo()

        assertEquals("\uD83D\uDE00Dentist", viewModel.outline().last())
        assertEquals(Caret(4, 0), viewModel.state.value.caret)
    }

    @Test
    fun `undoing an emoji swapped for another mid-title leaves the cursor after it, so the next Enter can't split one`() {
        repository.todos =
            listOf(groceries, milk, eggs, dentist.copy(title = "Dentist \uD83D\uDD70 9am"))
        val viewModel = viewModel()
        viewModel.select(4, 10)
        viewModel.setTitle(4, "Dentist  9am")
        viewModel.setTitle(4, "Dentist \uD83E\uDD70 9am")

        viewModel.undo()

        assertEquals("Dentist \uD83D\uDD70 9am", viewModel.outline().last())
        assertEquals(Caret(4, 10), viewModel.state.value.caret)
    }

    @Test
    fun `undoing typing in a note puts the cursor back in the note`() {
        val viewModel = viewModel()
        viewModel.selectNote(2, 0)
        viewModel.setNote(2, "oat")

        viewModel.undo()

        assertEquals("", viewModel.state.value.outline[2]!!.note)
        assertEquals(Caret(2, 0, inNote = true), viewModel.state.value.caret)
        assertEquals(2L, viewModel.state.value.noteOpen)
    }

    @Test
    fun `returning to the app keeps what Undo can reverse when nothing changed elsewhere`() {
        val viewModel = viewModel()
        viewModel.select(4, 0)
        viewModel.indent()
        repository.todos = listOf(groceries, milk, eggs, dentist.copy(parentId = 1, position = 2))

        viewModel.onForeground()

        assertTrue(viewModel.state.value.canUndo)
        viewModel.undo()
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `returning to the app after a change made elsewhere ends what Undo can reverse, so Undo never overwrites it`() {
        val viewModel = viewModel()
        viewModel.select(4, 0)
        viewModel.indent()
        repository.todos =
            listOf(
                groceries,
                milk.copy(title = "Oat milk"),
                eggs,
                dentist.copy(parentId = 1, position = 2),
            )

        viewModel.onForeground()
        viewModel.undo()

        assertFalse(viewModel.state.value.canUndo)
        assertEquals(listOf("PATCH 4 under 1 at 2", "GET"), repository.calls)
        assertEquals(listOf("Groceries", "  Oat milk", "  Eggs", "  Dentist"), viewModel.outline())
    }

    @Test
    fun `line breaks pasted into a title become one space each, as the server saves them, so returning to the app keeps Undo`() {
        val viewModel = viewModel()
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist\r\nat 9am\rsharp")
        viewModel.blur(4)
        repository.todos =
            listOf(groceries, milk, eggs, dentist.copy(title = "Dentist at 9am sharp"))

        viewModel.onForeground()

        assertEquals(listOf("PATCH 4 title \"Dentist at 9am sharp\"", "GET"), repository.calls)
        assertEquals("Dentist at 9am sharp", viewModel.outline().last())
        assertTrue(viewModel.state.value.canUndo)
    }

    @Test
    fun `undoing a merge brings the row back, and later edits to it reach the server under its new id`() {
        val viewModel = viewModel()
        viewModel.deleteBackward(3)
        viewModel.undo()
        assertEquals(original, viewModel.outline())
        assertEquals(Caret(3, 0), viewModel.state.value.caret)

        viewModel.select(3, 4)
        viewModel.setTitle(3, "Eggs!")
        viewModel.blur(3)

        assertEquals(
            listOf(
                "PATCH 2 title \"MilkEggs\"",
                "DELETE 3",
                "POST \"Eggs\" under 1 at 1",
                "PATCH 2 title \"Milk\"",
                "PATCH 100 title \"Eggs!\"",
            ),
            repository.calls,
        )
    }

    @Test
    fun `a change the server rejects or never receives is reported and the outline is reloaded so it never shows unsaved work`() {
        for (failure in listOf(ApiException.Invalid(), IOException("offline"))) {
            val viewModel = viewModel()
            repository.failure = failure
            viewModel.select(2, 0)
            viewModel.outdent()
            assertEquals(listOf("PATCH 2 under null at 1", "GET"), repository.calls)
            assertEquals(original, viewModel.outline())
            assertNotNull(viewModel.state.value.notice)
            assertFalse(viewModel.state.value.canUndo)
        }
    }

    @Test
    fun `a change cut off because the phone locked keeps the member signed in and asks them to unlock instead of blaming the connection`() {
        val viewModel = viewModel()
        repository.failure = DeviceLockedException()
        viewModel.select(2, 0)
        viewModel.outdent()

        assertEquals(AuthState.SignedIn, viewModel.state.value.auth)
        assertEquals("Unlock your phone, then try again.", viewModel.state.value.notice?.text)
    }

    @Test
    fun `deleting a todo already deleted elsewhere counts as done and keeps the edits made meanwhile and Undo`() {
        val viewModel = viewModel()
        repository.todos = listOf(groceries, milk, eggs)
        repository.gate = CompletableDeferred()
        viewModel.select(4, 3)
        viewModel.startDelete()
        viewModel.confirmDelete()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Milk, oat")
        viewModel.blur(2)
        viewModel.select(3, 0)
        viewModel.toggleCompleted()
        repository.failure = ApiException.NotFound(4)

        repository.gate!!.complete(Unit)

        assertEquals(
            listOf("DELETE 4", "PATCH 2 title \"Milk, oat\"", "PATCH 3 completed true"),
            repository.calls,
        )
        assertEquals(listOf("Groceries", "  Milk, oat", "  Eggs"), viewModel.outline())
        assertTrue(viewModel.state.value.rows[2].completed)
        assertNull(viewModel.state.value.notice)
        assertTrue(viewModel.state.value.canUndo)
    }

    @Test
    fun `deleting a todo that gained a todo under it elsewhere is reported and reloaded, so it never vanishes only here`() {
        val viewModel = viewModel()
        repository.todos = listOf(groceries, milk, eggs, dentist, Todo(5, 4, 0, "Insurance card"))
        repository.failure = ApiException.Server(409)
        viewModel.select(4, 3)
        viewModel.startDelete()
        viewModel.confirmDelete()

        assertEquals(listOf("DELETE 4", "GET"), repository.calls)
        assertEquals(original + "  Insurance card", viewModel.outline())
        assertEquals("The server returned an error (409).", viewModel.state.value.notice?.text)
        assertFalse(viewModel.state.value.canUndo)
    }

    @Test
    fun `editing a todo deleted elsewhere says it was deleted and reloads, so the edit is never taken as saved`() {
        val viewModel = viewModel()
        repository.todos = listOf(groceries, milk, eggs)
        repository.failure = ApiException.NotFound(4)
        viewModel.select(4, 7)
        viewModel.setTitle(4, "Dentist at 9")
        viewModel.blur(4)

        assertEquals(listOf("PATCH 4 title \"Dentist at 9\"", "GET"), repository.calls)
        assertEquals(listOf("Groceries", "  Milk", "  Eggs"), viewModel.outline())
        assertEquals("This todo has been deleted.", viewModel.state.value.notice?.text)
        assertFalse(viewModel.state.value.canUndo)
    }

    @Test
    fun `a link no app can open is reported where every other message is, so the tap is never silently ignored`() {
        val viewModel = viewModel()

        viewModel.openLink("https://example.com/") { throw ActivityNotFoundException() }

        assertEquals("No app can open this link.", viewModel.state.value.notice?.text)
    }

    @Test
    fun `a link an app opens is handed over as tapped and reports nothing, so no failure is claimed`() {
        val viewModel = viewModel()
        val opened = mutableListOf<String>()

        viewModel.openLink("https://example.com/") { opened += it }

        assertEquals(listOf("https://example.com/"), opened)
        assertNull(viewModel.state.value.notice)
    }

    @Test
    fun `returning to the app with a row still focused shows what was changed on the web meanwhile`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        repository.todos = listOf(groceries, milk.copy(title = "Oat milk"), eggs, dentist)

        viewModel.onForeground()

        assertEquals(listOf("GET"), repository.calls)
        assertEquals(listOf("Groceries", "  Oat milk", "  Eggs", "Dentist"), viewModel.outline())
        assertEquals(2L, viewModel.state.value.editing)
    }

    @Test
    fun `a toolbar action after a reload renamed the focused row keeps the cursor at the end of the new title, where the screen put it, instead of where it was before`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        repository.todos = listOf(groceries, milk, eggs, dentist.copy(title = "ab😀 Dentist"))
        viewModel.onForeground()

        viewModel.indent()

        assertEquals(Caret(4, "ab😀 Dentist".length), viewModel.state.value.caret)
    }

    @Test
    fun `a toolbar action after a reload changed the note being typed in keeps the cursor at the end of the new note, where the screen put it, instead of where it was before`() {
        repository.todos = listOf(groceries, milk, eggs.copy(note = "free-range"), dentist)
        val viewModel = viewModel()
        viewModel.selectNote(3, 4)
        repository.todos = listOf(groceries, milk, eggs.copy(note = "organic, free-range"), dentist)
        viewModel.onForeground()

        viewModel.indent()

        assertEquals(
            Caret(3, "organic, free-range".length, inNote = true),
            viewModel.state.value.caret,
        )
    }

    @Test
    fun `a toolbar action after a reload that left the focused title as it was keeps the cursor where the user put it, as the screen does, so the caret never jumps to the end`() {
        val viewModel = viewModel()
        viewModel.select(4, 3)
        repository.todos = listOf(groceries, milk.copy(title = "Oat milk"), eggs, dentist)
        viewModel.onForeground()

        viewModel.indent()

        assertEquals(Caret(4, 3), viewModel.state.value.caret)
    }

    @Test
    fun `a toolbar action after a reload that renamed only the row whose note is being typed in keeps the cursor where the user put it in the note, as the screen does`() {
        repository.todos = listOf(groceries, milk, eggs.copy(note = "free-range"), dentist)
        val viewModel = viewModel()
        viewModel.selectNote(3, 4)
        repository.todos =
            listOf(groceries, milk, eggs.copy(title = "Brown eggs", note = "free-range"), dentist)
        viewModel.onForeground()

        viewModel.indent()

        assertEquals(Caret(3, 4, inNote = true), viewModel.state.value.caret)
    }

    @Test
    fun `returning to the app after the first load failed loads the outline instead of spinning forever`() {
        repository.failure = IOException("offline")
        val viewModel = viewModel()
        assertFalse(viewModel.state.value.loaded)

        viewModel.onForeground()

        assertEquals(listOf("GET"), repository.calls)
        assertTrue(viewModel.state.value.loaded)
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `a load that succeeds after a failed one takes back its Retry, so a stale Retry never reloads over unsent typing`() {
        repository.failure = IOException("offline")
        val viewModel = viewModel()
        assertNotNull(viewModel.state.value.notice?.retry)

        viewModel.onForeground()

        assertTrue(viewModel.state.value.loaded)
        assertEquals(original, viewModel.outline())
        assertNull(viewModel.state.value.notice)
    }

    @Test
    fun `stats opened while the retried first load is in flight do not leave the outline spinning forever`() {
        repository.failure = IOException("offline")
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.onForeground()
        viewModel.openStats()

        repository.gate!!.complete(Unit)

        assertEquals(listOf("GET", "GET stats in Asia/Tokyo"), repository.calls)
        assertTrue(viewModel.state.value.loaded)
        assertEquals(original, viewModel.outline())
    }

    @Test
    fun `returning to the app waits until every change is saved so a reload never drops one`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        viewModel.setTitle(2, "Oat milk")
        viewModel.onForeground()
        assertEquals(emptyList<String>(), repository.calls)

        repository.gate = CompletableDeferred()
        viewModel.toggleCompleted()
        viewModel.onForeground()
        repository.gate!!.complete(Unit)
        assertEquals(
            listOf("PATCH 2 title \"Oat milk\"", "PATCH 2 completed true"),
            repository.calls,
        )

        viewModel.onForeground()

        assertEquals(
            listOf("PATCH 2 title \"Oat milk\"", "PATCH 2 completed true", "GET"),
            repository.calls,
        )
    }

    @Test
    fun `text typed while the reload on return is in flight is kept instead of the older server copy`() {
        val viewModel = viewModel()
        viewModel.select(2, 4)
        repository.gate = CompletableDeferred()
        viewModel.onForeground()

        viewModel.setTitle(2, "Milk, oat")
        repository.gate!!.complete(Unit)
        dispatcher.scheduler.advanceTimeBy(501)

        assertEquals(listOf("Groceries", "  Milk, oat", "  Eggs", "Dentist"), viewModel.outline())
        assertEquals(listOf("GET", "PATCH 2 title \"Milk, oat\""), repository.calls)
    }

    @Test
    fun `an expired login returns to the login screen with the reason`() {
        val viewModel = viewModel()
        repository.failure = ApiException.NotSignedIn()
        viewModel.select(2, 0)
        viewModel.toggleCompleted()
        val state = viewModel.state.value
        assertEquals(AuthState.SignedOut, state.auth)
        assertTrue(state.rows.isEmpty())
        assertNotNull(state.loginError)
    }

    @Test
    fun `a first launch asks for the server instead of connecting anywhere`() {
        settings.baseUrl = null
        val viewModel = viewModel()
        assertEquals(AuthState.NoServer, viewModel.state.value.auth)
        assertTrue(opened.isEmpty())
    }

    @Test
    fun `an address copied from the browser is saved as the server's https root, so the API and the sign-in go to the server and not to that page`() {
        settings.baseUrl = null
        val viewModel = viewModel()

        viewModel.setServer(" Nestodo.example.com/today?view=all ")

        val state = viewModel.state.value
        assertEquals("https://nestodo.example.com/".toHttpUrl(), settings.baseUrl)
        assertEquals(listOf("https://nestodo.example.com/".toHttpUrl()), opened)
        assertEquals(AuthState.SignedIn, state.auth)
        assertEquals("nestodo.example.com", state.server)
        assertTrue(state.loaded)
    }

    @Test
    fun `a server behind Access is saved before signing in, so the sign-in that follows goes to it`() {
        settings.baseUrl = null
        backend.signedIn = false
        val viewModel = viewModel()

        viewModel.setServer("nestodo.example.com")

        assertEquals(AuthState.SignedOut, viewModel.state.value.auth)
        assertEquals("https://nestodo.example.com/".toHttpUrl(), settings.baseUrl)
    }

    @Test
    fun `an address that can't be reached is not saved, so the next launch asks again instead of failing on a typo`() {
        settings.baseUrl = null
        backend.failure = IOException("no such host")
        val viewModel = viewModel()

        viewModel.setServer("nestodo.exmaple.com")

        val state = viewModel.state.value
        assertEquals(AuthState.NoServer, state.auth)
        assertEquals(
            "Can't reach this server. Check the address and your connection.",
            state.loginError,
        )
        assertEquals("nestodo.exmaple.com", state.server)
        assertNull(settings.baseUrl)
    }

    @Test
    fun `a site that answers without a todo list is not saved, since the outline could never load from it`() {
        settings.baseUrl = null
        repository.failure = ApiException.Server(404)
        val viewModel = viewModel()

        viewModel.setServer("example.com")

        val state = viewModel.state.value
        assertEquals(AuthState.NoServer, state.auth)
        assertEquals("Couldn't find Nestodo at this address.", state.loginError)
        assertNull(settings.baseUrl)
    }

    @Test
    fun `text that is not an address asks for one instead of trying to connect`() {
        settings.baseUrl = null
        val viewModel = viewModel()

        viewModel.setServer("my server")

        assertEquals(AuthState.NoServer, viewModel.state.value.auth)
        assertEquals(
            "Enter an address such as nestodo.example.com.",
            viewModel.state.value.loginError,
        )
        assertTrue(opened.isEmpty())
    }

    @Test
    fun `a plain http address is refused before any request, so the sign-in and the todos never travel unencrypted`() {
        settings.baseUrl = null
        val viewModel = viewModel()

        viewModel.setServer("http://nestodo.example.com")

        assertEquals(
            "Enter an address that starts with https://.",
            viewModel.state.value.loginError,
        )
        assertTrue(opened.isEmpty())
        assertNull(settings.baseUrl)
    }

    @Test
    fun `a debug build reaches the dev server over http on a host its network config allows`() {
        settings.baseUrl = null
        val viewModel =
            TodoViewModel(::backendAt, settings, cleartextAllowed = { it == "10.0.2.2" })

        viewModel.setServer("http://10.0.2.2:5173")

        assertEquals("http://10.0.2.2:5173/".toHttpUrl(), settings.baseUrl)
        assertEquals("http://10.0.2.2:5173", viewModel.state.value.server)
    }

    @Test
    fun `changing the server signs out of the old one and forgets it, so its sign-in is not left on the phone`() {
        val viewModel = viewModel()

        viewModel.changeServer()

        val state = viewModel.state.value
        assertEquals(AuthState.NoServer, state.auth)
        assertTrue(state.rows.isEmpty())
        assertTrue(backend.loggedOut)
        assertNull(settings.baseUrl)
        assertEquals("nestodo.example.com", state.server)
    }

    @Test
    fun `stats are fetched after the changes made before opening them, so a todo just completed is counted`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.select(2, 0)
        viewModel.toggleCompleted()
        viewModel.openStats()
        assertEquals(StatsState.Loading, viewModel.state.value.stats)

        repository.gate!!.complete(Unit)

        assertEquals(listOf("PATCH 2 completed true", "GET stats in Asia/Tokyo"), repository.calls)
        assertEquals(StatsState.Loaded(repository.days), viewModel.state.value.stats)
    }

    @Test
    fun `stats opened while the outline reloads still arrive instead of loading forever`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.refresh()
        viewModel.openStats()

        repository.gate!!.complete(Unit)

        assertEquals(listOf("GET", "GET stats in Asia/Tokyo"), repository.calls)
        assertEquals(StatsState.Loaded(repository.days), viewModel.state.value.stats)
    }

    @Test
    fun `stats that fail to load say why and load when opened again`() {
        val viewModel = viewModel()
        repository.failure = IOException("offline")
        viewModel.openStats()
        assertEquals(
            StatsState.Failed("Can't connect. Check your connection."),
            viewModel.state.value.stats,
        )

        viewModel.openStats()

        assertEquals(StatsState.Loaded(repository.days), viewModel.state.value.stats)
    }

    @Test
    fun `stats that arrive after going back to the outline do not open the stats screen again`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.openStats()
        viewModel.closeStats()

        repository.gate!!.complete(Unit)

        assertNull(viewModel.state.value.stats)
    }

    @Test
    fun `stats asked for before leaving and reopening the screen are not shown on the reopened one`() {
        val viewModel = viewModel()
        val first = CompletableDeferred<Unit>()
        repository.gate = first
        viewModel.openStats()
        viewModel.closeStats()
        viewModel.openStats()
        repository.gate = CompletableDeferred()

        first.complete(Unit)

        assertEquals(StatsState.Loading, viewModel.state.value.stats)
        repository.gate!!.complete(Unit)
        assertEquals(
            listOf("GET stats in Asia/Tokyo", "GET stats in Asia/Tokyo"),
            repository.calls,
        )
        assertEquals(StatsState.Loaded(repository.days), viewModel.state.value.stats)
    }

    @Test
    fun `an expired login while opening stats returns to the login screen with the reason`() {
        val viewModel = viewModel()
        repository.failure = ApiException.NotSignedIn()
        viewModel.openStats()
        val state = viewModel.state.value
        assertEquals(AuthState.SignedOut, state.auth)
        assertNull(state.stats)
        assertNotNull(state.loginError)
    }
}
