package io.github.yagipass.nestodo.domain

data class Shared(val title: String, val note: String)

fun sharedTodo(subject: String?, text: String?): Shared? {
    val body = text.orEmpty().let { encodePastedUrl(it) ?: it }
    val lines = body.lines().dropWhile { it.isBlank() }
    val line = lines.firstOrNull()?.trim().orEmpty()
    val rest = lines.drop(1).dropWhile { it.isBlank() }.dropLastWhile { it.isBlank() }
    val heading =
        subject.orEmpty().lines().map { it.trim() }.filter { it.isNotEmpty() }.joinToString(" ")
    val joined =
        when {
            heading.isEmpty() || heading in body -> line
            line.isEmpty() -> heading
            else -> "$heading $line"
        }
    val first = joined.takeWhile { !it.isWhitespace() }
    val leading = encodePastedUrl(first)
    val afterLeading = if (leading == null) joined else joined.substring(first.length).trimStart()
    val last = afterLeading.takeLastWhile { !it.isWhitespace() }
    val trailing = encodePastedUrl(last)
    val title = if (trailing == null) afterLeading else afterLeading.dropLast(last.length).trimEnd()
    val note = (listOfNotNull(leading, trailing) + rest).joinToString("\n")
    if (title.isEmpty() && note.isEmpty()) return null
    return Shared(title.take(MAX_TITLE_LENGTH), note.take(MAX_NOTE_LENGTH))
}
