package io.github.yagipass.nestodo.domain

import java.text.BreakIterator

const val MAX_ICON_LENGTH = 32

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

fun isIcon(value: String): Boolean {
    if (value.isEmpty() || value.length > MAX_ICON_LENGTH) return false
    val clusters = BreakIterator.getCharacterInstance().apply { setText(value) }
    clusters.first()
    if (clusters.next() != value.length) return false
    if (!Character.isEmoji(value.codePointAt(0))) return false
    return value.codePoints().anyMatch(Character::isEmojiPresentation) || '\uFE0F' in value
}

fun shownIcon(icon: String?): String? = icon?.takeUnless { TypedIcon.containsMatchIn(it) }
