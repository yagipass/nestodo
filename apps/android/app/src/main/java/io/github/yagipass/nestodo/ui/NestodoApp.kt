package io.github.yagipass.nestodo.ui

import androidx.compose.material3.SnackbarDuration
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.SnackbarResult
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.yagipass.nestodo.AuthState
import io.github.yagipass.nestodo.TodoViewModel

@Composable
fun NestodoApp(viewModel: TodoViewModel, openInBrowser: (String) -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val snackbar = remember { SnackbarHostState() }

    LaunchedEffect(state.notice) {
        val notice = state.notice ?: return@LaunchedEffect
        val result =
            snackbar.showSnackbar(
                message = notice.text,
                actionLabel = if (notice.retry != null) "Retry" else null,
                duration =
                    if (notice.retry != null) SnackbarDuration.Long else SnackbarDuration.Short,
            )
        viewModel.noticeShown(notice.id)
        if (result == SnackbarResult.ActionPerformed) notice.retry?.invoke()
    }

    if (state.auth != AuthState.SignedIn) {
        LoginScreen(
            auth = state.auth,
            error = state.loginError,
            server = state.server,
            onSetServer = viewModel::setServer,
            onChangeServer = viewModel::changeServer,
            onSignIn = { viewModel.signIn(openInBrowser) },
            onCancel = viewModel::cancelSignIn,
            onRetry = viewModel::connect,
        )
        return
    }

    state.stats?.let { stats ->
        StatsScreen(
            stats = stats,
            snackbar = snackbar,
            onBack = viewModel::closeStats,
            onRetry = viewModel::openStats,
        )
        return
    }

    OutlineScreen(
        state = state,
        viewModel = viewModel,
        snackbar = snackbar,
        onOpenLink = { viewModel.openLink(it, openInBrowser) },
    )
}
