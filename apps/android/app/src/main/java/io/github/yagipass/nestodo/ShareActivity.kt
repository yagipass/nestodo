package io.github.yagipass.nestodo

import android.content.Intent
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import io.github.yagipass.nestodo.domain.sharedTodo
import io.github.yagipass.nestodo.ui.NestodoTheme
import io.github.yagipass.nestodo.ui.ShareScreen

class ShareActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val shared =
            sharedTodo(
                intent.getCharSequenceExtra(Intent.EXTRA_SUBJECT)?.toString(),
                intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString(),
            )
        if (shared == null) {
            Toast.makeText(this, "Nothing to add", Toast.LENGTH_SHORT).show()
            finish()
            return
        }
        val viewModel: ShareViewModel by viewModels { ShareViewModel.factory(this, shared) }
        setContent {
            NestodoTheme {
                ShareScreen(viewModel, onBack = ::finish, onOpenApp = ::openApp, onAdded = ::added)
            }
        }
    }

    private fun openApp() {
        startActivity(
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        )
        finish()
    }

    private fun added(parent: String?) {
        val text = if (parent == null) "Added to the top level" else "Added to \"$parent\""
        Toast.makeText(applicationContext, text, Toast.LENGTH_SHORT).show()
        finish()
    }
}
