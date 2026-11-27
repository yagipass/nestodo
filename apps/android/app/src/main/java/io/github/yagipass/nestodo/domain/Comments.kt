package io.github.yagipass.nestodo.domain

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException
import java.util.Locale
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

const val MAX_COMMENT_IMAGES = 4

const val COMMENT_IMAGE_EDGE = 2048

const val MAX_COMMENT_IMAGE_BYTES = 2 * 1024 * 1024

@Serializable data class CommentImage(val id: String, val url: String)

class Attachment(val key: Long, val bytes: ByteArray, val image: CommentImage? = null) {
    val uploading: Boolean
        get() = image == null

    fun uploaded(image: CommentImage): Attachment = Attachment(key, bytes, image)

    override fun equals(other: Any?): Boolean =
        other is Attachment && other.key == key && other.image == image

    override fun hashCode(): Int = 31 * key.hashCode() + image.hashCode()
}

data class Size(val width: Int, val height: Int)

fun fitWithin(width: Int, height: Int, edge: Int = COMMENT_IMAGE_EDGE): Size {
    val scale = minOf(1.0, edge.toDouble() / maxOf(width, height))
    return Size(
        maxOf(1, Math.round(width * scale).toInt()),
        maxOf(1, Math.round(height * scale).toInt()),
    )
}

fun imageCount(count: Int): String = if (count == 1) "1 image" else "$count images"

fun Map<String, ByteArray>.keeping(
    id: String,
    bytes: ByteArray,
    budget: Int,
    shown: Set<String>,
): Map<String, ByteArray> {
    val kept = LinkedHashMap(this)
    kept.remove(id)
    kept[id] = bytes
    var total = kept.values.sumOf { it.size }
    val iterator = kept.entries.iterator()
    while (total > budget && iterator.hasNext()) {
        val (key, value) = iterator.next()
        if (key == id || key in shown) continue
        iterator.remove()
        total -= value.size
    }
    return kept
}

sealed interface Entry {
    val at: Instant
}

@Serializable
sealed interface TimelineItem : Entry {
    @Serializable
    @SerialName("event")
    data class Event(
        val kind: Kind,
        @Serializable(with = InstantText::class) override val at: Instant,
    ) : TimelineItem {
        @Serializable
        enum class Kind {
            @SerialName("created") Created,
            @SerialName("completed") Completed,
            @SerialName("reopened") Reopened,
        }
    }

    @Serializable
    @SerialName("comment")
    data class Comment(
        val id: Long,
        val body: String,
        val via: Via = Via.App,
        @Serializable(with = InstantText::class) override val at: Instant,
        @Serializable(with = InstantText::class) val editedAt: Instant? = null,
        val images: List<CommentImage> = emptyList(),
    ) : TimelineItem
}

@Serializable
enum class Via {
    @SerialName("app") App,
    @SerialName("mcp") Mcp,
}

data class Pending(
    val key: Long,
    val body: String,
    override val at: Instant,
    val images: List<Attachment> = emptyList(),
) : Entry

data class Stamped(val entry: Entry, val time: String)

data class LogDay(val label: String, val entries: List<Stamped>)

fun byDay(entries: List<Entry>, now: Instant, zone: ZoneId): List<LogDay> {
    val today = LocalDate.ofInstant(now, zone)
    val days = mutableListOf<Pair<LocalDate, MutableList<Stamped>>>()
    for (entry in entries) {
        val at = entry.at.atZone(zone)
        val date = at.toLocalDate()
        if (days.lastOrNull()?.first != date) days += date to mutableListOf()
        days.last().second += Stamped(entry, CLOCK.format(at))
    }
    return days.map { (date, stamped) -> LogDay(dayLabel(date, today), stamped) }
}

fun nextMidnight(now: Instant, zone: ZoneId): Instant =
    LocalDate.ofInstant(now, zone).plusDays(1).atStartOfDay(zone).toInstant()

private fun dayLabel(date: LocalDate, today: LocalDate): String =
    when {
        date == today -> "Today"
        date == today.minusDays(1) -> "Yesterday"
        date.year == today.year -> THIS_YEAR.format(date)
        else -> OTHER_YEAR.format(date)
    }

private val CLOCK = DateTimeFormatter.ofPattern("HH:mm", Locale.US)
private val THIS_YEAR = DateTimeFormatter.ofPattern("MMM d", Locale.US)
private val OTHER_YEAR = DateTimeFormatter.ofPattern("MMM d, yyyy", Locale.US)

internal object InstantText : KSerializer<Instant> {
    override val descriptor = PrimitiveSerialDescriptor("Instant", PrimitiveKind.STRING)

    override fun serialize(encoder: Encoder, value: Instant) =
        encoder.encodeString(value.toString())

    override fun deserialize(decoder: Decoder): Instant {
        val text = decoder.decodeString()
        return try {
            Instant.parse(text)
        } catch (_: DateTimeParseException) {
            throw SerializationException("$text is not a UTC time")
        }
    }
}
