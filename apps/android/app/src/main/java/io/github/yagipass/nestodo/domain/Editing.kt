package io.github.yagipass.nestodo.domain

data class Caret(val key: Long, val position: Int, val inNote: Boolean = false)

data class Edit(val changes: List<Change>, val caret: Caret?)

data class Destination(val key: Long, val title: String, val path: List<String>)

fun caretIn(text: String, position: Int): Int {
    val at = position.coerceIn(0, text.length)
    return if (text.hasSurrogatePairAt(at - 1)) at + 1 else at
}

fun Outline.enter(viewport: Viewport, key: Long, cursor: Int, newKey: Long): Edit? {
    val node = this[key] ?: return null
    val rows = rows(viewport)
    val at = rows.indexOfFirst { it.key == key }
    if (at < 0) return null
    val row = rows[at]
    if (node.title.isEmpty() && node.children.isEmpty() && row.depth > 0) {
        return outdent(key, 0)
    }
    val index = indexOf(key)
    val split = cursor.coerceIn(0, node.title.length)
    if (split == 0 && node.title.isNotEmpty()) {
        return Edit(listOf(Change.Create(newKey, node.parent, index, "")), Caret(key, 0))
    }
    val head = node.title.take(split)
    val tail = node.title.drop(split)
    val opensChildren = tail.isEmpty() && (rows.getOrNull(at + 1)?.depth ?: 0) > row.depth
    val create =
        if (opensChildren) Change.Create(newKey, key, 0, tail)
        else Change.Create(newKey, node.parent, index + 1, tail)
    val rename = Change.Rename(key, head).takeIf { tail.isNotEmpty() }
    return Edit(listOfNotNull(create, rename), Caret(newKey, 0))
}

fun Outline.deleteBackward(viewport: Viewport, key: Long): Edit? {
    val node = this[key] ?: return null
    if (node.children.isNotEmpty() || node.note.isNotEmpty()) return null
    val rows = rows(viewport)
    val at = rows.indexOfFirst { it.key == key }
    if (at < 0) return null
    if (node.title.isEmpty()) return delete(viewport, key)
    val previous = rows.getOrNull(at - 1)
    if (previous == null || previous.depth > rows[at].depth) return null
    if (node.completed || previous.completed) return null
    val joined = previous.title + node.title
    if (joined.length > MAX_TITLE_LENGTH) return null
    return Edit(
        listOf(Change.Rename(previous.key, joined), Change.Delete(key)),
        Caret(previous.key, previous.title.length),
    )
}

fun Outline.delete(viewport: Viewport, key: Long): Edit? {
    val node = this[key] ?: return null
    return Edit(
        listOf(Change.Delete(key, recursive = node.children.isNotEmpty())),
        caretNear(viewport, key, previousFirst = true),
    )
}

fun Outline.indent(viewport: Viewport, key: Long, cursor: Int): Edit? {
    val node = this[key] ?: return null
    val target = shownSibling(viewport, key, -1)?.let { children(node.parent)[it] } ?: return null
    if (depth(target) + height(key) > MAX_DEPTH) return null
    val into = getValue(target)
    return Edit(
        listOfNotNull(
            Change.Move(key, target, into.children.size),
            Change.Collapse(target, false).takeIf { into.collapsed },
        ),
        Caret(key, cursor),
    )
}

fun Outline.outdent(key: Long, cursor: Int): Edit? {
    val parent = this[key]?.parent ?: return null
    return Edit(
        listOf(Change.Move(key, getValue(parent).parent, indexOf(parent) + 1)),
        Caret(key, cursor),
    )
}

fun Outline.moveUp(viewport: Viewport, key: Long, cursor: Int): Edit? =
    moveBy(viewport, key, cursor, -1)

fun Outline.moveDown(viewport: Viewport, key: Long, cursor: Int): Edit? =
    moveBy(viewport, key, cursor, 1)

fun Outline.destinations(viewport: Viewport, key: Long?, query: String): List<Destination> =
    buildList {
        val match = query.trim()
        val levels = key?.let { height(it) } ?: 1
        fun walk(parent: Long?, path: List<String>) {
            for (child in children(parent)) {
                val node = getValue(child)
                if (child == key || !isShown(child, viewport)) continue
                if (path.size + 1 + levels > MAX_DEPTH) continue
                if (node.title.isNotBlank() && node.title.contains(match, ignoreCase = true)) {
                    add(Destination(child, node.title, path))
                }
                walk(child, path + node.title)
            }
        }
        walk(null, emptyList())
    }

fun Outline.moveTo(viewport: Viewport, key: Long, target: Long): Edit? {
    val node = this[key] ?: return null
    val into = this[target] ?: return null
    if (target in subtree(key) || depth(target) + height(key) > MAX_DEPTH) return null
    val index = into.children.size - if (node.parent == target) 1 else 0
    return Edit(
        listOf(Change.Move(key, target, index)),
        caretNear(viewport, key, previousFirst = false),
    )
}

fun Outline.toggleCompleted(viewport: Viewport, key: Long, cursor: Int): Edit? {
    val node = this[key] ?: return null
    val caret =
        if (viewport.showCompleted || node.completed) Caret(key, cursor)
        else caretNear(viewport, key, previousFirst = false)
    return Edit(listOf(Change.Complete(key, !node.completed)), caret)
}

fun Outline.toggleCollapsed(key: Long, cursor: Int): Edit? {
    val node = this[key]?.takeIf { it.children.isNotEmpty() } ?: return null
    return Edit(listOf(Change.Collapse(key, !node.collapsed)), Caret(key, cursor))
}

fun Outline.append(parent: Long?, newKey: Long): Edit =
    Edit(listOf(Change.Create(newKey, parent, children(parent).size, "")), Caret(newKey, 0))

private fun Outline.moveBy(viewport: Viewport, key: Long, cursor: Int, step: Int): Edit? {
    val node = this[key] ?: return null
    val target = shownSibling(viewport, key, step) ?: return null
    return Edit(listOf(Change.Move(key, node.parent, target)), Caret(key, cursor))
}

private fun Outline.caretNear(viewport: Viewport, key: Long, previousFirst: Boolean): Caret? {
    val rows = rows(viewport)
    val at = rows.indexOfFirst { it.key == key }
    val row = rows.getOrNull(at) ?: return null
    val previous = rows.getOrNull(at - 1)
    val next = rows.subList(at + 1, rows.size).firstOrNull { it.depth <= row.depth }
    val target = if (previousFirst) previous ?: next else next ?: previous
    return target?.let { Caret(it.key, it.title.length) }
}

private fun Outline.shownSibling(viewport: Viewport, key: Long, step: Int): Int? {
    val siblings = children(this[key]?.parent)
    val start = siblings.indexOf(key) + step
    val range = if (step < 0) start downTo 0 else start until siblings.size
    return range.firstOrNull { isShown(siblings[it], viewport) }
}

private fun Outline.getValue(key: Long): Node = nodes.getValue(key)

private fun Outline.isShown(key: Long, viewport: Viewport): Boolean =
    viewport.showCompleted || this[key]?.completed == false
