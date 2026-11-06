package io.github.yagipass.nestodo.domain

data class Shared(val title: String, val note: String)

fun sharedTodo(subject: String?, text: String?): Shared? {
    val body = text.orEmpty().let { encodePastedUrl(it) ?: it }
    val lines = body.lines().dropWhile { it.isBlank() }
    val line = lines.firstOrNull()?.trim().orEmpty()
    val note =
        lines.drop(1).dropWhile { it.isBlank() }.dropLastWhile { it.isBlank() }.joinToString("\n")
    val heading =
        subject.orEmpty().lines().map { it.trim() }.filter { it.isNotEmpty() }.joinToString(" ")
    val title =
        when {
            heading.isEmpty() || heading in body -> line
            line.isEmpty() -> heading
            else -> "$heading $line"
        }
    if (title.isEmpty()) return null
    return Shared(title.take(MAX_TITLE_LENGTH), note.take(MAX_NOTE_LENGTH))
}
