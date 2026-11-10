package io.github.yagipass.nestodo.data

import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

private val SCHEME = Regex("^[A-Za-z][A-Za-z0-9+.-]*://")

fun baseUrlOf(input: String): HttpUrl? {
    val address = input.trim()
    if (address.isEmpty()) return null
    val url =
        (if (SCHEME.containsMatchIn(address)) address else "https://$address").toHttpUrlOrNull()
            ?: return null
    return HttpUrl.Builder().scheme(url.scheme).host(url.host).port(url.port).build()
}

fun HttpUrl.label(): String = toString().removeSuffix("/").removePrefix("https://")
