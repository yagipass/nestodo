package io.github.yagipass.nestodo.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import io.github.yagipass.nestodo.SettingsState
import io.github.yagipass.nestodo.TodoViewModel
import io.github.yagipass.nestodo.data.avatarImage
import io.github.yagipass.nestodo.domain.Avatar
import io.github.yagipass.nestodo.domain.Profile
import io.github.yagipass.nestodo.domain.Webhook
import io.github.yagipass.nestodo.domain.WebhookForm
import io.github.yagipass.nestodo.domain.WebhookHeader
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val HIDDEN = "••••••••"

private val ButtonShape = RoundedCornerShape(10.dp)

private val FieldShape = RoundedCornerShape(12.dp)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    settings: SettingsState,
    profile: Profile?,
    avatar: Avatar?,
    snackbar: SnackbarHostState,
    viewModel: TodoViewModel,
) {
    BackHandler(onBack = viewModel::closeSettings)
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Settings") },
                navigationIcon = {
                    IconButton(onClick = viewModel::closeSettings) {
                        Icon(NestodoIcons.Back, contentDescription = "Back")
                    }
                },
                colors =
                    TopAppBarDefaults.topAppBarColors(
                        containerColor = MaterialTheme.colorScheme.background
                    ),
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            when (settings) {
                SettingsState.Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                is SettingsState.Failed ->
                    Column(
                        Modifier.align(Alignment.Center).padding(24.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        Text(settings.message, textAlign = TextAlign.Center)
                        TextButton(onClick = viewModel::openSettings) { Text("Retry") }
                    }
                is SettingsState.Loaded ->
                    Column(
                        Modifier.fillMaxSize()
                            .imePadding()
                            .verticalScroll(rememberScrollState())
                            .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 24.dp),
                        verticalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        ProfileCard(profile, avatar, settings, viewModel)
                        WebhookCard(settings, viewModel)
                        if (settings.analytics.available) AnalyticsCard(settings, viewModel)
                    }
            }
        }
    }
}

@Composable
private fun SettingsCard(
    title: String,
    action: (@Composable () -> Unit)? = null,
    content: @Composable () -> Unit,
) {
    Surface(
        Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.surfaceContainer,
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    title,
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.weight(1f),
                )
                action?.invoke()
            }
            content()
        }
    }
}

@Composable
private fun Note(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun Problem(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.error,
    )
}

@Composable
private fun ProfileCard(
    profile: Profile?,
    avatar: Avatar?,
    settings: SettingsState.Loaded,
    viewModel: TodoViewModel,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var reading by remember { mutableStateOf(false) }
    val busy = settings.profileSaving || reading
    val errors = settings.profileErrors
    val pick =
        rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
            if (uri == null) return@rememberLauncherForActivityResult
            reading = true
            scope.launch {
                val image =
                    withContext(Dispatchers.IO) { avatarImage(context.contentResolver, uri) }
                reading = false
                if (image == null) viewModel.avatarUnreadable() else viewModel.uploadAvatar(image)
            }
        }
    SettingsCard(title = "Profile") {
        settings.profileProblem?.let {
            Problem("Couldn't load your profile: $it")
            return@SettingsCard
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Box(
                Modifier.size(64.dp)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surfaceContainerHighest),
                contentAlignment = Alignment.Center,
            ) {
                AccountPicture(
                    avatar,
                    size = 64.dp,
                    iconSize = 32.dp,
                    contentDescription = "Profile picture",
                )
            }
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Button(
                    onClick = {
                        pick.launch(
                            PickVisualMediaRequest(
                                ActivityResultContracts.PickVisualMedia.ImageOnly
                            )
                        )
                    },
                    enabled = !busy,
                    shape = ButtonShape,
                ) {
                    Text("Choose image")
                }
                if (profile?.avatarUrl != null) {
                    TextButton(
                        shape = ButtonShape,
                        onClick = viewModel::removeAvatar,
                        enabled = !busy,
                    ) {
                        Text("Remove", color = MaterialTheme.colorScheme.error)
                    }
                }
            }
        }
        if (settings.imageUnreadable) Problem("The image could not be read.")
        errors["image"]?.let { Problem(it) }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            OutlinedTextField(
                value = settings.name,
                onValueChange = viewModel::changeName,
                modifier = Modifier.fillMaxWidth(),
                shape = FieldShape,
                label = { Text("Name") },
                placeholder = profile?.email?.let { email -> { Text(email) } },
                singleLine = true,
                isError = errors["name"] != null,
                supportingText = errors["name"]?.let { { Text(it) } },
            )
            Hint("Shown instead of your email address. Leave it blank to show the email address.")
        }
        Button(onClick = viewModel::saveName, enabled = !busy, shape = ButtonShape) {
            Text("Save")
        }
    }
}

