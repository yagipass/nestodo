package io.github.yagipass.nestodo.access

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.Headers
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.coroutines.executeAsync

class HttpResult(val url: HttpUrl, val code: Int, val headers: Headers, val body: ByteArray) {
    fun header(name: String): String? = headers[name]

    fun location(): HttpUrl? = headers["Location"]?.let(url::resolve)
}

suspend fun OkHttpClient.fetch(request: Request, maxBytes: Int? = null): HttpResult =
    newCall(request).executeAsync().use { response ->
        val body =
            withContext(Dispatchers.IO) {
                if (maxBytes != null && response.body.source().request(maxBytes + 1L)) {
                    throw AccessException("${request.url} returned more than $maxBytes bytes")
                }
                response.body.bytes()
            }
        HttpResult(request.url, response.code, response.headers, body)
    }

internal fun Headers.cookie(name: String): String? =
    values("Set-Cookie").firstNotNullOfOrNull { header ->
        val pair = header.substringBefore(';')
        val separator = pair.indexOf('=')
        if (separator >= 0 && pair.substring(0, separator).trim() == name) {
            pair.substring(separator + 1).trim()
        } else {
            null
        }
    }
