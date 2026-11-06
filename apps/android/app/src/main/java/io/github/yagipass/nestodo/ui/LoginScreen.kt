package io.github.yagipass.nestodo.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.AuthState
import io.github.yagipass.nestodo.R

@Composable
fun LoginScreen(
    auth: AuthState,
    error: String?,
    onSignIn: () -> Unit,
    onCancel: () -> Unit,
    onRetry: () -> Unit,
) {
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
                Box(
                    modifier =
                        Modifier.size(80.dp)
                            .clip(RoundedCornerShape(18.dp))
                            .background(colorResource(R.color.ic_launcher_background))
                ) {
                    Image(
                        painterResource(R.drawable.ic_launcher_foreground),
                        contentDescription = null,
                        modifier = Modifier.fillMaxSize().scale(1.25f),
                    )
                }
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
                if (error != null) {
                    Text(
                        error,
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                when (auth) {
                    AuthState.Checking,
                    AuthState.SignedIn -> CircularProgressIndicator()
                    AuthState.Unavailable -> WideButton("Try again", onRetry)
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
                    }
                }
            }
        }
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
