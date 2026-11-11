package io.github.yagipass.nestodo.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.AuthState
import io.github.yagipass.nestodo.R

@Composable
fun LoginScreen(
    auth: AuthState,
    error: String?,
    server: String?,
    onSetServer: (String) -> Unit,
    onChangeServer: () -> Unit,
    onCancelServerChange: (() -> Unit)?,
    onSignIn: () -> Unit,
    onCancel: () -> Unit,
    onRetry: () -> Unit,
) {
    if (onCancelServerChange != null) BackHandler(onBack = onCancelServerChange)
    Surface(color = MaterialTheme.colorScheme.background, modifier = Modifier.fillMaxSize()) {
        Column(
            modifier =
                Modifier.fillMaxSize()
                    .safeDrawingPadding()
                    .padding(horizontal = 24.dp)
                    .padding(bottom = 40.dp)
        ) {
            Column(
                modifier = Modifier.weight(1f).fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(16.dp, Alignment.CenterVertically),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                AppIcon(80.dp)
                Text(
                    stringResource(R.string.app_name),
                    style = MaterialTheme.typography.headlineLarge,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    "A todo list for everyone in the family",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Column(
                verticalArrangement = Arrangement.spacedBy(16.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                if (
                    error != null && auth != AuthState.NoServer && auth != AuthState.CheckingServer
                ) {
                    Text(
                        error,
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                when (auth) {
                    AuthState.NoServer,
                    AuthState.CheckingServer ->
                        ServerForm(
                            server,
                            error,
                            checking = auth == AuthState.CheckingServer,
                            onSetServer,
                            onCancelServerChange,
                        )
                    AuthState.Checking,
                    AuthState.SignedIn -> CircularProgressIndicator()
                    AuthState.Unavailable -> {
                        WideButton("Try again", onRetry)
                        ServerLine(server, onChangeServer)
                    }
                    AuthState.WaitingForBrowser -> {
                        WideButton("Signing in…", onClick = {}, enabled = false)
                        Hint(
                            "In the browser, sign in and tap Approve. " +
                                "Cloudflare calls this app a command-line tool.\n" +
                                "If the browser stays open afterward, close it to come back."
                        )
                        TextButton(onClick = onCancel) { Text("Cancel") }
                    }
                    AuthState.SignedOut -> {
                        WideButton("Sign in", onSignIn)
                        Hint(
                            "Opens the Cloudflare Access sign-in in your browser.\n" +
                                "Use the same account as the web app."
                        )
                        ServerLine(server, onChangeServer)
                    }
                }
            }
        }
    }
}

@Composable
private fun ServerForm(
    server: String?,
    error: String?,
    checking: Boolean,
    onSubmit: (String) -> Unit,
    onCancel: (() -> Unit)?,
) {
    val address = rememberTextFieldState(server.orEmpty())
    val submit = { if (!checking) onSubmit(address.text.toString()) }
    OutlinedTextField(
        state = address,
        modifier = Modifier.fillMaxWidth(),
        label = { Text("Server") },
        placeholder = { Text("nestodo.example.com") },
        supportingText = error?.let { { Text(it) } },
        isError = error != null,
        keyboardOptions =
            KeyboardOptions(
                autoCorrectEnabled = false,
                keyboardType = KeyboardType.Uri,
                imeAction = ImeAction.Go,
            ),
        onKeyboardAction = { submit() },
        lineLimits = TextFieldLineLimits.SingleLine,
    )
    WideButton(
        if (checking) "Checking…" else "Continue",
        submit,
        enabled = !checking && address.text.isNotBlank(),
    )
    Hint("The address you open the web app at.")
    if (onCancel != null) {
        TextButton(onClick = onCancel, enabled = !checking) { Text("Cancel") }
    }
}

@Composable
private fun ServerLine(server: String?, onChange: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        if (server != null) {
            Text(
                server,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f, fill = false),
            )
        }
        TextButton(onClick = onChange) { Text("Change server") }
    }
}

@Composable
private fun WideButton(label: String, onClick: () -> Unit, enabled: Boolean = true) {
    Button(onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth().height(52.dp)) {
        Text(label, style = MaterialTheme.typography.titleMedium)
    }
}

@Composable
private fun Hint(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth(),
    )
}