@Composable
private fun WebhookCard(settings: SettingsState.Loaded, viewModel: TodoViewModel) {
    val webhook = settings.webhooks.webhook
    val form = settings.form
    SettingsCard(
        title = "Webhook",
        action =
            if (settings.webhooks.available && webhook != null && form == null) {
                { Button(onClick = viewModel::editWebhook, shape = ButtonShape) { Text("Edit") } }
            } else null,
    ) {
        Note(
            "Within 13 minutes of adding a todo with no note or only a link, " +
                "its id, title and note are sent to this URL."
        )
        when {
            !settings.webhooks.available ->
                Problem(
                    "Webhooks are not available because WEBHOOK_SECRET_KEY is not set on this server."
                )
            else -> {
                webhook?.let { WebhookStatus(it) }
                when {
                    form != null -> WebhookFormFields(form, settings, viewModel)
                    webhook != null -> WebhookView(webhook)
                }
            }
        }
    }
}

@Composable
private fun WebhookStatus(webhook: Webhook) {
    webhook.lastSentAt?.let { Note("Last sent $it UTC.") }
    webhook.lastError?.let { Problem("The last send failed: $it.") }
    if (webhook.unreadable) {
        Problem(
            "The saved URL, token and headers can no longer be read. " +
                "Edit this webhook to enter them again."
        )
    }
}

@Composable
private fun WebhookView(webhook: Webhook) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Entry("URL", webhook.url ?: HIDDEN)
        Entry("Token", if (webhook.hasToken) HIDDEN else "None")
        Entry(
            "Headers",
            if (webhook.headers.isEmpty()) "None"
            else
                webhook.headers.joinToString("\n") {
                    "${it.name}: ${if (it.secret) HIDDEN else it.value}"
                },
        )
        Entry("Project", webhook.project)
    }
}

