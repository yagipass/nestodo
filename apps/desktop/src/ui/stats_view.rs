use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, PathBuilder, Pixels, SharedString, Window, canvas, div,
    point, prelude::*, px,
};

use nestodo_desktop::stats::{
    Day, RANGES, heat_level, summarize, ticks, total_added, total_completed,
};

use crate::ui::backend::Backend;
use crate::ui::theme::{FIGTREE, Theme, theme};
use crate::ui::widgets::{PAGE_WIDTH, sheet_shadow};

const WEEKDAY_LABELS: [&str; 7] = ["", "Mon", "", "Wed", "", "Fri", ""];
const TICK_GUTTER: f32 = 28.;

enum Load {
    Loading,
    Failed(String),
    Ready(Vec<Day>),
}

pub struct StatsView {
    backend: Arc<dyn Backend>,
    load: Load,
    range: usize,
    calendar_hover: Option<usize>,
    chart_hover: Option<usize>,
    show_table: bool,
}

fn date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn format_date(value: &str, pattern: &str) -> String {
    date(value).map_or_else(
        || value.to_string(),
        |date| date.format(pattern).to_string(),
    )
}

fn short_date(value: &str) -> String {
    format_date(value, "%b %-d")
}

fn long_date(value: &str) -> String {
    format_date(value, "%a, %b %-d")
}

fn signed(value: i64) -> String {
    match value {
        value if value > 0 => format!("+{value}"),
        value if value < 0 => format!("\u{2212}{}", -value),
        _ => "\u{00b1}0".to_string(),
    }
}

fn local_time_zone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

