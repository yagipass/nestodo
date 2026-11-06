package io.github.yagipass.nestodo

import io.github.yagipass.nestodo.data.Backend
import io.github.yagipass.nestodo.data.Connection
import io.github.yagipass.nestodo.data.LoginFlow
import io.github.yagipass.nestodo.data.Settings
import io.github.yagipass.nestodo.data.TodoRepository
import io.github.yagipass.nestodo.domain.Day
import io.github.yagipass.nestodo.domain.Todo
import kotlinx.coroutines.CompletableDeferred

class FakeRepository(var todos: List<Todo>) : TodoRepository {
    val calls = mutableListOf<String>()
    var days = listOf(Day("2026-10-26", added = 2, completed = 1, open = 3))
    var failure: Exception? = null
    var gate: CompletableDeferred<Unit>? = null
    private var nextId = 100L

    private suspend fun answer(call: String) {
        calls += call
        gate?.await()
        failure?.let {
            failure = null
            throw it
        }
    }

    private fun find(id: Long) = todos.firstOrNull { it.id == id } ?: Todo(id, title = "")

    override suspend fun list(): List<Todo> {
        answer("GET")
        return todos
    }

    override suspend fun create(title: String, parentId: Long?, index: Int, note: String): Todo {
        val withNote = if (note.isEmpty()) "" else " with note \"$note\""
        answer("POST \"$title\" under $parentId at $index$withNote")
        return Todo(nextId++, parentId, index, title, note = note)
    }

    override suspend fun rename(id: Long, title: String): Todo {
        answer("PATCH $id title \"$title\"")
        return find(id)
    }

    override suspend fun setNote(id: Long, note: String): Todo {
        answer("PATCH $id note \"$note\"")
        return find(id)
    }

    override suspend fun setCompleted(id: Long, completed: Boolean): Todo {
        answer("PATCH $id completed $completed")
        return find(id)
    }

    override suspend fun setCollapsed(id: Long, collapsed: Boolean): Todo {
        answer("PATCH $id collapsed $collapsed")
        return find(id)
    }

    override suspend fun move(id: Long, parentId: Long?, index: Int): Todo {
        answer("PATCH $id under $parentId at $index")
        return find(id)
    }

    override suspend fun delete(id: Long, recursive: Boolean) {
        answer(if (recursive) "DELETE $id with its subtree" else "DELETE $id")
    }

    override suspend fun stats(timeZone: String): List<Day> {
        answer("GET stats in $timeZone")
        return days
    }
}

class FakeBackend(private val repository: TodoRepository, var signedIn: Boolean = true) : Backend {
    override suspend fun connect() =
        if (signedIn) Connection.Ready(repository, "family@example.com") else Connection.SignedOut

    override suspend fun startLogin(): LoginFlow = error("not used")

    override suspend fun logout() {}
}

class FakeSettings : Settings {
    override var showCompleted = true
    override var shareParent: Long? = null
}