@Composable
private fun Entry(label: String, value: String) {
    Column {
        Text(
            label,
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun SecretToggle(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier =
            Modifier.toggleable(checked, role = Role.Checkbox, onValueChange = onChange).semantics {
                contentDescription = label
            },
    ) {
        Checkbox(checked = checked, onCheckedChange = null)
        Text(
            "Secret",
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.padding(start = 4.dp, end = 8.dp),
        )
    }
}

private fun hidden(secret: Boolean): VisualTransformation =
    if (secret) PasswordVisualTransformation() else VisualTransformation.None

private fun keyboard(secret: Boolean, type: KeyboardType = KeyboardType.Text) =
    KeyboardOptions(
        autoCorrectEnabled = false,
        keyboardType = if (secret) KeyboardType.Password else type,
    )

@Composable
private fun Hint(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun WebhookFormFields(
    form: WebhookForm,
    settings: SettingsState.Loaded,
    viewModel: TodoViewModel,
) {
    val errors = settings.errors
    val change = viewModel::changeWebhook
    var confirming by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OutlinedTextField(
            value = form.url,
            onValueChange = { change(form.copy(url = it)) },
            modifier = Modifier.fillMaxWidth(),
            shape = FieldShape,
            label = { Text("URL") },
            placeholder = {
                Text(
                    if (form.keepsHiddenUrl) "Leave blank to keep the saved URL"
                    else "https://example.com/hook"
                )
            },
            singleLine = true,
            visualTransformation = hidden(form.urlSecret),
            keyboardOptions = keyboard(form.urlSecret, KeyboardType.Uri),
            isError = errors["url"] != null,
            supportingText = errors["url"]?.let { { Text(it) } },
        )
        SecretToggle("Keep the URL secret", form.urlSecret) {
            change(form.copy(urlSecret = it))
        }
        if (form.replacing) {
            Hint(
                "A new URL drops the saved token and asks for secret header values again, " +
                    "so they never reach another server."
            )
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OutlinedTextField(
            value = form.token.orEmpty(),
            onValueChange = { change(form.copy(token = it)) },
            modifier = Modifier.fillMaxWidth(),
            shape = FieldShape,
            label = { Text("Token") },
            placeholder = { Text(if (form.tokenKept) HIDDEN else "Optional") },
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = keyboard(secret = true),
            trailingIcon =
                if (form.tokenKept) {
                    {
                        IconButton(onClick = { change(form.copy(token = null)) }) {
                            Icon(NestodoIcons.Close, contentDescription = "Remove the token")
                        }
                    }
                } else null,
            isError = errors["token"] != null,
            supportingText = errors["token"]?.let { { Text(it) } },
        )
        Hint("Sent as a bearer token and never shown again.")
    }

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Headers", style = MaterialTheme.typography.labelLarge)
        form.headers.forEachIndexed { index, header ->
            HeaderFields(form, index, header, change)
        }
        TextButton(shape = ButtonShape, onClick = { change(form.withNewHeader()) }) {
            Text("Add header")
        }
        errors["headers"]?.let { Problem(it) }
        Hint(
            "A secret value is never shown again." +
                if (form.replacing) " Leave it blank to keep it." else ""
        )
    }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OutlinedTextField(
            value = form.project,
            onValueChange = { change(form.copy(project = it)) },
            modifier = Modifier.fillMaxWidth(),
            shape = FieldShape,
            label = { Text("Project") },
            singleLine = true,
            isError = errors["project"] != null,
            supportingText = errors["project"]?.let { { Text(it) } },
        )
        Hint("The title of a top-level todo. Only todos under it are sent.")
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Button(onClick = viewModel::saveWebhook, enabled = !settings.saving, shape = ButtonShape) {
            Text("Save")
        }
        if (form.existing != null) {
            TextButton(
                shape = ButtonShape,
                onClick = viewModel::cancelWebhook,
                enabled = !settings.saving,
            ) {
                Text("Cancel")
            }
            TextButton(
                shape = ButtonShape,
                onClick = { confirming = true },
                enabled = !settings.saving,
            ) {
                Text("Remove", color = MaterialTheme.colorScheme.error)
            }
        }
    }

    if (confirming) {
        AlertDialog(
            onDismissRequest = { confirming = false },
            title = { Text("Remove this webhook?") },
            text = { Text("Its URL, token and headers cannot be shown again.") },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirming = false
                        viewModel.removeWebhook()
                    }
                ) {
                    Text("Remove")
                }
            },
            dismissButton = { TextButton(onClick = { confirming = false }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun HeaderFields(
    form: WebhookForm,
    index: Int,
    header: WebhookHeader,
    change: (WebhookForm) -> Unit,
) {
    val number = index + 1
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        OutlinedTextField(
            value = header.name,
            onValueChange = { change(form.withHeader(index, header.copy(name = it))) },
            modifier = Modifier.fillMaxWidth(),
            shape = FieldShape,
            label = { Text("Header $number name") },
            placeholder = { Text("Name") },
            singleLine = true,
            keyboardOptions = keyboard(secret = false),
        )
        OutlinedTextField(
            value = header.value,
            onValueChange = { change(form.withHeader(index, header.copy(value = it))) },
            modifier = Modifier.fillMaxWidth(),
            shape = FieldShape,
            label = { Text("Header $number value") },
            placeholder = { Text(if (form.valueKept(header)) HIDDEN else "Value") },
            singleLine = true,
            visualTransformation = hidden(header.secret),
            keyboardOptions = keyboard(header.secret),
        )
        Row(verticalAlignment = Alignment.CenterVertically) {
            SecretToggle("Keep header $number secret", header.secret) {
                change(form.withHeader(index, header.copy(secret = it)))
            }
            TextButton(shape = ButtonShape, onClick = { change(form.withoutHeader(index)) }) {
                Text("Remove header $number")
            }
        }
    }
}

@Composable
private fun AnalyticsCard(settings: SettingsState.Loaded, viewModel: TodoViewModel) {
    SettingsCard(title = "Usage analytics") {
        Note(
            "When on, the pages and screens you open in the web, desktop and Android apps, your browser or device, and your approximate location from your IP address are sent to PostHog. Your todos and email address are never sent. This applies to your account on every device."
        )
        if (settings.analyticsFailed) Problem("The setting could not be saved. Try again.")
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier =
                Modifier.fillMaxWidth()
                    .toggleable(
                        value = settings.analytics.enabled,
                        enabled = !settings.analyticsSaving,
                        role = Role.Switch,
                        onValueChange = viewModel::setAnalytics,
                    ),
        ) {
            Text(
                "Send usage data",
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            Switch(
                checked = settings.analytics.enabled,
                onCheckedChange = null,
                enabled = !settings.analyticsSaving,
            )
        }
    }
}
