package io.github.yagipass.nestodo.domain

import java.time.Instant
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Test

class CommentsTest {
    private val tokyo = ZoneId.of("Asia/Tokyo")
    private val now = Instant.parse("2026-11-16T05:00:00Z")

    private fun event(at: String) =
        TimelineItem.Event(TimelineItem.Event.Kind.Created, Instant.parse(at))

    private fun comment(id: Long, at: String) =
        TimelineItem.Comment(id, "Called $id", at = Instant.parse(at))

    @Test
    fun `entries are grouped by the phone's calendar day, so a comment written after midnight here is not filed under the server's UTC date`() {
        val days =
            byDay(
                listOf(
                    event("2026-11-14T14:30:00Z"),
                    comment(1, "2026-11-14T15:30:00Z"),
                    comment(2, "2026-11-15T14:59:00Z"),
                    comment(3, "2026-11-15T16:00:00Z"),
                ),
                now,
                tokyo,
            )

        assertEquals(listOf("Nov 14", "Yesterday", "Today"), days.map { it.label })
        assertEquals(listOf(1, 2, 1), days.map { it.entries.size })
    }

    @Test
    fun `days in another year carry the year, so last December is not mistaken for this one`() {
        val days =
            byDay(
                listOf(event("2025-12-31T03:00:00Z"), comment(1, "2026-11-12T03:00:00Z")),
                now,
                tokyo,
            )

        assertEquals(listOf("Dec 31, 2025", "Nov 12"), days.map { it.label })
    }

    @Test
    fun `the day labels are next worked out at the phone's own midnight, so entries from before it stop being labelled Today`() {
        assertEquals(Instant.parse("2026-11-16T15:00:00Z"), nextMidnight(now, tokyo))
    }

    @Test
    fun `times are shown on a 24-hour clock in the phone's time zone, so an afternoon entry never reads as morning`() {
        val days = byDay(listOf(comment(1, "2026-11-16T05:02:00Z")), now, tokyo)

        assertEquals("14:02", days.single().entries.single().time)
    }

    @Test
    fun `entries keep the order the server sent them in, oldest first, so the newest sits next to the composer`() {
        val entries =
            listOf(
                event("2026-11-16T01:00:00Z"),
                comment(1, "2026-11-16T02:00:00Z"),
                Pending(-1, "sending", Instant.parse("2026-11-16T03:00:00Z")),
            )

        assertEquals(entries, byDay(entries, now, tokyo).single().entries.map { it.entry })
    }

    @Test
    fun `a large photo is shrunk until its longer side fits, keeping its shape, so an upload stays small without cropping anything`() {
        assertEquals(Size(2048, 1536), fitWithin(4032, 3024))
        assertEquals(Size(946, 2048), fitWithin(1170, 2532))
    }

    @Test
    fun `a small screenshot is never enlarged, which would only blur it and make it bigger`() {
        assertEquals(Size(800, 600), fitWithin(800, 600))
    }

    @Test
    fun `a very thin image keeps at least one pixel on each side, so it can still be drawn`() {
        assertEquals(Size(2048, 1), fitWithin(10000, 1))
    }

    @Test
    fun `kept pictures drop the oldest once over budget, but never one the open log shows or the one just added`() {
        val kept =
            mapOf("old" to ByteArray(4), "shown" to ByteArray(4), "older" to ByteArray(4))
                .keeping("new", ByteArray(4), budget = 8, shown = setOf("shown"))

        assertEquals(listOf("shown", "new"), kept.keys.toList())
    }

    @Test
    fun `a comment of only pictures is described by how many it has, as the web says it`() {
        assertEquals(listOf("1 image", "2 images"), listOf(imageCount(1), imageCount(2)))
    }
}