impl StatsView {
    pub fn new(backend: Arc<dyn Backend>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            backend,
            load: Load::Loading,
            range: 30,
            calendar_hover: None,
            chart_hover: None,
            show_table: false,
        };
        view.reload(cx);
        view
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let zone = local_time_zone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { backend.stats(&zone) })
                .await;
            this.update(cx, |this, cx| {
                this.load = match result {
                    Ok(response) => Load::Ready(response.days),
                    Err(error) => Load::Failed(error.to_string()),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn card(colors: &Theme) -> gpui::Div {
        div()
            .px(px(20.))
            .py(px(18.))
            .border_1()
            .border_color(colors.border)
            .rounded(px(16.))
            .bg(colors.surface)
            .shadow(sheet_shadow(colors))
    }

    fn render_calendar(&self, days: &[Day], width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let blanks = days
            .first()
            .and_then(|day| date(&day.date))
            .map_or(0, |first| first.weekday().num_days_from_sunday() as usize);
        let weeks = (blanks + days.len()).div_ceil(7);
        let gap = px(3.);
        let inner = width - px(32.) - px(2.);
        let cell = ((inner - px(TICK_GUTTER) - gap * weeks as f32) / weeks as f32).max(px(4.));
        let activity = |day: &Day| day.added + day.completed;
        let busiest = days.iter().map(activity).max().unwrap_or(0);
        let column_x = |column: usize| px(TICK_GUTTER) + (cell + gap) * column as f32;
        let months = days
            .iter()
            .enumerate()
            .filter(|(_, day)| day.date.ends_with("-01"))
            .map(|(index, day)| {
                let column = (blanks + index) / 7;
                div()
                    .absolute()
                    .left(column_x(column))
                    .top_0()
                    .text_size(px(11.))
                    .text_color(colors.muted)
                    .whitespace_nowrap()
                    .child(format_date(&day.date, "%b"))
            });
        let labels = WEEKDAY_LABELS.iter().enumerate().map(|(row, label)| {
            div()
                .absolute()
                .left_0()
                .top(px(18.) + (cell + gap) * row as f32)
                .h(cell)
                .flex()
                .items_center()
                .text_size(px(10.))
                .text_color(colors.muted)
                .child(label.to_string())
        });
        let hovered = self.calendar_hover;
        let cells = days.iter().enumerate().map(|(index, day)| {
            let slot = blanks + index;
            let level = heat_level(activity(day), busiest);
            div()
                .id(("cell", index))
                .absolute()
                .left(column_x(slot / 7))
                .top(px(18.) + (cell + gap) * (slot % 7) as f32)
                .size(cell)
                .rounded(px(4.))
                .bg(colors.heat_color(level))
                .when(hovered == Some(index), |cell| {
                    cell.border_2().border_color(colors.text)
                })
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    if *hovering {
                        this.calendar_hover = Some(index);
                    } else if this.calendar_hover == Some(index) {
                        this.calendar_hover = None;
                    }
                    cx.notify();
                }))
        });
        let height = px(18.) + (cell + gap) * 7.;
        let foot = match hovered.and_then(|index| days.get(index)) {
            Some(day) => format!(
                "{} added \u{00b7} {} completed \u{00b7} {}",
                day.added,
                day.completed,
                long_date(&day.date)
            ),
            None => format!(
                "{} added \u{00b7} {} completed in the past 26 weeks",
                total_added(days),
                total_completed(days)
            ),
        };
        Self::card(&colors)
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(FontWeight::BOLD)
                    .child("Past 26 weeks"),
            )
            .child(
                div()
                    .mt(px(2.))
                    .mb(px(20.))
                    .text_size(px(13.))
                    .text_color(colors.muted)
                    .child("Todos added and completed each day"),
            )
            .child(
                div()
                    .relative()
                    .h(height)
                    .children(months)
                    .children(labels)
                    .children(cells),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .mt(px(12.))
                    .text_size(px(13.))
                    .child(foot)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(11.))
                            .text_color(colors.muted)
                            .child("Less")
                            .children((0..=4).map(|level| {
                                div()
                                    .size(px(11.))
                                    .rounded(px(3.))
                                    .bg(colors.heat_color(level))
                            }))
                            .child("More"),
                    ),
            )
            .into_any_element()
    }

    fn render_chart(&self, days: &[Day], width: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let plot_width = width - px(32.) - px(2.) - px(TICK_GUTTER);
        let count = days.len().max(1);
        let scale = ticks(
            days.iter()
                .map(|day| day.added.max(day.completed))
                .max()
                .unwrap_or(0),
        );
        let top = *scale.last().unwrap_or(&1) as f32;
        let open_scale = ticks(days.iter().map(|day| day.open).max().unwrap_or(0));
        let open_top = *open_scale.last().unwrap_or(&1) as f32;
        let at = |index: usize| (index as f32 + 0.5) / count as f32;
        let hover = self.chart_hover.filter(|index| *index < days.len());
        let added = total_added(days);
        let completed = total_completed(days);
        let mirrored_height = px(200.);
        let open_height = px(140.);
        let half = mirrored_height / 2.;

        let mut values: Vec<i64> = scale
            .iter()
            .skip(1)
            .rev()
            .map(|value| -i64::from(*value))
            .collect();
        values.extend(scale.iter().map(|value| i64::from(*value)));
        let gridlines = values.into_iter().map(|value| {
            let y = half - half * (value as f32 / top);
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(y)
                .h(px(1.))
                .bg(if value == 0 {
                    colors.border
                } else {
                    colors.grid
                })
                .child(
                    div()
                        .absolute()
                        .top(px(-7.))
                        .left(px(-TICK_GUTTER))
                        .w(px(22.))
                        .flex()
                        .justify_end()
                        .text_size(px(11.))
                        .line_height(px(11.))
                        .text_color(colors.muted)
                        .child(value.abs().to_string()),
                )
        });
        let dense = days.len() > 30;
        let radius = if dense { px(2.) } else { px(4.) };
        let columns = days.iter().enumerate().map(|(index, day)| {
            let active = hover == Some(index);
            let bar_width = (plot_width / count as f32 - px(2.))
                .min(px(24.))
                .max(px(1.));
            div()
                .id(("column", index))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .flex_col()
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    if *hovering {
                        this.chart_hover = Some(index);
                    } else if this.chart_hover == Some(index) {
                        this.chart_hover = None;
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .h(half)
                        .flex()
                        .justify_center()
                        .items_end()
                        .pb(px(1.))
                        .child(
                            div()
                                .w(bar_width)
                                .h((half - px(1.)) * (day.added as f32 / top))
                                .rounded_t(radius)
                                .bg(if active {
                                    colors.added_strong
                                } else {
                                    colors.added
                                }),
                        ),
                )
                .child(
                    div()
                        .h(half)
                        .flex()
                        .justify_center()
                        .items_start()
                        .pt(px(1.))
                        .child(
                            div()
                                .w(bar_width)
                                .h((half - px(1.)) * (day.completed as f32 / top))
                                .rounded_b(radius)
                                .bg(if active {
                                    colors.accent_strong
                                } else {
                                    colors.accent
                                }),
                        ),
                )
        });
        let tip = hover.map(|index| {
            let day = &days[index];
            let x = plot_width * at(index);
            let bar = (half - px(1.)) * (day.added as f32 / top);
            edge_aligned(
                x,
                plot_width,
                div()
                    .flex()
                    .flex_col()
                    .px(px(8.))
                    .py(px(6.))
                    .border_1()
                    .border_color(colors.border)
                    .rounded(px(10.))
                    .bg(colors.surface)
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .whitespace_nowrap()
                    .child(div().font_weight(FontWeight::BOLD).child(format!(
                        "{} added \u{00b7} {} completed",
                        day.added, day.completed
                    )))
                    .child(div().text_color(colors.muted).child(long_date(&day.date))),
                160.,
            )
            .bottom(half + bar + px(10.))
        });

        let open_gridlines = open_scale.iter().map(|value| {
            let y = open_height - open_height * (*value as f32 / open_top);
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(y)
                .h(px(1.))
                .bg(colors.grid)
                .child(
                    div()
                        .absolute()
                        .top(px(-7.))
                        .left(px(-TICK_GUTTER))
                        .w(px(22.))
                        .flex()
                        .justify_end()
                        .text_size(px(11.))
                        .line_height(px(11.))
                        .text_color(colors.muted)
                        .child(value.to_string()),
                )
        });
        let levels: Vec<f32> = days.iter().map(|day| day.open as f32 / open_top).collect();
        let line_color = colors.accent;
        let mut area_color = colors.accent;
        area_color.a = 0.12;
        let points = {
            let levels = levels.clone();
            move |bounds: gpui::Bounds<Pixels>| -> Vec<gpui::Point<Pixels>> {
                levels
                    .iter()
                    .enumerate()
                    .map(|(index, level)| {
                        point(
                            bounds.left()
                                + bounds.size.width
                                    * ((index as f32 + 0.5) / levels.len().max(1) as f32),
                            bounds.bottom() - bounds.size.height * *level,
                        )
                    })
                    .collect()
            }
        };
        let line = canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                let points = points(bounds);
                if points.is_empty() {
                    return;
                }
                let mut area = PathBuilder::fill();
                area.move_to(point(points[0].x, bounds.bottom()));
                for at in &points {
                    area.line_to(*at);
                }
                area.line_to(point(points[points.len() - 1].x, bounds.bottom()));
                area.close();
                if let Ok(path) = area.build() {
                    window.paint_path(path, area_color);
                }
                let mut stroke = PathBuilder::stroke(px(2.));
                stroke.move_to(points[0]);
                for at in &points[1..] {
                    stroke.line_to(*at);
                }
                if let Ok(path) = stroke.build() {
                    window.paint_path(path, line_color);
                }
            },
        )
        .absolute()
        .size_full();
        let focus = hover.unwrap_or(days.len().saturating_sub(1));
        let focus_x = plot_width * at(focus);
        let focus_level = levels.get(focus).copied().unwrap_or(0.);
        let dot_y = open_height - open_height * focus_level;
        let hits = days.iter().enumerate().map(|(index, _)| {
            div()
                .id(("hit", index))
                .absolute()
                .top_0()
                .bottom_0()
                .left(plot_width * (index as f32 / count as f32))
                .w(plot_width / count as f32)
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    if *hovering {
                        this.chart_hover = Some(index);
                    } else if this.chart_hover == Some(index) {
                        this.chart_hover = None;
                    }
                    cx.notify();
                }))
        });
        let every = if days.len() <= 7 {
            1
        } else if days.len() <= 30 {
            7
        } else {
            14
        };
        let mut axis_indexes: Vec<usize> = (0..days.len()).rev().step_by(every).collect();
        axis_indexes.reverse();
        let axis = axis_indexes.into_iter().map(|index| {
            let text = if days.len() <= 7 {
                format_date(&days[index].date, "%a")
            } else {
                short_date(&days[index].date)
            };
            edge_aligned(
                plot_width * at(index),
                plot_width,
                div()
                    .text_size(px(11.))
                    .text_color(colors.muted)
                    .whitespace_nowrap()
                    .child(text),
                50.,
            )
            .top_0()
        });

        let table = self.show_table.then(|| {
            let header = |text: &'static str, first: bool| {
                div()
                    .flex_1()
                    .flex()
                    .when(!first, gpui::Styled::justify_end)
                    .text_color(colors.muted)
                    .font_weight(FontWeight::BOLD)
                    .child(text)
            };
            div()
                .id("table")
                .mt(px(8.))
                .max_h(px(240.))
                .overflow_y_scroll()
                .border_1()
                .border_color(colors.border)
                .rounded(px(10.))
                .text_size(px(13.))
                .child(
                    div()
                        .flex()
                        .px(px(10.))
                        .py(px(6.))
                        .child(header("Day", true))
                        .child(header("Added", false))
                        .child(header("Completed", false))
                        .child(header("Open", false)),
                )
                .children(days.iter().rev().map(|day| {
                    let cell =
                        |value: u32| div().flex_1().flex().justify_end().child(value.to_string());
                    div()
                        .flex()
                        .px(px(10.))
                        .py(px(6.))
                        .border_t_1()
                        .border_color(colors.border)
                        .child(div().flex_1().child(long_date(&day.date)))
                        .child(cell(day.added))
                        .child(cell(day.completed))
                        .child(cell(day.open))
                }))
        });

        let legend_item = |color: Hsla, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(10.)).rounded(px(3.)).bg(color))
                .child(label)
        };

        Self::card(&colors)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap_x(px(16.))
                    .child(
                        div()
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Added and completed"),
                            )
                            .child(
                                div()
                                    .mt(px(2.))
                                    .mb(px(20.))
                                    .text_size(px(13.))
                                    .text_color(colors.muted)
                                    .child(format!(
                                        "{added} added and {completed} completed in the last {} days",
                                        days.len()
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .mb(px(16.))
                            .text_size(px(11.))
                            .text_color(colors.muted)
                            .child(legend_item(colors.added, "Added \u{2191}"))
                            .child(legend_item(colors.accent, "Completed \u{2193}")),
                    ),
            )
            .child(
                div()
                    .id("mirrored")
                    .relative()
                    .h(mirrored_height)
                    .ml(px(TICK_GUTTER))
                    .children(gridlines)
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .flex()
                            .gap(px(2.))
                            .children(columns),
                    )
                    .children(tip),
            )
            .child(
                div()
                    .mt(px(20.))
                    .mb(px(8.))
                    .ml(px(TICK_GUTTER))
                    .text_size(px(12.))
                    .text_color(colors.muted)
                    .child("Open at the end of each day"),
            )
            .child(
                div()
                    .relative()
                    .h(open_height)
                    .ml(px(TICK_GUTTER))
                    .children(open_gridlines)
                    .child(line)
                    .when_some(hover, |plot, index| {
                        plot.child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(plot_width * at(index))
                                .w(px(1.))
                                .bg(colors.muted),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .left(focus_x - px(8.))
                            .top(dot_y - px(8.))
                            .size(px(16.))
                            .rounded_full()
                            .border(px(3.))
                            .border_color(colors.surface)
                            .bg(colors.accent),
                    )
                    .child(
                        edge_aligned(
                            focus_x,
                            plot_width,
                            div()
                                .text_size(px(12.))
                                .font_weight(FontWeight::BOLD)
                                .whitespace_nowrap()
                                .child(format!(
                                    "{} open",
                                    days.get(focus).map_or(0, |day| day.open)
                                )),
                            60.,
                        )
                        .top(dot_y - px(28.)),
                    )
                    .children(hits),
            )
            .child(div().relative().h(px(16.)).mt(px(6.)).ml(px(TICK_GUTTER)).children(axis))
            .child(
                div()
                    .id("table-toggle")
                    .mt(px(14.))
                    .text_size(px(13.))
                    .text_color(colors.accent)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_table = !this.show_table;
                        cx.notify();
                    }))
                    .child(if self.show_table {
                        "\u{25be} Show as table"
                    } else {
                        "\u{25b8} Show as table"
                    }),
            )
            .children(table)
            .into_any_element()
    }
}

