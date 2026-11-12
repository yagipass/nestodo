package io.github.yagipass.nestodo.domain

data class Node(
    val key: Long,
    val parent: Long?,
    val title: String,
    val completed: Boolean = false,
    val collapsed: Boolean = false,
    val children: List<Long> = emptyList(),
    val note: String = "",
)

sealed interface Change {
    val key: Long

    data class Create(
        override val key: Long,
        val parent: Long?,
        val index: Int,
        val title: String,
        val note: String = "",
    ) : Change

    data class Rename(override val key: Long, val title: String) : Change

    data class Annotate(override val key: Long, val note: String) : Change

    data class Complete(override val key: Long, val completed: Boolean) : Change

    data class Collapse(override val key: Long, val collapsed: Boolean) : Change

    data class Move(override val key: Long, val parent: Long?, val index: Int) : Change

    data class Delete(override val key: Long, val recursive: Boolean = false) : Change
}

data class Viewport(val showCompleted: Boolean = true)

data class OutlineRow(
    val key: Long,
    val title: String,
    val depth: Int,
    val completed: Boolean,
    val dimmed: Boolean,
    val hasChildren: Boolean,
    val collapsed: Boolean,
    val note: String = "",
)

data class Guide(val through: List<Boolean>, val stem: Boolean)

fun guides(rows: List<OutlineRow>): List<Guide> {
    val followed = BooleanArray(rows.size)
    val later = mutableListOf<Boolean>()
    for (i in rows.indices.reversed()) {
        val depth = rows[i].depth
        followed[i] = later.getOrElse(depth) { false }
        later.resize(depth)
        later += true
    }
    val open = mutableListOf<Boolean>()
    return rows.mapIndexed { i, row ->
        open.resize(row.depth)
        open += followed[i]
        Guide(through = open.drop(1), stem = (rows.getOrNull(i + 1)?.depth ?: 0) > row.depth)
    }
}

private fun MutableList<Boolean>.resize(size: Int) {
    while (this.size > size) removeAt(lastIndex)
    while (this.size < size) add(false)
}

data class Outline(val nodes: Map<Long, Node> = emptyMap(), val roots: List<Long> = emptyList()) {
    operator fun get(key: Long): Node? = nodes[key]

    operator fun contains(key: Long): Boolean = key in nodes

    fun children(parent: Long?): List<Long> =
        if (parent == null) roots else nodes[parent]?.children.orEmpty()

    fun indexOf(key: Long): Int = nodes[key]?.let { children(it.parent).indexOf(key) } ?: -1

    fun subtree(key: Long): List<Long> =
        listOf(key) + nodes[key]?.children.orEmpty().flatMap(::subtree)

    fun depth(key: Long): Int = generateSequence(nodes[key]) { it.parent?.let(nodes::get) }.count()

    fun height(key: Long): Int = 1 + (nodes[key]?.children.orEmpty().maxOfOrNull(::height) ?: 0)

    fun rows(viewport: Viewport): List<OutlineRow> = buildList {
        fun walk(parent: Long?, depth: Int, dimmed: Boolean) {
            for (key in children(parent)) {
                val node = nodes.getValue(key)
                if (!viewport.showCompleted && node.completed) continue
                add(
                    OutlineRow(
                        key = key,
                        title = node.title,
                        depth = depth,
                        completed = node.completed,
                        dimmed = dimmed,
                        hasChildren = node.children.isNotEmpty(),
                        collapsed = node.collapsed,
                        note = node.note,
                    )
                )
                if (!node.collapsed) walk(key, depth + 1, dimmed || node.completed)
            }
        }
        walk(null, 0, false)
    }

    fun accepts(change: Change): Boolean =
        when (change) {
            is Change.Create ->
                change.key !in nodes && (change.parent == null || change.parent in nodes)
            is Change.Move ->
                change.key in nodes &&
                    (change.parent == null ||
                        (change.parent in nodes && change.parent !in subtree(change.key)))
            else -> change.key in nodes
        }

