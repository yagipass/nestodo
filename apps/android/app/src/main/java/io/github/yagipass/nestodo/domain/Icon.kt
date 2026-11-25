package io.github.yagipass.nestodo.domain

import java.text.BreakIterator
import java.text.Normalizer
import kotlinx.serialization.Serializable

const val MAX_ICON_LENGTH = 32

const val MAX_ICON_NAME_LENGTH = 30

const val ICON_IMAGE_SIZE = 128

val IconChoices =
    """
        🏠 🧹 🧺 🧽 🗑️ 🪴 🔧 💡
        🛒 🥛 🍳 🧻 🛍️ 🎁 👟 🎂
        📅 🏥 🦷 💊 ✂️ 📞 🚗 ✈️
        📚 ✏️ 📝 🎒 💼 💻 📮 🧾
        💰 🏛️ 🐈 🐕 ⭐ ❤️ 🎉 ⚠️
    """
        .trim()
        .split(Regex("\\s+"))

private val TypedIcon = Regex("^[a-z][a-z0-9_-]*:")

private val Extension = Regex("\\.[^.]*$")

fun isIcon(value: String): Boolean {
    if (value.isEmpty() || value.length > MAX_ICON_LENGTH) return false
    val clusters = BreakIterator.getCharacterInstance().apply { setText(value) }
    clusters.first()
    if (clusters.next() != value.length) return false
    if (!Character.isEmoji(value.codePointAt(0))) return false
    return value.codePoints().anyMatch(Character::isEmojiPresentation) || '️' in value
}

@Serializable
data class ImageIcon(val id: String, val name: String, val url: String) {
    val value: String
        get() = "image:$id"
}

class MyIcons(
    val list: List<ImageIcon> = emptyList(),
    val images: Map<String, ByteArray> = emptyMap(),
) {
    val missing: List<ImageIcon>
        get() = list.filter { it.id !in images }

    operator fun get(id: String): ImageIcon? = list.firstOrNull { it.id == id }

    fun listing(icons: List<ImageIcon>): MyIcons =
        MyIcons(icons, images.filterKeys { id -> icons.any { it.id == id } })

    fun adding(icon: ImageIcon, image: ByteArray): MyIcons =
        MyIcons(list.filterNot { it.id == icon.id } + icon, images + (icon.id to image))

    fun showing(id: String, image: ByteArray): MyIcons =
        if (this[id] == null) this else MyIcons(list, images + (id to image))

    fun renaming(icon: ImageIcon): MyIcons =
        MyIcons(list.map { if (it.id == icon.id) icon else it }, images)

    fun removing(id: String): MyIcons = MyIcons(list.filterNot { it.id == id }, images - id)

    override fun equals(other: Any?): Boolean =
        other is MyIcons && other.list == list && other.images.keys == images.keys

    override fun hashCode(): Int = 31 * list.hashCode() + images.keys.hashCode()
}

sealed interface ShownIcon {
    data class Emoji(val text: String) : ShownIcon

    data class Image(val icon: ImageIcon) : ShownIcon
}

fun shownIcon(icon: String?, images: List<ImageIcon>): ShownIcon? {
    if (icon == null) return null
    if (!TypedIcon.containsMatchIn(icon)) return ShownIcon.Emoji(icon)
    return images.firstOrNull { it.value == icon }?.let(ShownIcon::Image)
}

fun iconName(fileName: String): String {
    val name = Normalizer.normalize(fileName.replace(Extension, ""), Normalizer.Form.NFC).trim()
    var end = 0
    while (end < name.length) {
        val next = name.offsetByCodePoints(end, 1)
        if (next > MAX_ICON_NAME_LENGTH) break
        end = next
    }
    return name.substring(0, end).trim()
}

fun Outline.usesOf(icon: String): Int = nodes.values.count { it.icon == icon }

fun Outline.withoutIcon(icon: String): Outline =
    copy(
        nodes =
            nodes.mapValues { (_, node) -> if (node.icon == icon) node.copy(icon = null) else node }
    )
