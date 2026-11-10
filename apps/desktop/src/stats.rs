use serde::Deserialize;

pub const RANGES: [usize; 3] = [7, 30, 90];

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Day {
    pub date: String,
    pub added: u32,
    pub completed: u32,
    pub open: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub since: String,
    pub completed: u32,
    pub completed_before: u32,
    pub added: u32,
    pub added_before: u32,
    pub open: u32,
    pub open_before: u32,
}

#[must_use]
pub fn total_added(days: &[Day]) -> u32 {
    days.iter().map(|day| day.added).sum()
}

#[must_use]
pub fn total_completed(days: &[Day]) -> u32 {
    days.iter().map(|day| day.completed).sum()
}

#[must_use]
pub fn summarize(days: &[Day], range: usize) -> Summary {
    let len = days.len();
    let current = &days[len.saturating_sub(range)..];
    let previous = &days[len.saturating_sub(2 * range)..len.saturating_sub(range)];
    let first = current.first();
    Summary {
        since: first.map(|day| day.date.clone()).unwrap_or_default(),
        completed: total_completed(current),
        completed_before: total_completed(previous),
        added: total_added(current),
        added_before: total_added(previous),
        open: current.last().map_or(0, |day| day.open),
        open_before: previous.last().or(first).map_or(0, |day| day.open),
    }
}

#[must_use]
pub fn ticks(max: u32) -> Vec<u32> {
    const COUNT: u64 = 4;
    let total = u64::from(max.max(1));
    let step = if total < COUNT {
        1
    } else {
        let mut magnitude = 1;
        while magnitude * 10 * COUNT <= total {
            magnitude *= 10;
        }
        [1, 2, 5, 10]
            .into_iter()
            .map(|m| m * magnitude)
            .find(|step| step * COUNT >= total)
            .unwrap_or(10 * magnitude)
    };
    let top = step.max(u64::from(max).div_ceil(step) * step);
    (0..=top / step)
        .filter_map(|i| u32::try_from(i * step).ok())
        .collect()
}

#[must_use]
pub fn heat_level(count: u32, busiest: u32) -> u8 {
    if count == 0 || busiest == 0 {
        return 0;
    }
    let level = (u64::from(count) * 4).div_ceil(u64::from(busiest));
    u8::try_from(level).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAYS: usize = 182;

    mod summarize {
        use super::*;

        fn days() -> Vec<Day> {
            (0..DAYS)
                .map(|index| Day {
                    date: format!("day-{index}"),
                    added: if index >= DAYS - 7 { 3 } else { 1 },
                    completed: if index >= DAYS - 7 { 2 } else { 1 },
                    open: u32::try_from(index).unwrap(),
                })
                .collect()
        }

        #[test]
        fn compares_the_chosen_period_with_the_one_right_before_it() {
            let summary = summarize(&days(), 7);
            assert_eq!(
                (
                    summary.completed,
                    summary.completed_before,
                    summary.added,
                    summary.added_before
                ),
                (14, 7, 21, 7)
            );
        }

        #[test]
        fn reports_how_the_open_count_moved_since_the_day_before_the_period_began() {
            let summary = summarize(&days(), 30);
            let days = u32::try_from(DAYS).unwrap();
            assert_eq!(summary.open, days - 1);
            assert_eq!(summary.open_before, days - 31);
            assert_eq!(summary.since, format!("day-{}", DAYS - 30));
        }

        #[test]
        fn summarizes_no_history_as_zeros_instead_of_bringing_down_the_window_with_unsaved_edits_in_it()
         {
            let summary = summarize(&[], 7);
            assert_eq!(summary.since, "");
            assert_eq!(
                (
                    summary.completed,
                    summary.added,
                    summary.open,
                    summary.open_before
                ),
                (0, 0, 0, 0)
            );
        }
    }

    mod ticks {
        use super::*;

        #[test]
        fn always_reaches_the_busiest_day_so_no_column_is_drawn_past_the_top_of_the_chart() {
            for max in [0, 1, 3, 7, 12, 48, 130] {
                let scale = ticks(max);
                assert_eq!(scale.first(), Some(&0), "{max}");
                assert!(scale.last().is_some_and(|&top| top >= max), "{max}");
            }
        }

        #[test]
        fn uses_whole_evenly_spaced_steps_because_counts_are_whole_numbers() {
            assert_eq!(ticks(7), [0, 2, 4, 6, 8]);
            assert_eq!(ticks(2), [0, 1, 2]);
            assert_eq!(ticks(130), [0, 50, 100, 150]);
        }
    }

    mod heat_level {
        use super::*;

        #[test]
        fn shades_a_day_by_its_share_of_the_busiest_day_leaving_quiet_days_blank() {
            assert_eq!(heat_level(0, 8), 0);
            assert_eq!(heat_level(1, 8), 1);
            assert_eq!(heat_level(5, 16), 2);
            assert_eq!(heat_level(9, 16), 3);
            assert_eq!(heat_level(8, 8), 4);
            assert_eq!(heat_level(0, 0), 0);
        }
    }
}
