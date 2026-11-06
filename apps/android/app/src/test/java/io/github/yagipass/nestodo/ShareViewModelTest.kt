package io.github.yagipass.nestodo

import io.github.yagipass.nestodo.data.ApiException
import io.github.yagipass.nestodo.domain.Destination
import io.github.yagipass.nestodo.domain.Shared
import io.github.yagipass.nestodo.domain.Todo
import java.io.IOException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ShareViewModelTest {
    private val groceries = Todo(1, null, 0, "Groceries")
    private val milk = Todo(2, 1, 0, "Milk")
    private val eggs = Todo(3, 1, 1, "Eggs")
    private val dentist = Todo(4, null, 1, "Dentist")
    private val repository = FakeRepository(listOf(groceries, milk, eggs, dentist))
    private val backend = FakeBackend(repository)
    private val settings = FakeSettings()
    private val recipe = Shared("Pancakes https://example.com/pancakes", "Serves 4")

    private fun viewModel() = ShareViewModel(backend, settings, recipe)

    private fun ShareViewModel.ready() = state.value as ShareState.Ready

    @Before
    fun setUp() {
        Dispatchers.setMain(UnconfinedTestDispatcher())
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `a tap adds the shared todo at the end of that todo's children with its note in the same request`() {
        val viewModel = viewModel()
        viewModel.add(1)
        assertEquals(
            listOf(
                "GET",
                "POST \"Pancakes https://example.com/pancakes\" under 1 at 2 with note \"Serves 4\"",
            ),
            repository.calls,
        )
        assertEquals(ShareState.Added("Groceries"), viewModel.state.value)
    }

    @Test
    fun `the todo added under last time is offered first so sharing to the usual place takes one tap`() {
        viewModel().add(2)
        assertEquals(Destination(2, "Milk", listOf("Groceries")), viewModel().ready().last)
    }

    @Test
    fun `adding at the top level is remembered too, so the next share starts from the top level`() {
        settings.shareParent = 1
        viewModel().add(null)
        assertNull(settings.shareParent)
        assertNull(viewModel().ready().last)
    }

    @Test
    fun `a remembered todo that is gone or hidden as completed is not offered, so a tap never adds out of sight`() {
        settings.shareParent = 99
        assertNull(viewModel().ready().last)

        repository.todos = listOf(groceries.copy(completedAt = "2026-10-30 09:00:00"), milk, eggs)
        settings.showCompleted = false
        settings.shareParent = 2
        assertNull(viewModel().ready().last)
    }

    @Test
    fun `a failed add keeps the screen and the remembered todo so another tap tries again`() {
        settings.shareParent = 4
        val viewModel = viewModel()
        repository.failure = IOException()

        viewModel.add(1)
        assertEquals("Can't connect. Check your connection.", viewModel.ready().error)
        assertEquals(4L, settings.shareParent)

        viewModel.add(1)
        assertEquals(ShareState.Added("Groceries"), viewModel.state.value)
        assertEquals(2, repository.calls.count { it.startsWith("POST") })
    }

    @Test
    fun `a second tap while the first add is on its way does not add the todo twice`() {
        val viewModel = viewModel()
        repository.gate = CompletableDeferred()
        viewModel.add(1)
        viewModel.add(4)
        repository.gate!!.complete(Unit)
        assertEquals(1, repository.calls.count { it.startsWith("POST") })
        assertEquals(ShareState.Added("Groceries"), viewModel.state.value)
    }

    @Test
    fun `a todo deleted on another device leaves the list once the server refuses it`() {
        val viewModel = viewModel()
        repository.todos = listOf(groceries, milk, eggs)
        repository.failure = ApiException.Invalid()

        viewModel.add(4)

        assertEquals(
            listOf(
                "GET",
                "POST \"Pancakes https://example.com/pancakes\" under 4 at 0 with note \"Serves 4\"",
                "GET",
            ),
            repository.calls,
        )
        assertNull(viewModel.ready().outline[4])
        assertEquals("The todo you picked has been deleted.", viewModel.ready().error)
    }

    @Test
    fun `without a saved sign-in the screen asks to open the app instead of adding`() {
        backend.signedIn = false
        val viewModel = viewModel()
        viewModel.add(null)
        assertEquals(ShareState.SignedOut, viewModel.state.value)
        assertEquals(emptyList<String>(), repository.calls)
    }

    @Test
    fun `a sign-in that expires while adding asks to open the app`() {
        val viewModel = viewModel()
        repository.failure = ApiException.NotSignedIn()
        viewModel.add(1)
        assertEquals(ShareState.SignedOut, viewModel.state.value)
        assertNull(settings.shareParent)
    }

    @Test
    fun `a list that fails to load can be tried again`() {
        repository.failure = IOException()
        val viewModel = viewModel()
        assertEquals(
            ShareState.Failed("Can't connect. Check your connection."),
            viewModel.state.value,
        )

        viewModel.retry()
        assertEquals(listOf(1L, 4L), viewModel.ready().outline.roots)
    }
}
