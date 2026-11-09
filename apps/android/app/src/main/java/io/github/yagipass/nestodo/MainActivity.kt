package io.github.yagipass.nestodo

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.net.toUri
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import io.github.yagipass.nestodo.ui.NestodoApp
import io.github.yagipass.nestodo.ui.NestodoTheme
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private val viewModel: TodoViewModel by viewModels { TodoViewModel.factory(this) }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        lifecycleScope.launch { viewModel.loginEvents.collect { returnFromBrowser() } }
        setContent { NestodoTheme { NestodoApp(viewModel, openInBrowser = ::openInBrowser) } }
    }

    override fun onStart() {
        super.onStart()
        viewModel.onForeground()
    }

    private fun openInBrowser(url: String) {
        CustomTabsIntent.Builder().build().launchUrl(this, url.toUri().normalizeScheme())
    }

    private fun returnFromBrowser() {
        if (lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
        startActivity(
            Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        )
    }
}
