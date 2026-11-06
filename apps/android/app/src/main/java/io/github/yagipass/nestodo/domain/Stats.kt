package io.github.yagipass.nestodo.domain

import kotlin.math.ceil
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.pow
import kotlinx.serialization.Serializable

@Serializable data class Day(val date: String, val added: Int, val completed: Int, val open: Int)

val RANGES = listOf(7, 30, 90)

data class Summary(
    val since: String,
    val completed: Int,
    val completedBefore: Int,
    val added: Int,
    val addedBefore: Int,
    val open: Int,
    val openBefore: Int,
)

fun summarize(days: List<Day>, range: Int): Summary {
    val current = days.takeLast(range)
    val previous = days.dropLast(range).takeLast(range)
    return Summary(
        since = current.first().date,
        completed = current.sumOf { it.completed },
        completedBefore = previous.sumOf { it.completed },
        added = current.sumOf { it.added },
        addedBefore = previous.sumOf { it.added },
        open = current.last().open,
        openBefore = previous.lastOrNull()?.open ?: current.first().open,
    )
}

fun ticks(max: Int, count: Int = 4): List<Int> {
    val rough = maxOf(1, max).toDouble() / count
    val magnitude = 10.0.pow(floor(log10(rough)))
    val step =
        maxOf(
                1.0,
                listOf(1, 2, 5, 10).map { it * magnitude }.firstOrNull { it >= rough }
                    ?: (10 * magnitude),
            )
            .toInt()
    val top = maxOf(step, ceil(max.toDouble() / step).toInt() * step)
    return (0..top step step).toList()
}

fun heatLevel(count: Int, busiest: Int): Int =
    if (count <= 0 || busiest <= 0) 0 else ceil(count.toDouble() / busiest * 4).toInt()
