package io.github.yagipass.nestodo.ui

import io.github.yagipass.nestodo.CommentsState
import io.github.yagipass.nestodo.domain.Pending
import io.github.yagipass.nestodo.domain.TimelineItem
import java.time.Instant
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Test

class CommentSheetTest {
    private val now = Instant.parse("2026-11-16T05:00:00Z")
    private val utc = ZoneId.of("UTC")
    private val created =
        TimelineItem.Event(TimelineItem.Event.Kind.Created, Instant.parse("2026-11-16T02:00:00Z"))

    private fun kinds(comments: CommentsState) =
        logLines(comments, now, utc).map {
            when (it) {
                is LogLine.Date -> "date ${it.label}"
                is LogLine.Item -> it.stamped.entry::class.simpleName
                LogLine.Unwritten -> "unwritten"
                LogLine.Unloaded -> "unloaded"
            }
        }

    @Test
    fun `a hardware keyboard posts with Shift+Enter, and with Ctrl or Cmd+Enter for those used to it from other apps`() {
        assertEquals(
            listOf(true, true, true),
            listOf(
                isPostKey(enter = true, shift = true, ctrl = false, meta = false, alt = false),
                isPostKey(enter = true, shift = false, ctrl = true, meta = false, alt = false),
                isPostKey(enter = true, shift = false, ctrl = false, meta = true, alt = false),
            ),
        )
    }

    @Test
    fun `Enter alone is left to the field, so a comment typed on a hardware keyboard can have more than one line`() {
        assertEquals(
            false,
            isPostKey(enter = true, shift = false, ctrl = false, meta = false, alt = false),
        )
    }

    @Test
    fun `a log with only its events invites the member to write under them, so an empty log does not look broken`() {
        assertEquals(
            listOf("date Today", "Event", "unwritten"),
            kinds(CommentsState(4, done = false, items = listOf(created))),
        )
    }

    @Test
    fun `a comment still being sent already counts as written, so the invitation does not sit under it`() {
        val sending = Pending(-1, "Call at 9", Instant.parse("2026-11-16T03:00:00Z"))
        assertEquals(
            listOf("date Today", "Event", "Pending"),
            kinds(CommentsState(4, done = false, items = listOf(created), local = listOf(sending))),
        )
    }

    @Test
    fun `a log that could not load says so instead of looking empty, so the member can try again`() {
        assertEquals(listOf("unloaded"), kinds(CommentsState(4, done = false, failed = true)))
    }

    @Test
    fun `nothing is shown while the log loads, so the invitation never flashes before the comments arrive`() {
        assertEquals(emptyList<String>(), kinds(CommentsState(4, done = false)))
    }
}