fn edge_aligned(x: Pixels, width: Pixels, content: gpui::Div, approx: f32) -> gpui::Div {
    let percent = if width > px(0.) {
        x / width * 100.
    } else {
        50.
    };
    let left = if percent < 10. {
        x
    } else if percent > 90. {
        x - px(approx)
    } else {
        x - px(approx / 2.)
    };
    div()
        .absolute()
        .left(left)
        .w(px(approx))
        .flex()
        .when(percent < 10., gpui::Styled::justify_start)
        .when(percent > 90., gpui::Styled::justify_end)
        .when((10.0..=90.0).contains(&percent), |wrap| {
            wrap.justify_center()
        })
        .child(content)
}

impl Render for StatsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx);
        let width = (window.viewport_size().width - px(32.)).min(px(PAGE_WIDTH));
        let body: AnyElement =
            match &self.load {
                Load::Loading => div()
                    .text_size(px(14.))
                    .text_color(colors.muted)
                    .child("Loading\u{2026}")
                    .into_any_element(),
                Load::Failed(message) => div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(14.))
                    .text_color(colors.danger)
                    .child(SharedString::from(format!(
                        "Couldn\u{2019}t load the stats: {message}"
                    )))
                    .child(
                        div()
                            .id("retry-stats")
                            .text_color(colors.accent)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load = Load::Loading;
                                this.reload(cx);
                                cx.notify();
                            }))
                            .child("Try again"),
                    )
                    .into_any_element(),
                Load::Ready(days) if days.is_empty() => div().into_any_element(),
                Load::Ready(days) => {
                    let days = days.clone();
                    let range = self.range.min(days.len());
                    let shown = days[days.len() - range..].to_vec();
                    let totals = summarize(&days, range);
                    let period = format!("{range} days");
                    let tiles = [
                        (
                            "Completed",
                            totals.completed.to_string(),
                            format!(
                                "{} vs previous {period}",
                                signed(
                                    i64::from(totals.completed)
                                        - i64::from(totals.completed_before)
                                )
                            ),
                        ),
                        (
                            "Added",
                            totals.added.to_string(),
                            format!(
                                "{} vs previous {period}",
                                signed(i64::from(totals.added) - i64::from(totals.added_before))
                            ),
                        ),
                        (
                            "Open now",
                            totals.open.to_string(),
                            format!(
                                "{} since {}",
                                signed(i64::from(totals.open) - i64::from(totals.open_before)),
                                short_date(&totals.since)
                            ),
                        ),
                        (
                            "Per day",
                            format!("{:.1}", totals.completed as f32 / range.max(1) as f32),
                            "completed on average".to_string(),
                        ),
                    ];
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .child(self.render_calendar(&days, width, cx))
                        .child(div().flex().flex_wrap().gap(px(8.)).pt(px(8.)).children(
                            RANGES.iter().map(|value| {
                                let value = *value;
                                let pressed = value == self.range;
                                div()
                                    .id(("range", value))
                                    .h(px(34.))
                                    .px(px(14.))
                                    .flex()
                                    .items_center()
                                    .border_1()
                                    .border_color(if pressed {
                                        colors.accent
                                    } else {
                                        colors.border
                                    })
                                    .rounded(px(10.))
                                    .bg(if pressed {
                                        colors.accent_soft
                                    } else {
                                        colors.surface
                                    })
                                    .when(pressed, |pill| pill.font_weight(FontWeight::SEMIBOLD))
                                    .text_size(px(14.))
                                    .text_color(if pressed { colors.accent } else { colors.muted })
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.range = value;
                                        this.chart_hover = None;
                                        cx.notify();
                                    }))
                                    .child(format!("{value} days"))
                            }),
                        ))
                        .child(div().flex().flex_wrap().gap(px(12.)).children(
                            tiles.into_iter().map(|(label, value, note)| {
                                div()
                                    .flex_1()
                                    .min_w(px(150.))
                                    .px(px(14.))
                                    .py(px(12.))
                                    .border_1()
                                    .border_color(colors.border)
                                    .rounded(px(12.))
                                    .bg(colors.surface)
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .text_color(colors.muted)
                                            .child(label),
                                    )
                                    .child(
                                        div()
                                            .mt(px(2.))
                                            .font_family(FIGTREE)
                                            .text_size(px(26.))
                                            .font_weight(FontWeight::BOLD)
                                            .line_height(px(32.))
                                            .child(value),
                                    )
                                    .child(
                                        div()
                                            .mt(px(2.))
                                            .text_size(px(12.))
                                            .text_color(colors.muted)
                                            .child(note),
                                    )
                            }),
                        ))
                        .child(self.render_chart(&shown, width, cx))
                        .into_any_element()
                }
            };
        div()
            .id("stats-scroll")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .px(px(16.))
            .pb(px(24.))
            .text_color(colors.text)
            .child(
                div()
                    .w_full()
                    .max_w(px(PAGE_WIDTH))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(36.))
                            .child("Stats"),
                    )
                    .child(body),
            )
    }
}
