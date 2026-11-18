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

    private fun posts(
        shift: Boolean = false,
        ctrl: Boolean = false,
        meta: Boolean = false,
        hardware: Boolean = true,
    ) = isPostKey(enter = true, shift, ctrl, meta, alt = false, hardware)

    @Test
    fun `a hardware keyboard posts with Enter, and with Ctrl or Cmd+Enter for those used to it from other apps`() {
        assertEquals(
            listOf(true, true, true),
            listOf(posts(), posts(ctrl = true), posts(meta = true)),
        )
    }

    @Test
    fun `Shift+Enter is left to the field, so a comment typed on a hardware keyboard can have more than one line`() {
        assertEquals(false, posts(shift = true))
    }

    @Test
    fun `Enter from the on-screen keyboard is left to the field, since it is the only key that starts a new line there`() {
        assertEquals(false, posts(hardware = false))
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
    fun `a log that fails to refresh keeps what it showed and says so at its newest end, so the member knows it may be out of date and can try again`() {
        assertEquals(
            listOf("date Today", "Event", "unwritten", "unloaded"),
            kinds(CommentsState(4, done = false, items = listOf(created), failed = true)),
        )
    }

    @Test
    fun `a comment being sent shows before the log has loaded, so the text just posted does not seem to vanish`() {
        val sending = Pending(-1, "Call at 9", Instant.parse("2026-11-16T03:00:00Z"))
        assertEquals(
            listOf("date Today", "Pending"),
            kinds(CommentsState(4, done = false, local = listOf(sending))),
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
