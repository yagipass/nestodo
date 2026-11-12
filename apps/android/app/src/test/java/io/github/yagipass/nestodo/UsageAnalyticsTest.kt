package io.github.yagipass.nestodo

import io.github.yagipass.nestodo.RecordingCapture.Sent
import io.github.yagipass.nestodo.data.Capture
import io.github.yagipass.nestodo.data.PostHogCapture
import io.github.yagipass.nestodo.data.Screen
import io.github.yagipass.nestodo.data.UsageAnalytics
import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.Todo
import java.io.IOException
import java.util.UUID
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import mockwebserver3.MockResponse
import mockwebserver3.MockWebServer
import okhttp3.OkHttpClient
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class UsageAnalyticsTest {
    private val dispatcher = UnconfinedTestDispatcher()
    private val repository = FakeRepository(listOf(Todo(1, null, 0, "Groceries")))
    private val settings = FakeSettings()
    private val backend = FakeBackend(repository)
    private val capture = RecordingCapture()
    private val agreed = Analytics(available = true, enabled = true, token = "phc_test")

    private fun viewModel(capture: Capture = this.capture) =
        TodoViewModel({ backend }, settings, usage = UsageAnalytics(settings, capture))

    private fun screens() = capture.sent.map { it.screen }

    @Before
    fun setUp() {
        Dispatchers.setMain(dispatcher)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `a member who has not turned on usage data sends nothing, so nothing about them reaches PostHog`() {
        repository.analytics = agreed.copy(enabled = false)
        val viewModel = viewModel()

        viewModel.openStats()
        viewModel.closeStats()
        viewModel.openSettings()

        assertTrue(capture.sent.isEmpty())
        assertNull(settings.analyticsId)
    }

    @Test
    fun `a server without a PostHog project sends nothing even when the account agreed, so the app never picks a project for the server`() {
        repository.analytics = Analytics(available = false, enabled = true, token = null)
        val viewModel = viewModel()

        viewModel.openStats()

        assertTrue(capture.sent.isEmpty())
    }

    @Test
    fun `a failed check of the agreement sends nothing, because only a yes from the server allows sending`() {
        repository.analytics = agreed
        repository.analyticsFailure = IOException("offline")
        val viewModel = viewModel()

        viewModel.openStats()

        assertTrue(capture.sent.isEmpty())
    }

    @Test
    fun `the screen the app opens on is counted once the account's yes arrives, to the project the server names`() {
        repository.analytics = agreed
        repository.analyticsGate = CompletableDeferred()

        viewModel()
        assertTrue(capture.sent.isEmpty())

        repository.analyticsGate!!.complete(Unit)
        assertEquals(listOf(Sent("phc_test", settings.analyticsId!!, Screen.Todos)), capture.sent)
    }

    @Test
    fun `each screen change is counted under its name and one random install ID, so PostHog shows which screens are used without knowing who used them`() {
        repository.analytics = agreed
        val viewModel = viewModel()

        viewModel.openStats()
        viewModel.closeStats()
        viewModel.openSettings()
        viewModel.closeSettings()

        assertEquals(
            listOf(Screen.Todos, Screen.Stats, Screen.Todos, Screen.Settings, Screen.Todos),
            screens(),
        )
        val ids = capture.sent.map { it.distinctId }.toSet()
        assertEquals(1, ids.size)
        UUID.fromString(ids.single())
    }

    @Test
    fun `turning usage data on starts counting from the next screen, like the web app, so the switch itself is not reported`() {
        repository.analytics = agreed.copy(enabled = false)
        val viewModel = viewModel()
        viewModel.openSettings()

        viewModel.setAnalytics(true)
        assertTrue(capture.sent.isEmpty())

        viewModel.closeSettings()
        assertEquals(listOf(Screen.Todos), screens())
    }

    @Test
    fun `turning usage data off stops sending and forgets the install ID, so screens after turning it on again cannot be tied to earlier ones`() {
        repository.analytics = agreed
        val viewModel = viewModel()
        val before = settings.analyticsId
        assertNotNull(before)
        viewModel.openSettings()

        viewModel.setAnalytics(false)
        assertNull(settings.analyticsId)
        viewModel.closeSettings()
        assertEquals(listOf(Screen.Todos, Screen.Settings), screens())

        viewModel.openSettings()
        viewModel.setAnalytics(true)
        viewModel.closeSettings()
        assertEquals(Screen.Todos, capture.sent.last().screen)
        assertNotEquals(before, capture.sent.last().distinctId)
    }

    @Test
    fun `turning usage data off on another device forgets the install ID at the next launch`() {
        settings.analyticsId = "earlier-id"
        repository.analytics = agreed.copy(enabled = false)

        viewModel()

        assertNull(settings.analyticsId)
        assertTrue(capture.sent.isEmpty())
    }

    @Test
    fun `signing in to another server waits for that account's answer, so one account's yes never covers another`() {
        repository.analytics = agreed
        val viewModel = viewModel()
        assertEquals(1, capture.sent.size)
        viewModel.changeServer()
        repository.analytics = agreed.copy(enabled = false)
        repository.analyticsGate = CompletableDeferred()

        viewModel.setServer("other.example.com")
        assertEquals(AuthState.SignedIn, viewModel.state.value.auth)
        assertEquals(1, capture.sent.size)

        repository.analyticsGate!!.complete(Unit)
        assertEquals(1, capture.sent.size)
    }

    @Test
    fun `PostHog being down never shows an error, because usage data is not something the member has to deal with`() {
        repository.analytics = agreed
        val viewModel = viewModel { _, _, _ -> throw IOException("PostHog is down") }

        viewModel.openStats()

        val state = viewModel.state.value
        assertEquals(AuthState.SignedIn, state.auth)
        assertNull(state.notice)
        assertFalse(state.stats is StatsState.Failed)
    }

    @Test
    fun `what reaches PostHog holds no server address, todo or email, so the member's data stays on their server`() {
        val posthog = MockWebServer()
        posthog.start()
        try {
            posthog.enqueue(MockResponse.Builder().code(200).build())
            repository.analytics = agreed

            viewModel(PostHogCapture(OkHttpClient(), posthog.url("/i/v0/e/"), "0.0.0"))

            val body = posthog.takeRequest(5, TimeUnit.SECONDS)!!.body!!.utf8()
            assertFalse(body.contains("nestodo.example.com"))
            assertFalse(body.contains("Groceries"))
            assertFalse(body.contains("family@example.com"))
            assertTrue(body.contains("\"\$process_person_profile\":false"))
        } finally {
            posthog.close()
        }
    }
}
