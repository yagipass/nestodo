package io.github.yagipass.nestodo

import android.app.UiModeManager
import android.content.ActivityNotFoundException
import android.content.Context
import android.security.NetworkSecurityPolicy
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import io.github.yagipass.nestodo.access.DeviceLockedException
import io.github.yagipass.nestodo.access.KeystoreTokenStore
import io.github.yagipass.nestodo.data.AccessBackend
import io.github.yagipass.nestodo.data.ApiException
import io.github.yagipass.nestodo.data.Backend
import io.github.yagipass.nestodo.data.Connection
import io.github.yagipass.nestodo.data.PostHogCapture
import io.github.yagipass.nestodo.data.PreferenceSettings
import io.github.yagipass.nestodo.data.Screen
import io.github.yagipass.nestodo.data.Settings
import io.github.yagipass.nestodo.data.TodoRepository
import io.github.yagipass.nestodo.data.UsageAnalytics
import io.github.yagipass.nestodo.data.baseUrlOf
import io.github.yagipass.nestodo.data.label
import io.github.yagipass.nestodo.domain.Analytics
import io.github.yagipass.nestodo.domain.Appearance
import io.github.yagipass.nestodo.domain.Avatar
import io.github.yagipass.nestodo.domain.Caret
import io.github.yagipass.nestodo.domain.Change
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Edit
import io.github.yagipass.nestodo.domain.MAX_NOTE_LENGTH
import io.github.yagipass.nestodo.domain.MAX_TITLE_LENGTH
import io.github.yagipass.nestodo.domain.Outline
import io.github.yagipass.nestodo.domain.OutlineRow
import io.github.yagipass.nestodo.domain.Profile
import io.github.yagipass.nestodo.domain.Todo
import io.github.yagipass.nestodo.domain.Viewport
import io.github.yagipass.nestodo.domain.WebhookEdit
import io.github.yagipass.nestodo.domain.WebhookForm
import io.github.yagipass.nestodo.domain.Webhooks
import io.github.yagipass.nestodo.domain.append
import io.github.yagipass.nestodo.domain.delete
import io.github.yagipass.nestodo.domain.deleteBackward
import io.github.yagipass.nestodo.domain.enter
import io.github.yagipass.nestodo.domain.indent
import io.github.yagipass.nestodo.domain.moveDown
import io.github.yagipass.nestodo.domain.moveTo
import io.github.yagipass.nestodo.domain.moveUp
import io.github.yagipass.nestodo.domain.outdent
import io.github.yagipass.nestodo.domain.outlineOf
import io.github.yagipass.nestodo.domain.singleLine
import io.github.yagipass.nestodo.domain.toggleCollapsed
import io.github.yagipass.nestodo.domain.toggleCompleted
import java.io.IOException
import java.time.ZoneId
import java.util.concurrent.TimeUnit
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlin.time.TimeMark
import kotlin.time.TimeSource
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

enum class AuthState {
    NoServer,
    CheckingServer,
    Checking,
    Unavailable,
    SignedOut,
    WaitingForBrowser,
    SignedIn,
}

data class Notice(val id: Long, val text: String, val retry: (() -> Unit)? = null)

sealed interface StatsState {
    data object Loading : StatsState

    data class Loaded(val days: List<Day>) : StatsState

    data class Failed(val message: String) : StatsState
}

sealed interface SettingsState {
    data object Loading : SettingsState

    data class Loaded(
        val webhooks: Webhooks,
        val analytics: Analytics,
        val form: WebhookForm? = null,
        val errors: Map<String, String> = emptyMap(),
        val saving: Boolean = false,
        val analyticsSaving: Boolean = false,
        val analyticsFailed: Boolean = false,
        val name: String = "",
        val profileProblem: String? = null,
        val profileErrors: Map<String, String> = emptyMap(),
        val profileSaving: Boolean = false,
        val imageUnreadable: Boolean = false,
    ) : SettingsState {
        fun showing(webhooks: Webhooks): Loaded =
            copy(
                webhooks = webhooks,
                form =
                    WebhookForm.of(null).takeIf { webhooks.available && webhooks.webhook == null },
                errors = emptyMap(),
                saving = false,
            )
    }

    data class Failed(val message: String) : SettingsState
}

data class TodoUiState(
    val auth: AuthState = AuthState.Checking,
    val loginError: String? = null,
    val server: String? = null,
    val changingServer: Boolean = false,
    val email: String? = null,
    val profile: Profile? = null,
    val avatar: Avatar? = null,
    val outline: Outline = Outline(),
    val loaded: Boolean = false,
    val showCompleted: Boolean = true,
    val appearance: Appearance = Appearance.System,
    val editing: Long? = null,
    val inNote: Boolean = false,
    val noteOpen: Long? = null,
    val caret: Caret? = null,
    val moving: Caret? = null,
    val deleting: Caret? = null,
    val completing: Caret? = null,
    val revision: Long = 0,
    val canUndo: Boolean = false,
    val notice: Notice? = null,
    val stats: StatsState? = null,
    val settings: SettingsState? = null,
) {
    val viewport: Viewport
        get() = Viewport(showCompleted)

    val rows: List<OutlineRow>
        get() = outline.rows(viewport)

    val screen: Screen?
        get() =
            when {
                auth != AuthState.SignedIn -> null
                stats != null -> Screen.Stats
                settings != null -> Screen.Settings
                else -> Screen.Todos
            }
}

