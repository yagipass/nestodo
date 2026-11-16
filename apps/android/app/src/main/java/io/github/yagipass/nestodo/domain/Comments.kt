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
    ) : TimelineItem
}

@Serializable
enum class Via {
    @SerialName("app") App,
    @SerialName("mcp") Mcp,
}

data class Pending(val key: Long, val body: String, override val at: Instant) : Entry

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
