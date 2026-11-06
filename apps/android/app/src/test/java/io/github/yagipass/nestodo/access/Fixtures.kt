package io.github.yagipass.nestodo.access

import java.util.Base64

object Fixtures {
    const val NOW = 1_792_000_000L

    fun text(name: String): String =
        Fixtures::class.java.getResource("/access/$name")!!.readText().trim()

    fun token(payload: String): String =
        "e30.${Base64.getUrlEncoder().withoutPadding().encodeToString(payload.toByteArray())}.c2ln"
}

class MemoryTokenStore : TokenStore {
    private val tokens = mutableMapOf<Pair<String, TokenKind>, String>()

    override fun load(host: String, kind: TokenKind): String? = tokens[host to kind]

    override fun save(host: String, kind: TokenKind, token: String) {
        tokens[host to kind] = token
    }

    override fun delete(host: String, kind: TokenKind) {
        tokens.remove(host to kind)
    }
}
