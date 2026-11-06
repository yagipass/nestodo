package io.github.yagipass.nestodo.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class StatsTest {
    private val days =
        List(182) { index ->
            Day(
                date = "day-$index",
                added = 1,
                completed = if (index >= 175) 2 else 1,
                open = index,
            )
        }

    @Test
    fun `a period is compared with the one right before it`() {
        val summary = summarize(days, 7)
        assertEquals(14, summary.completed)
        assertEquals(7, summary.completedBefore)
        assertEquals(7, summary.added)
        assertEquals(7, summary.addedBefore)
    }

    @Test
    fun `the open count is compared with the day before the period began`() {
        val summary = summarize(days, 30)
        assertEquals(181, summary.open)
        assertEquals(151, summary.openBefore)
        assertEquals("day-152", summary.since)
    }

    @Test
    fun `the scale always reaches the busiest day so no column is drawn past the top of the chart`() {
        for (max in listOf(0, 1, 3, 7, 12, 48, 130)) {
            val scale = ticks(max)
            assertEquals(0, scale.first())
            assertTrue("scale for $max is $scale", scale.last() >= max)
        }
    }

    @Test
    fun `the scale uses whole, evenly spaced steps because counts are whole numbers`() {
        assertEquals(listOf(0, 2, 4, 6, 8), ticks(7))
        assertEquals(listOf(0, 1, 2), ticks(2))
        assertEquals(listOf(0, 50, 100, 150), ticks(130))
    }

    @Test
    fun `a day is shaded by its share of the busiest day and quiet days stay blank`() {
        assertEquals(0, heatLevel(0, 8))
        assertEquals(1, heatLevel(1, 8))
        assertEquals(4, heatLevel(8, 8))
        assertEquals(0, heatLevel(0, 0))
    }
}
