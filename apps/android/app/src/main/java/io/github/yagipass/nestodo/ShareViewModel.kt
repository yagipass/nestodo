package io.github.yagipass.nestodo

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import io.github.yagipass.nestodo.data.ApiException
import io.github.yagipass.nestodo.data.Backend
import io.github.yagipass.nestodo.data.Connection
import io.github.yagipass.nestodo.data.PreferenceSettings
import io.github.yagipass.nestodo.data.Settings
import io.github.yagipass.nestodo.data.TodoRepository
import io.github.yagipass.nestodo.domain.Destination
import io.github.yagipass.nestodo.domain.Outline
import io.github.yagipass.nestodo.domain.Shared
import io.github.yagipass.nestodo.domain.Viewport
import io.github.yagipass.nestodo.domain.destinations
import io.github.yagipass.nestodo.domain.outlineOf
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface ShareState {
    data object Loading : ShareState

    data object SignedOut : ShareState

    data class Failed(val message: String) : ShareState

    data class Ready(
        val outline: Outline,
        val viewport: Viewport,
        val last: Destination?,
        val adding: Boolean = false,
        val error: String? = null,
    ) : ShareState

    data class Added(val parent: String?) : ShareState
}

class ShareViewModel(
    private val backend: Backend,
    private val settings: Settings,
    val shared: Shared,
) : ViewModel() {
    private val _state = MutableStateFlow<ShareState>(ShareState.Loading)
    val state: StateFlow<ShareState> = _state.asStateFlow()

    private var repository: TodoRepository? = null

    init {
        retry()
    }

    fun retry() {
        _state.value = ShareState.Loading
        viewModelScope.launch { load() }
    }

    fun add(parent: Long?) {
        val ready = _state.value as? ShareState.Ready ?: return
        val repository = repository ?: return
        if (ready.adding) return
        _state.value = ready.copy(adding = true, error = null)
        viewModelScope.launch {
            try {
                val index = ready.outline.children(parent).size
                repository.create(shared.title, parent, index, shared.note)
                settings.shareParent = parent
                _state.value = ShareState.Added(parent?.let { ready.outline[it]?.title })
            } catch (e: CancellationException) {
                throw e
            } catch (_: ApiException.Invalid) {
                load(error = "The todo you picked has been deleted.")
            } catch (e: Exception) {
                fail(e) { ready.copy(error = reason(e)) }
            }
        }
    }

    private suspend fun load(error: String? = null) {
        try {
            val todos = (repository ?: connect() ?: return).list()
            val outline = outlineOf(todos)
            val viewport = Viewport(settings.showCompleted)
            val last =
                settings.shareParent?.let { id ->
                    outline.destinations(viewport, null, "").firstOrNull { it.key == id }
                }
            _state.value = ShareState.Ready(outline, viewport, last, error = error)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            fail(e) { ShareState.Failed(reason(e)) }
        }
    }

    private suspend fun connect(): TodoRepository? =
        when (val connection = backend.connect()) {
            is Connection.Ready -> connection.todos.also { repository = it }
            Connection.SignedOut -> {
                _state.value = ShareState.SignedOut
                null
            }
        }

    private fun fail(error: Exception, otherwise: () -> ShareState) {
        _state.value = if (error is ApiException.NotSignedIn) ShareState.SignedOut else otherwise()
    }

    companion object {
        fun factory(context: Context, shared: Shared): ViewModelProvider.Factory {
            val appContext = context.applicationContext
            return viewModelFactory {
                initializer {
                    ShareViewModel(
                        accessBackend(appContext),
                        PreferenceSettings(appContext),
                        shared,
                    )
                }
            }
        }
    }
}