    fun accepts(changes: List<Change>): Boolean {
        var outline = this
        for (change in changes) {
            if (!outline.accepts(change)) return false
            outline = outline.apply(change)
        }
        return true
    }

    fun apply(change: Change): Outline =
        when (change) {
            is Change.Create ->
                attach(
                    Node(change.key, change.parent, change.title, note = change.note),
                    change.index,
                )
            is Change.Rename -> update(change.key) { it.copy(title = change.title) }
            is Change.Annotate -> update(change.key) { it.copy(note = change.note) }
            is Change.Complete -> update(change.key) { it.copy(completed = change.completed) }
            is Change.Collapse -> update(change.key) { it.copy(collapsed = change.collapsed) }
            is Change.Move -> {
                val node = nodes.getValue(change.key)
                detach(node).attach(node.copy(parent = change.parent), change.index)
            }
            is Change.Delete -> {
                val node = nodes.getValue(change.key)
                val detached = detach(node)
                detached.copy(nodes = detached.nodes - subtree(change.key).toSet())
            }
        }

    fun undoFor(changes: List<Change>): List<Change>? {
        var outline = this
        val steps = mutableListOf<List<Change>>()
        for (change in changes) {
            steps += outline.inverse(change) ?: return null
            outline = outline.apply(change)
        }
        return steps.asReversed().flatten()
    }

    private fun inverse(change: Change): List<Change>? {
        val node = nodes[change.key]
        return when (change) {
            is Change.Create -> listOf(Change.Delete(change.key))
            is Change.Rename -> node?.let { listOf(Change.Rename(it.key, it.title)) }
            is Change.Annotate -> node?.let { listOf(Change.Annotate(it.key, it.note)) }
            is Change.Complete -> node?.let { listOf(Change.Complete(it.key, it.completed)) }
            is Change.Collapse -> node?.let { listOf(Change.Collapse(it.key, it.collapsed)) }
            is Change.Move -> node?.let { listOf(Change.Move(it.key, it.parent, indexOf(it.key))) }
            is Change.Delete -> node?.let { subtree(it.key).flatMap(::recreate) }
        }
    }

    private fun recreate(key: Long): List<Change> {
        val node = nodes.getValue(key)
        return listOfNotNull(
            Change.Create(key, node.parent, indexOf(key), node.title, node.note),
            Change.Complete(key, true).takeIf { node.completed },
            Change.Collapse(key, true).takeIf { node.collapsed },
        )
    }

    private fun update(key: Long, transform: (Node) -> Node): Outline {
        val node = nodes[key] ?: return this
        return copy(nodes = nodes + (key to transform(node)))
    }

    private fun withChildren(parent: Long?, children: List<Long>): Outline =
        if (parent == null) copy(roots = children)
        else update(parent) { it.copy(children = children) }

    private fun detach(node: Node): Outline =
        withChildren(node.parent, children(node.parent) - node.key)

    private fun attach(node: Node, index: Int): Outline {
        val siblings = children(node.parent).toMutableList()
        siblings.add(index.coerceIn(0, siblings.size), node.key)
        return copy(nodes = nodes + (node.key to node)).withChildren(node.parent, siblings)
    }
}

fun outlineOf(todos: List<Todo>, keyOf: (Long) -> Long = { it }): Outline {
    val ids = todos.mapTo(mutableSetOf()) { it.id }
    fun parentOf(todo: Todo): Long? = todo.parentId?.takeIf { it in ids }
    val byParent = todos.groupBy(::parentOf)
    fun childrenOf(parent: Long?): List<Long> =
        byParent[parent].orEmpty().sortedWith(compareBy({ it.position }, { it.id })).map {
            keyOf(it.id)
        }
    val nodes = todos.associate { todo ->
        val key = keyOf(todo.id)
        key to
            Node(
                key = key,
                parent = parentOf(todo)?.let(keyOf),
                title = todo.title,
                completed = todo.completed,
                collapsed = todo.collapsed,
                children = childrenOf(todo.id),
                note = todo.note,
            )
    }
    return Outline(nodes, childrenOf(null))
}
