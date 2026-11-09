package io.github.yagipass.nestodo.access

import io.github.yagipass.nestodo.data.Credentials
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

enum class TokenKind {
    App,
    Org,
}

interface TokenStore {
    suspend fun load(host: String, kind: TokenKind): String?

    suspend fun save(host: String, kind: TokenKind, token: String)

    suspend fun delete(host: String, kind: TokenKind)
}

sealed interface Plan {
    data class Use(val token: String) : Plan

    data class Exchange(val orgToken: String) : Plan

    data object SignIn : Plan
}

fun plan(appToken: String?, orgToken: String?, app: AccessApp, now: Long): Plan {
    if (
        appToken != null &&
            AccessToken.parse(appToken)?.let { it.isLive(now) && it.isFor(app.aud) } == true
    ) {
        return Plan.Use(appToken)
    }
    if (
        orgToken != null &&
            AccessToken.parse(orgToken)?.let { it.isLive(now) && it.isIssuedBy(app.authDomain) } ==
                true
    ) {
        return Plan.Exchange(orgToken)
    }
    return Plan.SignIn
}

class AccessSession(
    private val http: OkHttpClient,
    private val base: HttpUrl,
    private val app: AccessApp,
    private val store: TokenStore,
    private var appToken: String,
    private val clock: () -> Long,
) : Credentials {
    private val mutex = Mutex()

    val email: String?
        get() = AccessToken.parse(appToken)?.email

    override fun tokenFor(url: HttpUrl): String? = appToken.takeIf { url.host == app.hostname }

    override suspend fun refresh(rejected: String?): Boolean = mutex.withLock {
        if (appToken != rejected) return@withLock true
        val token = exchange(store.load(app.hostname, TokenKind.Org)) ?: return@withLock false
        appToken = token
        true
    }

    private suspend fun exchange(orgToken: String?): String? {
        val plan = plan(null, orgToken, app, clock())
        if (plan !is Plan.Exchange) return null
        val token =
            Exchange.exchangeOrgToken(http, base, app, plan.orgToken, clock()) ?: return null
        store.save(app.hostname, TokenKind.App, token)
        return token
    }

    companion object {
        suspend fun open(
            http: OkHttpClient,
            base: HttpUrl,
            app: AccessApp,
            store: TokenStore,
            clock: () -> Long,
        ): AccessSession? {
            val token =
                when (
                    val plan =
                        plan(
                            store.load(app.hostname, TokenKind.App),
                            store.load(app.hostname, TokenKind.Org),
                            app,
                            clock(),
                        )
                ) {
                    is Plan.Use -> plan.token
                    is Plan.Exchange ->
                        Exchange.exchangeOrgToken(http, base, app, plan.orgToken, clock())?.also {
                            store.save(app.hostname, TokenKind.App, it)
                        }
                    Plan.SignIn -> null
                } ?: return null
            return AccessSession(http, base, app, store, token, clock)
        }
    }
}