class TodoViewModel(
    private val backendAt: (HttpUrl) -> Backend,
    private val settings: Settings,
    private val cleartextAllowed: (String) -> Boolean = { false },
    private val timeZone: () -> String = { ZoneId.systemDefault().id },
    private val timeSource: TimeSource = TimeSource.Monotonic,
    private val usage: UsageAnalytics? = null,
    private val applyAppearance: (Appearance) -> Unit = {},
) : ViewModel() {
    private sealed interface Task {
        data class Send(val change: Change) : Task

        data class Load(val force: Boolean) : Task

        data class Stats(val request: Long) : Task

        data class Settings(val request: Long, val call: SettingsCall) : Task

        data object LoadProfile : Task
    }

    private sealed interface SettingsCall {
        data object Load : SettingsCall

        data class Save(val edit: WebhookEdit) : SettingsCall

        data object Remove : SettingsCall

        data class SetAnalytics(val enabled: Boolean) : SettingsCall

        data class SaveName(val name: String) : SettingsCall

        class SaveAvatar(val image: ByteArray) : SettingsCall

        data object RemoveAvatar : SettingsCall
    }

    private data class Queued(val task: Task, val generation: Int)

    private data class Typing(
        val key: Long,
        val inNote: Boolean,
        val original: String,
        val at: TimeMark,
        val composing: Boolean,
    )

    private data class Undo(
        val changes: List<Change>,
        val caret: Caret?,
        val typing: Typing? = null,
    )

    private val _state = MutableStateFlow(TodoUiState())
    val state: StateFlow<TodoUiState> = _state.asStateFlow()

    private val current: TodoUiState
        get() = _state.value

    private val loginCompleted = Channel<Unit>(Channel.CONFLATED)
    val loginEvents: Flow<Unit> = loginCompleted.receiveAsFlow()

    private var backend: Backend? = null
    private var loginJob: Job? = null
    private var syncJob: Job? = null
    private var analyticsJob: Job? = null
    private var typingJob: Job? = null
    private var tasks: Channel<Queued>? = null
    private var generation = 0
    private var pending = 0
    private var lastKey = 0L
    private var cursor = 0
    private val serverIds = mutableMapOf<Long, Long>()
    private val unsavedTitles = linkedSetOf<Long>()
    private val unsavedNotes = linkedSetOf<Long>()
    private val history = ArrayDeque<Undo>()
    private var noticeId = 0L
    private var statsRequest = 0L
    private var settingsRequest = 0L

    init {
        usage?.let { usage ->
            viewModelScope.launch {
                state
                    .map { it.screen }
                    .distinctUntilChanged()
                    .collect { screen -> launch { usage.show(screen) } }
            }
        }
        connect()
    }

    fun connect() {
        _state.update { it.copy(auth = AuthState.Checking, loginError = null) }
        viewModelScope.launch {
            val shown = settings.showCompleted()
            val appearance = settings.appearance()
            _state.update { it.copy(showCompleted = shown, appearance = appearance) }
            val backend = backend ?: settings.baseUrl()?.let(::open)
            if (backend == null) {
                _state.update { it.copy(auth = AuthState.NoServer) }
                return@launch
            }
            try {
                when (val connection = backend.connect()) {
                    is Connection.Ready -> onReady(connection)
                    Connection.SignedOut -> _state.update { it.copy(auth = AuthState.SignedOut) }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(
                        auth = AuthState.Unavailable,
                        loginError =
                            if (e is IOException) {
                                "Can't reach the server. Check your connection and try again."
                            } else {
                                "Couldn't verify the server. Try again later."
                            },
                    )
                }
            }
        }
    }

    fun setServer(address: String) {
        val base = baseUrlOf(address)
        if (base == null || !base.isHttps && !cleartextAllowed(base.host)) {
            _state.update {
                it.copy(
                    loginError =
                        if (base == null) "Enter an address such as nestodo.example.com."
                        else "Enter an address that starts with https://."
                )
            }
            return
        }
        loginJob?.cancel()
        _state.update { it.copy(auth = AuthState.CheckingServer, loginError = null) }
        loginJob = viewModelScope.launch {
            val candidate = backendAt(base)
            try {
                val connection = candidate.connect()
                val todos = (connection as? Connection.Ready)?.todos?.list()
                val saved = settings.baseUrl()
                try {
                    settings.setBaseUrl(base)
                } catch (_: IOException) {
                    rejectServer("Couldn't save the server. Try again.")
                    return@launch
                }
                backend.takeIf { saved != null && saved.host != base.host }?.let { logout(it) }
                backend = candidate
                _state.update {
                    it.copy(server = base.label(), changingServer = false, settings = null)
                }
                when (connection) {
                    is Connection.Ready -> onReady(connection, todos)
                    Connection.SignedOut -> _state.update { it.copy(auth = AuthState.SignedOut) }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                rejectServer(
                    when (e) {
                        is DeviceLockedException -> reason(e)
                        is IOException ->
                            "Can't reach this server. Check the address and your connection."
                        else -> "Couldn't find Nestodo at this address."
                    }
                )
            }
        }
    }

    private fun rejectServer(error: String) {
        _state.update { it.copy(auth = AuthState.NoServer, loginError = error) }
    }

    fun changeServer() {
        loginJob?.cancel()
        disconnect()
        _state.value =
            TodoUiState(
                auth = AuthState.NoServer,
                server = current.server,
                showCompleted = current.showCompleted,
                changingServer = true,
                settings = current.settings?.let { SettingsState.Loading },
            )
    }

    fun cancelChangeServer() {
        if (!current.changingServer || current.auth == AuthState.CheckingServer) return
        _state.update { it.copy(changingServer = false) }
        connect()
    }

    fun signIn(open: (String) -> Unit) {
        val backend = backend ?: return
        loginJob?.cancel()
        loginJob = viewModelScope.launch {
            try {
                val flow = backend.startLogin()
                _state.update { it.copy(auth = AuthState.WaitingForBrowser, loginError = null) }
                open(flow.url)
                onReady(flow.await())
                loginCompleted.trySend(Unit)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                _state.update {
                    it.copy(
                        auth = AuthState.SignedOut,
                        loginError = "Couldn't sign in. Try again.",
                    )
                }
            }
        }
    }

    fun cancelSignIn() {
        loginJob?.cancel()
        _state.update { it.copy(auth = AuthState.SignedOut) }
    }

    fun signOut() {
        loginJob?.cancel()
        disconnect()
        _state.value =
            TodoUiState(
                auth = AuthState.SignedOut,
                server = current.server,
                showCompleted = current.showCompleted,
            )
        val backend = backend ?: return
        viewModelScope.launch { logout(backend) }
    }

    private suspend fun logout(backend: Backend) {
        try {
            backend.logout()
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {}
    }

    private fun open(base: HttpUrl): Backend {
        _state.update { it.copy(server = base.label()) }
        return backendAt(base).also { backend = it }
    }

    fun refresh() {
        enqueue(Task.Load(force = true))
    }

    fun onForeground() {
        val state = current
        if (state.auth == AuthState.SignedIn && isIdle(queued = 0)) {
            enqueue(Task.Load(force = !state.loaded))
        }
    }

    fun noticeShown(id: Long) {
        _state.update { if (it.notice?.id == id) it.copy(notice = null) else it }
    }

    fun openLink(url: String, open: (String) -> Unit) {
        try {
            open(url)
        } catch (_: ActivityNotFoundException) {
            notify("No app can open this link.")
        }
    }

    fun setTitle(key: Long, title: String, composing: Boolean = false) {
        val text = singleLine(title).take(MAX_TITLE_LENGTH)
        val node = current.outline[key] ?: return
        type(key, inNote = false, node.title, text, composing)
        if (node.title == text) return
        _state.update {
            it.copy(outline = it.outline.apply(Change.Rename(key, text)), canUndo = true)
        }
        unsavedTitles += key
        saveTypingLater()
    }

    fun setNote(key: Long, note: String, composing: Boolean = false) {
        val text = note.take(MAX_NOTE_LENGTH)
        val node = current.outline[key] ?: return
        type(key, inNote = true, node.note, text, composing)
        if (node.note == text) return
        _state.update {
            it.copy(outline = it.outline.apply(Change.Annotate(key, text)), canUndo = true)
        }
        unsavedNotes += key
        saveTypingLater()
    }

    fun select(key: Long, cursor: Int) {
        this.cursor = cursor
        if (current.editing != key || current.inNote) {
            _state.update { it.copy(editing = key, inNote = false) }
        }
    }

    fun selectNote(key: Long, cursor: Int) {
        this.cursor = cursor
        if (current.editing != key || !current.inNote) {
            _state.update { it.copy(editing = key, inNote = true, noteOpen = key) }
        }
    }

    fun blur(key: Long) {
        if (current.editing == key && !current.inNote) _state.update { it.copy(editing = null) }
        endTyping()
        saveTyping()
    }

    fun blurNote(key: Long) {
        val cleared = current.outline[key]?.note?.let { it.isNotEmpty() && it.isBlank() } == true
        if (cleared) setNote(key, "")
        endTyping()
        _state.update {
            val leaving = it.editing == key && it.inNote
            it.copy(
                editing = if (leaving) null else it.editing,
                inNote = it.inNote && !leaving,
                noteOpen = it.noteOpen.takeUnless { open -> open == key },
                revision = if (cleared) it.revision + 1 else it.revision,
            )
        }
        saveTyping()
    }

    fun toggleNote() {
        val state = current
        val key = state.editing ?: return
        val node = state.outline[key] ?: return
        _state.update {
            if (state.inNote) it.copy(caret = Caret(key, node.title.length))
            else it.copy(noteOpen = key, caret = Caret(key, node.note.length, inNote = true))
        }
    }

    fun caretShown(caret: Caret) {
        _state.update { if (it.caret == caret) it.copy(caret = null) else it }
    }

    fun enter(key: Long, cursor: Int) {
        val state = current
        perform(state.outline.enter(state.viewport, key, cursor, newKey()), Caret(key, cursor))
    }

    fun deleteBackward(key: Long): Boolean {
        val state = current
        val edit = state.outline.deleteBackward(state.viewport, key) ?: return false
        perform(edit, Caret(key, 0))
        return true
    }

    fun indent() = act { state, key -> state.outline.indent(state.viewport, key, cursor) }

    fun outdent() = act { state, key -> state.outline.outdent(key, cursor) }

    fun moveUp() = act { state, key -> state.outline.moveUp(state.viewport, key, cursor) }

    fun moveDown() = act { state, key -> state.outline.moveDown(state.viewport, key, cursor) }

    fun startMove() {
        val key = current.editing ?: return
        _state.update { it.copy(moving = Caret(key, cursor, it.inNote)) }
    }

    fun moveTo(target: Long) {
        val state = current
        val moving = state.moving ?: return
        val edit = state.outline.moveTo(state.viewport, moving.key, target) ?: return cancelMove()
        _state.update { it.copy(moving = null) }
        perform(edit, moving)
    }

    fun cancelMove() {
        val moving = current.moving ?: return
        _state.update { it.copy(moving = null, caret = moving) }
    }

    fun startDelete() {
        val key = current.editing ?: return
        _state.update { it.copy(deleting = Caret(key, cursor, it.inNote)) }
    }

    fun confirmDelete() {
        val state = current
        val deleting = state.deleting ?: return
        val edit = state.outline.delete(state.viewport, deleting.key) ?: return cancelDelete()
        _state.update { it.copy(deleting = null) }
        perform(edit, deleting)
    }

    fun cancelDelete() {
        val deleting = current.deleting ?: return
        _state.update { it.copy(deleting = null, caret = deleting) }
    }

    fun toggleCompleted() {
        val state = current
        val key = state.editing ?: return
        if (state.outline[key]?.completed == false && state.outline.openUnder(key) > 0) {
            _state.update { it.copy(completing = Caret(key, cursor, it.inNote)) }
            return
        }
        act(
            Caret(key, cursor, state.inNote),
            state.outline.toggleCompleted(state.viewport, key, cursor),
        )
    }

    fun confirmComplete() {
        val state = current
        val completing = state.completing ?: return
        if (state.outline[completing.key]?.completed != false) return cancelComplete()
        _state.update { it.copy(completing = null) }
        act(
            completing,
            state.outline.toggleCompleted(state.viewport, completing.key, completing.position),
        )
    }

    fun cancelComplete() {
        val completing = current.completing ?: return
        _state.update { it.copy(completing = null, caret = completing) }
    }

    fun toggleCollapsed() = act { state, key -> state.outline.toggleCollapsed(key, cursor) }

    fun toggleCollapsed(key: Long) {
        val state = current
        val edit = state.outline.toggleCollapsed(key, cursor) ?: return
        perform(edit.copy(caret = null), state.editing?.let { Caret(it, cursor, state.inNote) })
    }

    fun add() {
        perform(current.outline.append(null, newKey()), before = null)
    }

    fun toggleShowCompleted() {
        val shown = !current.showCompleted
        viewModelScope.launch { settings.setShowCompleted(shown) }
        _state.update { it.copy(showCompleted = shown) }
    }

    fun setAppearance(appearance: Appearance) {
        _state.update { it.copy(appearance = appearance) }
        viewModelScope.launch { settings.setAppearance(appearance) }
        applyAppearance(appearance)
    }

    fun openStats() {
        _state.update { it.copy(stats = StatsState.Loading) }
        statsRequest += 1
        enqueue(Task.Stats(statsRequest))
    }

    fun closeStats() {
        _state.update { it.copy(stats = null) }
    }

    fun openSettings() {
        _state.update { it.copy(settings = SettingsState.Loading) }
        settingsRequest += 1
        enqueue(Task.Settings(settingsRequest, SettingsCall.Load))
    }

    fun closeSettings() {
        _state.update { it.copy(settings = null) }
    }

    fun editWebhook() = updateSettings { settings ->
        settings.copy(form = WebhookForm.of(settings.webhooks.webhook), errors = emptyMap())
    }

    fun cancelWebhook() = updateSettings { settings ->
        if (settings.webhooks.webhook == null || settings.saving) settings
        else settings.copy(form = null, errors = emptyMap())
    }

    fun changeWebhook(form: WebhookForm) = updateSettings { settings ->
        if (settings.form == null) settings else settings.copy(form = form)
    }

    fun saveWebhook() {
        val form = (current.settings as? SettingsState.Loaded)?.form ?: return
        callSettings(SettingsCall.Save(form.edit))
    }

    fun removeWebhook() {
        callSettings(SettingsCall.Remove)
    }

    fun setAnalytics(enabled: Boolean) {
        val settings = current.settings as? SettingsState.Loaded ?: return
        if (settings.analyticsSaving) return
        updateSettings { it.copy(analyticsSaving = true, analyticsFailed = false) }
        enqueue(Task.Settings(settingsRequest, SettingsCall.SetAnalytics(enabled)))
    }

    fun changeName(name: String) = updateSettings { it.copy(name = name) }

    fun saveName() {
        val settings = current.settings as? SettingsState.Loaded ?: return
        callProfile(SettingsCall.SaveName(settings.name))
    }

    fun uploadAvatar(image: ByteArray) {
        callProfile(SettingsCall.SaveAvatar(image))
    }

    fun avatarUnreadable() = updateSettings {
        if (it.profileSaving) it else it.copy(imageUnreadable = true, profileErrors = emptyMap())
    }

    fun removeAvatar() {
        callProfile(SettingsCall.RemoveAvatar)
    }

    private fun callProfile(call: SettingsCall) {
        val settings = current.settings as? SettingsState.Loaded ?: return
        if (settings.profileSaving) return
        updateSettings {
            it.copy(profileSaving = true, profileErrors = emptyMap(), imageUnreadable = false)
        }
        enqueue(Task.Settings(settingsRequest, call))
    }

    private fun callSettings(call: SettingsCall) {
        val settings = current.settings as? SettingsState.Loaded ?: return
        if (settings.saving) return
        updateSettings { it.copy(saving = true, errors = emptyMap()) }
        enqueue(Task.Settings(settingsRequest, call))
    }

    private fun updateSettings(change: (SettingsState.Loaded) -> SettingsState.Loaded) {
        _state.update { state ->
            val settings = state.settings as? SettingsState.Loaded ?: return@update state
            state.copy(settings = change(settings))
        }
    }

    fun undo() {
        val undo = history.removeLastOrNull() ?: return
        saveTyping(undo.changes)
        if (current.outline.accepts(undo.changes)) {
            commit(undo.changes, undo.caret)
        } else {
            _state.update { it.copy(canUndo = history.isNotEmpty()) }
        }
    }

    private fun act(edit: (TodoUiState, Long) -> Edit?) {
        val state = current
        val key = state.editing ?: return
        act(Caret(key, cursor, state.inNote), edit(state, key))
    }

    private fun act(before: Caret, edit: Edit?) {
        val done = edit?.let { done ->
            done.copy(
                caret =
                    done.caret?.let {
                        if (it.key == before.key) it.copy(inNote = before.inNote) else it
                    }
            )
        }
        perform(done, before)
    }

    private fun perform(edit: Edit?, before: Caret?) {
        edit ?: return
        saveTyping(edit.changes)
        val undo = current.outline.undoFor(edit.changes)
        if (undo == null) history.clear() else push(Undo(undo, before))
        commit(edit.changes, edit.caret)
    }

    private fun type(
        key: Long,
        inNote: Boolean,
        before: String,
        after: String,
        composing: Boolean,
    ) {
        val last = history.lastOrNull()
        val typing = last?.typing?.takeIf { it.key == key && it.inNote == inNote }
        if (before == after) {
            if (typing != null && typing.composing && !composing) {
                val settled = typing.copy(at = timeSource.markNow(), composing = false)
                history[history.lastIndex] = history.last().copy(typing = settled)
            }
            return
        }
        val open = typing?.takeIf { it.composing || it.at.elapsedNow() <= TYPING_PAUSE }
        val original = open?.original ?: before
        val undo =
            Undo(
                listOf(
                    if (inNote) Change.Annotate(key, original) else Change.Rename(key, original)
                ),
                Caret(key, editEnd(original, after), inNote),
                Typing(key, inNote, original, timeSource.markNow(), composing),
            )
        if (open != null) history[history.lastIndex] = undo else push(undo)
    }

    private fun endTyping() {
        val last = history.lastOrNull() ?: return
        if (last.typing != null) history[history.lastIndex] = last.copy(typing = null)
    }

    private fun push(undo: Undo) {
        endTyping()
        history.addLast(undo)
        while (history.size > HISTORY_LIMIT) history.removeFirst()
    }

    private fun commit(changes: List<Change>, caret: Caret?) {
        _state.update { state ->
            val outline = changes.fold(state.outline) { outline, change -> outline.apply(change) }
            val rows = outline.rows(state.viewport)
            val target = caret?.takeIf { c -> rows.any { it.key == c.key } }
            state.copy(
                outline = outline,
                caret = target,
                editing = target?.key ?: state.editing?.takeIf { e -> rows.any { it.key == e } },
                noteOpen = target?.takeIf { it.inNote }?.key ?: state.noteOpen,
                revision = state.revision + 1,
                canUndo = history.isNotEmpty(),
            )
        }
        current.caret?.let { cursor = it.position }
        changes.forEach { enqueue(Task.Send(it)) }
    }

    private fun saveTypingLater() {
        typingJob?.cancel()
        typingJob = viewModelScope.launch {
            delay(TYPING_DELAY)
            saveTyping()
        }
    }

    private fun saveTyping(changes: List<Change> = emptyList()) {
        typingJob?.cancel()
        typingJob = null
        val outline = current.outline
        val titles = unsavedTitles.toList()
        val notes = unsavedNotes.toList()
        unsavedTitles.clear()
        unsavedNotes.clear()
        for (key in titles) {
            if (changes.any { it.key == key && (it is Change.Rename || it is Change.Delete) })
                continue
            val node = outline[key] ?: continue
            enqueue(Task.Send(Change.Rename(key, node.title)))
        }
        for (key in notes) {
            if (changes.any { it.key == key && (it is Change.Annotate || it is Change.Delete) })
                continue
            val node = outline[key] ?: continue
            enqueue(Task.Send(Change.Annotate(key, node.note)))
        }
    }

    private fun newKey(): Long {
        lastKey -= 1
        return lastKey
    }

    private fun isIdle(queued: Int): Boolean =
        pending == queued && unsavedTitles.isEmpty() && unsavedNotes.isEmpty()

    private fun enqueue(task: Task) {
        val channel = tasks ?: return
        pending += 1
        channel.trySend(Queued(task, generation))
    }

    private fun onReady(ready: Connection.Ready, todos: List<Todo>? = null) {
        disconnect()
        _state.update {
            it.copy(auth = AuthState.SignedIn, email = ready.email, loginError = null)
        }
        startSync(ready.todos)
        if (todos == null) enqueue(Task.Load(force = true)) else show(todos)
        enqueue(Task.LoadProfile)
        if (current.settings != null) openSettings()
        startAnalytics(ready.todos)
    }

    private fun startAnalytics(repository: TodoRepository) {
        val usage = usage ?: return
        analyticsJob = viewModelScope.launch {
            val analytics =
                try {
                    repository.analytics()
                } catch (e: CancellationException) {
                    throw e
                } catch (_: Exception) {
                    return@launch
                }
            usage.start(analytics)
        }
    }

    private fun startSync(repository: TodoRepository) {
        val channel = Channel<Queued>(Channel.UNLIMITED)
        tasks = channel
        pending = 0
        syncJob = viewModelScope.launch {
            for (queued in channel) {
                try {
                    if (
                        queued.generation == generation ||
                            queued.task is Task.Stats ||
                            queued.task is Task.Settings ||
                            queued.task is Task.LoadProfile
                    ) {
                        execute(repository, queued.task)
                    }
                } catch (e: CancellationException) {
                    throw e
                } catch (e: Exception) {
                    recover(e, queued.task)
                } finally {
                    pending -= 1
                }
            }
        }
    }

    private fun disconnect() {
        analyticsJob?.cancel()
        analyticsJob = null
        usage?.stop()
        syncJob?.cancel()
        syncJob = null
        tasks?.close()
        tasks = null
        discardPending()
        history.clear()
        serverIds.clear()
    }

    private fun discardPending() {
        generation += 1
        typingJob?.cancel()
        typingJob = null
        unsavedTitles.clear()
        unsavedNotes.clear()
    }

    private suspend fun execute(repository: TodoRepository, task: Task) {
        when (task) {
            is Task.Load -> {
                val todos = repository.list()
                if (task.force || isIdle(queued = 1)) show(todos)
            }
            is Task.Send -> send(repository, task.change)
            is Task.Stats ->
                showStats(task.request, StatsState.Loaded(repository.stats(timeZone())))
            is Task.Settings -> settle(repository, task)
            Task.LoadProfile -> showProfile(repository, repository.profile())
        }
    }

    private suspend fun settle(repository: TodoRepository, task: Task.Settings) {
        when (val call = task.call) {
            SettingsCall.Load -> {
                val webhooks = repository.webhooks()
                val analytics = repository.analytics()
                usage?.consent(analytics)
                val loaded =
                    try {
                        val profile = repository.profile()
                        showProfile(repository, profile)
                        SettingsState.Loaded(webhooks, analytics, name = profile.name.orEmpty())
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: ApiException.NotSignedIn) {
                        throw e
                    } catch (e: Exception) {
                        SettingsState.Loaded(webhooks, analytics, profileProblem = reason(e))
                    }
                showSettings(task.request) { loaded.showing(loaded.webhooks) }
            }
            is SettingsCall.SaveName -> {
                val profile = repository.saveName(call.name)
                _state.update { it.copy(profile = profile) }
                showSettings(task.request) { settings ->
                    settings.loaded()?.let {
                        it.copy(
                            name = if (it.name == call.name) profile.name.orEmpty() else it.name,
                            profileSaving = false,
                        )
                    }
                }
            }
            is SettingsCall.SaveAvatar -> {
                val profile = repository.saveAvatar(call.image)
                _state.update {
                    it.copy(
                        profile = profile,
                        avatar = profile.avatarUrl?.let { url -> Avatar(url, call.image) },
                    )
                }
                showSettings(task.request) { it.loaded()?.copy(profileSaving = false) }
            }
            SettingsCall.RemoveAvatar -> {
                repository.removeAvatar()
                _state.update {
                    it.copy(profile = it.profile?.copy(avatarUrl = null), avatar = null)
                }
                showSettings(task.request) { it.loaded()?.copy(profileSaving = false) }
            }
            is SettingsCall.Save -> {
                val webhooks = repository.saveWebhook(call.edit)
                showSettings(task.request) { it.loaded()?.showing(webhooks) }
            }
            SettingsCall.Remove -> {
                repository.removeWebhook()
                showSettings(task.request) { settings ->
                    settings.loaded()?.let { it.showing(it.webhooks.copy(webhook = null)) }
                }
            }
            is SettingsCall.SetAnalytics -> {
                val enabled = repository.setAnalytics(call.enabled)
                usage?.consent(enabled)
                showSettings(task.request) { settings ->
                    settings.loaded()?.let {
                        it.copy(
                            analytics = it.analytics.copy(enabled = enabled),
                            analyticsSaving = false,
                        )
                    }
                }
            }
        }
    }

    private fun SettingsState.loaded(): SettingsState.Loaded? = this as? SettingsState.Loaded

    private suspend fun showProfile(repository: TodoRepository, profile: Profile) {
        val avatar = avatarOf(repository, profile)
        _state.update { it.copy(profile = profile, avatar = avatar) }
    }

    private suspend fun avatarOf(repository: TodoRepository, profile: Profile): Avatar? {
        val url = profile.avatarUrl ?: return null
        current.avatar
            ?.takeIf { it.url == url }
            ?.let {
                return it
            }
        return try {
            Avatar(url, repository.avatar(url))
        } catch (e: CancellationException) {
            throw e
        } catch (e: ApiException.NotSignedIn) {
            throw e
        } catch (_: Exception) {
            null
        }
    }

    private suspend fun send(repository: TodoRepository, change: Change) {
        fun id(key: Long): Long = serverIds[key] ?: key
        when (change) {
            is Change.Create ->
                serverIds[change.key] =
                    repository
                        .create(change.title, change.parent?.let(::id), change.index, change.note)
                        .id
            is Change.Rename -> repository.rename(id(change.key), change.title)
            is Change.Annotate -> repository.setNote(id(change.key), change.note)
            is Change.Complete -> repository.setCompleted(id(change.key), change.completed)
            is Change.Collapse -> repository.setCollapsed(id(change.key), change.collapsed)
            is Change.Move ->
                repository.move(id(change.key), change.parent?.let(::id), change.index)
            is Change.Delete ->
                try {
                    repository.delete(id(change.key), change.recursive)
                } catch (_: ApiException.NotFound) {}
        }
    }

    private fun show(todos: List<Todo>) {
        discardPending()
        val keys = serverIds.entries.associate { (key, id) -> id to key }
        serverIds.clear()
        todos.forEach { todo -> keys[todo.id]?.let { serverIds[it] = todo.id } }
        val outline = outlineOf(todos) { keys[it] ?: it }
        if (outline != current.outline) history.clear()
        current.editing?.let { key ->
            fun edited(of: Outline) = of[key]?.let { if (current.inNote) it.note else it.title }
            val text = edited(outline)
            if (text != null && text != edited(current.outline)) cursor = text.length
        }
        _state.update { state ->
            state.copy(
                outline = outline,
                loaded = true,
                editing = state.editing?.takeIf { it in outline },
                inNote = state.inNote && state.editing?.let { it in outline } == true,
                noteOpen = state.noteOpen?.takeIf { it in outline },
                caret = null,
                revision = state.revision + 1,
                canUndo = history.isNotEmpty(),
                notice = state.notice?.takeIf { it.retry == null },
            )
        }
    }

    private fun showStats(request: Long, stats: StatsState) {
        if (request != statsRequest) return
        _state.update { if (it.stats == null) it else it.copy(stats = stats) }
    }

    private fun showSettings(request: Long, change: (SettingsState) -> SettingsState?) {
        if (request != settingsRequest) return
        _state.update { state ->
            val settings = state.settings ?: return@update state
            state.copy(settings = change(settings) ?: settings)
        }
    }

    private fun recover(error: Exception, task: Task) {
        if (task is Task.Load) {
            fail(error, retry = ::refresh)
            return
        }
        if (task is Task.Stats) {
            if (error is ApiException.NotSignedIn) {
                fail(error, retry = null)
            } else {
                showStats(task.request, StatsState.Failed(reason(error)))
            }
            return
        }
        if (task is Task.Settings) {
            recoverSettings(error, task)
            return
        }
        if (task is Task.LoadProfile) {
            if (error is ApiException.NotSignedIn) fail(error, retry = null)
            return
        }
        discardPending()
        history.clear()
        _state.update { it.copy(canUndo = false) }
        fail(error, retry = null)
        enqueue(Task.Load(force = true))
    }

    private fun recoverSettings(error: Exception, task: Task.Settings) {
        if (error is ApiException.NotSignedIn) {
            fail(error, retry = null)
            return
        }
        when (task.call) {
            SettingsCall.Load -> showSettings(task.request) { SettingsState.Failed(reason(error)) }
            is SettingsCall.SetAnalytics ->
                showSettings(task.request) {
                    it.loaded()?.copy(analyticsSaving = false, analyticsFailed = true)
                }
            is SettingsCall.Save,
            SettingsCall.Remove -> {
                val fields = (error as? ApiException.Invalid)?.fields.orEmpty()
                showSettings(task.request) { it.loaded()?.copy(saving = false, errors = fields) }
                if (fields.isEmpty() && task.request == settingsRequest) fail(error, retry = null)
            }
            is SettingsCall.SaveName,
            is SettingsCall.SaveAvatar,
            SettingsCall.RemoveAvatar -> {
                val fields = (error as? ApiException.Invalid)?.fields.orEmpty()
                showSettings(task.request) {
                    it.loaded()?.copy(profileSaving = false, profileErrors = fields)
                }
                if (fields.isEmpty() && task.request == settingsRequest) fail(error, retry = null)
            }
        }
    }

    private fun fail(error: Exception, retry: (() -> Unit)?) {
        when (error) {
            is ApiException.NotSignedIn -> {
                loginJob?.cancel()
                disconnect()
                _state.value =
                    TodoUiState(
                        auth = AuthState.SignedOut,
                        loginError = "Your sign-in has expired. Sign in again.",
                        server = current.server,
                        showCompleted = current.showCompleted,
                    )
            }
            is ApiException.NotFound -> notify("This todo has been deleted.")
            is ApiException.Invalid -> notify("The server rejected this change.")
            else -> notify(reason(error), retry)
        }
    }

    private fun notify(text: String, retry: (() -> Unit)? = null) {
        noticeId += 1
        _state.update { it.copy(notice = Notice(noticeId, text, retry)) }
    }

    companion object {
        private const val HISTORY_LIMIT = 50
        private val TYPING_DELAY = 500.milliseconds
        private val TYPING_PAUSE = 1.seconds

        fun factory(context: Context): ViewModelProvider.Factory {
            val appContext = context.applicationContext
            return viewModelFactory {
                initializer {
                    val settings = PreferenceSettings(appContext)
                    val uiModes = appContext.getSystemService(UiModeManager::class.java)
                    TodoViewModel(
                        accessBackends(appContext),
                        settings,
                        NetworkSecurityPolicy.getInstance()::isCleartextTrafficPermitted,
                        usage = usageAnalytics(settings),
                        applyAppearance = { uiModes.setApplicationNightMode(nightModeOf(it)) },
                    )
                }
            }
        }
    }
}

private fun nightModeOf(appearance: Appearance) =
    when (appearance) {
        Appearance.System -> UiModeManager.MODE_NIGHT_AUTO
        Appearance.Light -> UiModeManager.MODE_NIGHT_NO
        Appearance.Dark -> UiModeManager.MODE_NIGHT_YES
    }

internal fun accessBackends(context: Context): (HttpUrl) -> Backend {
    val http =
        OkHttpClient.Builder()
            .followRedirects(false)
            .followSslRedirects(false)
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .build()
    val store = KeystoreTokenStore(context)
    return { base -> AccessBackend(http, base, store) }
}

private fun usageAnalytics(settings: Settings): UsageAnalytics? {
    if (BuildConfig.DEBUG) return null
    val http = OkHttpClient.Builder().callTimeout(10, TimeUnit.SECONDS).build()
    return UsageAnalytics(
        settings,
        PostHogCapture(http, PostHogCapture.ENDPOINT, BuildConfig.VERSION_NAME),
    )
}

private fun editEnd(before: String, after: String): Int {
    val prefix = before.commonPrefixWith(after).length
    val suffix = before.substring(prefix).commonSuffixWith(after.substring(prefix)).length
    return before.length - suffix
}

internal fun reason(error: Exception): String =
    when (error) {
        is ApiException.Server -> "The server returned an error (${error.status})."
        is DeviceLockedException -> "Unlock your phone, then try again."
        is IOException -> "Can't connect. Check your connection."
        else -> "Something went wrong."
    }
