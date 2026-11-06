package io.github.yagipass.nestodo.domain

import java.net.IDN

data class Link(val start: Int, val end: Int, val url: String) {
    val label: String
        get() = host(url).lowercase().replaceFirst(LeadingWww, "")
}

data class Pasted(val text: String, val caret: Int)

private val Scheme = Regex("https?://", RegexOption.IGNORE_CASE)
private val LeadingWww = Regex("^www\\.(?=.)", RegexOption.IGNORE_CASE)

private class UrlEnd {
    private var parens = 0
    private var brackets = 0

    fun at(c: Char): Boolean =
        when (c) {
            '(' -> {
                parens++
                false
            }
            '[' -> {
                brackets++
                false
            }
            ')' ->
                if (parens == 0) true
                else {
                    parens--
                    false
                }
            ']' ->
                if (brackets == 0) true
                else {
                    brackets--
                    false
                }
            '"',
            '`',
            '<',
            '>' -> true
            else -> c !in '!'..'~'
        }
}

private fun host(url: String): String =
    url.substring(url.indexOf("://") + 3)
        .takeWhile { it != '/' && it != '\\' && it != '?' && it != '#' }
        .substringAfterLast('@')

fun findLinks(text: String): List<Link> {
    val links = mutableListOf<Link>()
    for (match in Scheme.findAll(text)) {
        val start = match.range.first
        if (start < (links.lastOrNull()?.end ?: 0)) continue
        val ends = UrlEnd()
        var end = match.range.last + 1
        while (end < text.length && !ends.at(text[end])) end++
        val url = text.substring(start, end)
        if (host(url).isNotEmpty()) links += Link(start, end, url)
    }
    return links
}

private fun percentEncode(chunk: String): String =
    chunk.toByteArray(Charsets.UTF_8).joinToString("") { "%%%02X".format(it.toInt() and 0xff) }

private fun asciiHost(server: String): String? {
    if (server.all { it < '\u0080' }) return server
    val colon = server.lastIndexOf(':')
    val host = if (colon < 0) server else server.substring(0, colon)
    val port = if (colon < 0) "" else server.substring(colon)
    return try {
        IDN.toASCII(host).lowercase() + port
    } catch (e: IllegalArgumentException) {
        null
    }
}

fun encodePastedUrl(text: String): String? {
    val pasted = text.trim()
    val scheme = Scheme.matchAt(pasted, 0)?.value ?: return null
    if (pasted.any { it.isWhitespace() }) return null
    val rest = pasted.substring(scheme.length)
    val head = rest.takeWhile { it != '/' && it != '?' && it != '#' }
    val at = head.lastIndexOf('@') + 1
    val server = asciiHost(head.substring(at))?.takeIf { it.isNotEmpty() } ?: return null
    val tail = head.substring(0, at) + server + rest.substring(head.length)
    val ends = UrlEnd()
    return buildString {
        append(scheme)
        var i = 0
        while (i < tail.length) {
            val next = i + Character.charCount(tail.codePointAt(i))
            val chunk = tail.substring(i, next)
            append(if (ends.at(chunk[0])) percentEncode(chunk) else chunk)
            i = next
        }
    }
}

fun encodePaste(before: String, start: Int, end: Int, after: String): Pasted? {
    if (after == before) return null
    val prefix = before.substring(0, start)
    val suffix = before.substring(end)
    if (after.length < prefix.length + suffix.length) return null
    if (!after.startsWith(prefix) || !after.endsWith(suffix)) return null
    val inserted = after.substring(start, after.length - suffix.length)
    val url = encodePastedUrl(inserted)?.takeIf { it != inserted } ?: return null
    return Pasted(prefix + url + suffix, start + url.length)
}
