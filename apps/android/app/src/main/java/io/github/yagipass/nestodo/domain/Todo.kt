package io.github.yagipass.nestodo.domain

import kotlinx.serialization.Serializable

@Serializable
data class Todo(
    val id: Long,
    val parentId: Long? = null,
    val position: Int = 0,
    val title: String,
    val completedAt: String? = null,
    val collapsed: Boolean = false,
    val note: String = "",
) {
    val completed: Boolean
        get() = completedAt != null
}

const val MAX_TITLE_LENGTH = 2000

const val MAX_NOTE_LENGTH = 10000

const val MAX_DEPTH = 64

private val LineBreak = Regex("\r\n?|\n")

fun singleLine(title: String): String = title.replace(LineBreak, " ")
