package io.github.yagipass.nestodo.data

import io.github.yagipass.nestodo.access.AccessApp
import io.github.yagipass.nestodo.access.AccessException
import io.github.yagipass.nestodo.access.AccessSession
import io.github.yagipass.nestodo.access.AccessToken
import io.github.yagipass.nestodo.access.Metadata
import io.github.yagipass.nestodo.access.TokenKind
import io.github.yagipass.nestodo.access.TokenStore
import io.github.yagipass.nestodo.access.Transfer
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

sealed interface Connection {
    data class Ready(val todos: TodoRepository, val email: String?) : Connection

    data object SignedOut : Connection
}

interface LoginFlow {
    val url: String

    suspend fun await(): Connection.Ready
}

interface Backend {
    suspend fun connect(): Connection

    suspend fun startLogin(): LoginFlow

    suspend fun logout()
}

class AccessBackend(
    private val http: OkHttpClient,
    private val base: HttpUrl,
    private val store: TokenStore,
    private val clock: () -> Long = { System.currentTimeMillis() / 1000 },
) : Backend {
    private var discovered: AccessApp? = null

    private suspend fun app(): AccessApp? =
        discovered ?: Metadata.discover(http, base, clock()).also { discovered = it }

    override suspend fun connect(): Connection =
        withContext(Dispatchers.IO) {
            val app = app() ?: return@withContext Connection.Ready(TodoApi(http, base, null), null)
            val session =
                AccessSession.open(http, base, app, store, clock)
                    ?: return@withContext Connection.SignedOut
            Connection.Ready(TodoApi(http, base, session), session.email)
        }

    override suspend fun startLogin(): LoginFlow {
        val app =
            withContext(Dispatchers.IO) { app() }
                ?: throw AccessException("${base.host} is not protected by Cloudflare Access")
        val pending = Transfer.start(base, app.aud)
        return object : LoginFlow {
            override val url = pending.loginUrl.toString()

            override suspend fun await(): Connection.Ready =
                withContext(Dispatchers.IO) {
                    val tokens = Transfer.await(http, pending)
                    val parsed =
                        AccessToken.parse(tokens.appToken)?.takeIf {
                            it.isLive(clock()) && it.isFor(app.aud)
                        }
                            ?: throw AccessException(
                                "Access returned a token that is not valid for this application"
                            )
                    store.save(app.hostname, TokenKind.App, tokens.appToken)
                    if (tokens.orgToken.isEmpty()) {
                        store.delete(app.hostname, TokenKind.Org)
                    } else {
                        store.save(app.hostname, TokenKind.Org, tokens.orgToken)
                    }
                    val session = AccessSession(http, base, app, store, tokens.appToken, clock)
                    Connection.Ready(TodoApi(http, base, session), parsed.email)
                }
        }
    }

    override suspend fun logout() =
        withContext(Dispatchers.IO) {
            store.delete(base.host, TokenKind.App)
            store.delete(base.host, TokenKind.Org)
        }
}
