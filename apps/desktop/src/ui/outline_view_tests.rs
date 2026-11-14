use std::sync::Arc;
use std::time::Duration;

use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};

use super::*;
use crate::ui::fake_backend::{FakeServer, seed};
use crate::ui::theme::Theme;

struct Shell {
    outline: Entity<OutlineView>,
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tools = self
            .outline
            .update(cx, |outline, cx| outline.render_tools(window, cx));
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(tools)
            .child(self.outline.clone())
    }
}

fn open<'a>(
    cx: &'a mut TestAppContext,
    server: &Arc<FakeServer>,
    show_done: bool,
) -> (Entity<OutlineView>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        cx.set_global(Theme::light());
        cx.bind_keys(crate::ui::text_field::bindings());
        cx.bind_keys(crate::ui::move_picker::bindings());
        cx.bind_keys(bindings());
    });
    let todos = server.list().unwrap();
    let backend: Arc<dyn Backend> = server.clone();
    let (shell, cx) = cx.add_window_view(|window, cx| Shell {
        outline: cx.new(|cx| OutlineView::new(backend, &todos, show_done, window, cx)),
    });
    let outline = shell.read_with(cx, |shell, _| shell.outline.clone());
    (outline, cx)
}

fn key_of(view: &Entity<OutlineView>, cx: &mut VisualTestContext, title: &str) -> Key {
    view.read_with(cx, |view, _| {
        visible_rows(&view.outline, true, "", &HashSet::new())
            .into_iter()
            .find(|row| view.outline.item(&row.key).title == title)
            .map_or_else(|| panic!("no row titled {title:?}"), |row| row.key)
    })
}

fn focus(view: &Entity<OutlineView>, cx: &mut VisualTestContext, title: &str, pos: usize) {
    let key = key_of(view, cx, title);
    view.update_in(cx, |view, window, cx| {
        view.focus_row(
            Focus {
                key,
                pos,
                note: false,
            },
            window,
            cx,
        );
    });
    cx.run_until_parked();
}

fn focused(
    view: &Entity<OutlineView>,
    cx: &mut VisualTestContext,
) -> Option<(String, usize, bool)> {
    view.read_with(cx, |view, cx| {
        view.focused(cx).map(|focus| {
            (
                view.outline.item(&focus.key).title.clone(),
                focus.pos,
                focus.note,
            )
        })
    })
}

fn type_text(view: &Entity<OutlineView>, cx: &mut VisualTestContext, text: &str) {
    let field = view.read_with(cx, |view, _| {
        if view.editing.as_ref().is_some_and(|editing| editing.note) {
            view.note_field.clone()
        } else {
            view.title_field.clone()
        }
    });
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, text, window, cx);
    });
    cx.run_until_parked();
}

fn settle(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
}

fn titles(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, _| {
        view.rows()
            .into_iter()
            .map(|row| {
                format!(
                    "{}{}",
                    "  ".repeat(row.depth),
                    view.outline.item(&row.key).title
                )
            })
            .collect()
    })
}

fn change_elsewhere(
    server: &FakeServer,
    method: &'static str,
    path: &str,
    body: Option<serde_json::Value>,
) {
    server
        .request(&nestodo_desktop::wire::Request {
            method,
            path: path.into(),
            body,
        })
        .unwrap();
}

fn half_bought() -> Arc<FakeServer> {
    Arc::new(FakeServer::with(vec![
        seed("Groceries"),
        seed("Milk").under(1).done(),
        seed("Bread").under(1),
        seed("Trip"),
    ]))
}

fn groceries() -> Arc<FakeServer> {
    Arc::new(FakeServer::with(vec![
        seed("Groceries"),
        seed("Milk").under(1),
        seed("Bread").under(1),
        seed("Trip"),
    ]))
}

#[gpui::test]
fn typing_reaches_the_server_only_after_a_half_second_pause_so_each_keystroke_is_not_a_request(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", END);

    cx.simulate_input("oat");
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(server.sent().len(), 1);
    assert_eq!(server.tree(), "Groceries\n  Milkoat\n  Bread\nTrip");
}

#[gpui::test]
fn enter_splits_the_title_at_the_caret_and_keeps_typing_on_the_new_row(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 2);

    cx.simulate_keystrokes("enter");
    cx.simulate_input("X");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Br\n  Xead\nTrip");
    assert_eq!(focused(&view, cx), Some(("Xead".into(), 1, false)));
}

#[gpui::test]
fn enter_at_the_end_of_an_open_parent_starts_its_first_child(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Groceries", END);

    cx.simulate_keystrokes("enter");
    cx.simulate_input("Eggs");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Eggs\n  Milk\n  Bread\nTrip");
}

#[gpui::test]
fn tab_nests_under_the_row_above_and_shift_tab_puts_it_back_after_its_parent(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 1);

    cx.simulate_keystrokes("tab");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n    Bread\nTrip");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 1, false)));

    cx.simulate_keystrokes("shift-tab shift-tab");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\nBread\nTrip");
}

#[gpui::test]
fn cmd_enter_completes_and_moves_on_when_completed_rows_are_hidden(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, false);
    focus(&view, cx, "Milk", 0);

    cx.simulate_keystrokes("cmd-enter");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk [done]\n  Bread\nTrip");
    assert_eq!(titles(&view, cx), ["Groceries", "  Bread", "Trip"]);
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5, false)));
}

#[gpui::test]
fn backspace_at_the_start_joins_the_row_onto_the_one_above(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("backspace");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  MilkBread\nTrip");
    assert_eq!(focused(&view, cx), Some(("MilkBread".into(), 4, false)));
}

#[gpui::test]
fn backspace_inside_the_title_only_deletes_a_character(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 2);

    cx.simulate_keystrokes("backspace");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bead\nTrip");
}

#[gpui::test]
fn backspace_removes_an_emoji_whole_but_takes_a_separate_voiced_mark_off_its_kana_as_a_browser_does(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip\u{2764}\u{FE0F}"),
        seed("Code\u{1F469}\u{200D}\u{1F4BB}"),
        seed("\u{304B}\u{3099}"),
    ]));
    let (view, cx) = open(cx, &server, true);

    for title in [
        "Trip\u{2764}\u{FE0F}",
        "Code\u{1F469}\u{200D}\u{1F4BB}",
        "\u{304B}\u{3099}",
    ] {
        focus(&view, cx, title, END);
        cx.simulate_keystrokes("backspace");
    }
    settle(cx);

    assert_eq!(server.tree(), "Trip\nCode\n\u{304B}");
}

#[gpui::test]
fn backspace_in_an_empty_row_deletes_it_and_moves_to_the_end_of_the_row_above(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", END);

    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\n  \nTrip");

    cx.simulate_keystrokes("backspace");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5, false)));
}

#[gpui::test]
fn cmd_z_takes_back_a_split_on_the_server_and_puts_the_caret_back(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 2);

    cx.simulate_keystrokes("enter");
    settle(cx);
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 2, false)));
}

#[gpui::test]
fn cmd_z_takes_back_a_burst_of_typing_in_one_step(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);

    cx.simulate_input("ToKyoto");
    settle(cx);
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
}

#[gpui::test]
fn up_and_down_move_between_rows_keeping_the_caret_column(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 3);

    cx.simulate_keystrokes("up");
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 3, false)));
    cx.simulate_keystrokes("up");
    assert_eq!(focused(&view, cx), Some(("Groceries".into(), 3, false)));
    cx.simulate_keystrokes("ctrl-n ctrl-n ctrl-n");
    assert_eq!(focused(&view, cx), Some(("Trip".into(), 3, false)));
    cx.simulate_keystrokes("down");
    assert_eq!(focused(&view, cx), Some(("Trip".into(), 4, false)));
}

#[gpui::test]
fn up_and_down_keep_the_caret_column_in_characters_across_japanese_and_ascii(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("牛乳を買う"), seed("buy milk")]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "牛乳を買う", "牛乳".len());

    cx.simulate_keystrokes("down");

    assert_eq!(focused(&view, cx), Some(("buy milk".into(), 2, false)));
}

fn long_list() -> Arc<FakeServer> {
    let titles: Vec<String> = (1..=40).map(|n| format!("Row {n}")).collect();
    Arc::new(FakeServer::with(
        titles.iter().map(|title| seed(title)).collect(),
    ))
}

fn on_screen(
    view: &Entity<OutlineView>,
    cx: &mut VisualTestContext,
    selector: &'static str,
) -> bool {
    let viewport = view.read_with(cx, |view, _| view.scroll.bounds());
    cx.debug_bounds(selector)
        .is_some_and(|title| viewport.top() <= title.top() && title.bottom() <= viewport.bottom())
}

fn press(cx: &mut VisualTestContext, keystroke: &str, times: usize) {
    cx.simulate_keystrokes(&vec![keystroke; times].join(" "));
}

#[gpui::test]
fn moving_the_caret_past_the_window_edge_scrolls_its_row_into_view_so_typing_is_never_blind(
    cx: &mut TestAppContext,
) {
    let server = long_list();
    let (view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    focus(&view, cx, "Row 1", 0);
    assert!(!on_screen(&view, cx, "title-t30"));

    press(cx, "ctrl-n", 29);
    assert_eq!(focused(&view, cx), Some(("Row 30".into(), 0, false)));
    assert!(on_screen(&view, cx, "title-t30"));

    press(cx, "ctrl-p", 27);
    assert_eq!(focused(&view, cx), Some(("Row 3".into(), 0, false)));
    assert!(on_screen(&view, cx, "title-t3"));
}

#[gpui::test]
fn the_filter_banner_above_the_rows_does_not_shift_which_row_is_scrolled_into_view(
    cx: &mut TestAppContext,
) {
    let server = long_list();
    let (view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    view.update_in(cx, OutlineView::open_filter);
    cx.simulate_input("Row");
    focus(&view, cx, "Row 1", 0);

    press(cx, "ctrl-n", 29);

    assert_eq!(focused(&view, cx), Some(("Row 30".into(), 0, false)));
    assert!(on_screen(&view, cx, "title-t30"));
}

#[gpui::test]
fn shift_enter_writes_a_note_where_enter_adds_a_line_and_escape_returns_to_the_title(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("shift-enter");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 0, true)));
    type_text(&view, cx, "near the station");
    cx.simulate_keystrokes("enter");
    type_text(&view, cx, "opens at 7");
    cx.simulate_keystrokes("escape");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread [note: near the station\\nopens at 7]\nTrip"
    );
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5, false)));
    assert_eq!(view.read_with(cx, |view, _| view.note_key.clone()), None);
}

#[gpui::test]
fn undoing_a_note_on_a_hidden_row_does_not_leave_an_open_note_editor_that_would_drop_what_is_typed_into_it(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Rent"), seed("Paid").done()]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Paid", 0);
    cx.simulate_keystrokes("shift-enter");
    type_text(&view, cx, "abc");
    cx.simulate_keystrokes("escape");
    settle(cx);

    view.update_in(cx, OutlineView::toggle_show_done);
    cx.simulate_keystrokes("cmd-z");
    view.update_in(cx, OutlineView::toggle_show_done);
    settle(cx);

    assert_eq!(server.tree(), "Rent\nPaid [done]");
    assert_eq!(view.read_with(cx, |view, _| view.note_key.clone()), None);
}

#[gpui::test]
fn a_note_left_with_only_spaces_is_cleared_so_the_row_does_not_show_an_empty_note(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("shift-enter");
    type_text(&view, cx, "   ");
    cx.simulate_keystrokes("shift-enter");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    let key = key_of(&view, cx, "Bread");
    view.read_with(cx, |view, _| assert_eq!(view.outline.item(&key).note, ""));
}

#[gpui::test]
fn up_at_the_start_of_a_note_goes_back_to_its_title(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Bread").note("near the station"),
        seed("Trip"),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);
    cx.simulate_keystrokes("shift-enter");

    cx.simulate_keystrokes("down");
    assert_eq!(focused(&view, cx), Some(("Trip".into(), 0, false)));

    focus(&view, cx, "Bread", 0);
    cx.simulate_keystrokes("shift-enter cmd-up up");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5, false)));
}

#[gpui::test]
fn cmd_shift_m_moves_the_row_under_the_chosen_todo(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 0);

    cx.simulate_keystrokes("cmd-shift-m");
    assert!(view.read_with(cx, |view, _| view.picker.is_some()));
    cx.simulate_input("trip");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Bread\nTrip\n  Milk");
    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5, false)));
}

#[gpui::test]
fn escape_closes_the_move_picker_and_returns_to_where_the_caret_was(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 2);

    cx.simulate_keystrokes("cmd-shift-m escape");

    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2, false)));
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
}

#[gpui::test]
fn clicking_outside_the_move_picker_closes_it_and_returns_to_where_the_caret_was(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 2);
    cx.simulate_keystrokes("cmd-shift-m");

    let outside = gpui::point(px(3.), px(3.));
    cx.simulate_mouse_move(outside, None, gpui::Modifiers::none());
    cx.simulate_click(outside, gpui::Modifiers::none());

    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2, false)));
}

#[gpui::test]
fn picking_a_destination_another_device_deleted_while_the_picker_was_open_leaves_the_row_where_it_was_instead_of_crashing(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 0);

    cx.simulate_keystrokes("cmd-shift-m");
    change_elsewhere(&server, "DELETE", "/api/v1/todos/4", None);
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();
    cx.simulate_input("trip");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread");
    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 0, false)));
}

#[gpui::test]
fn cmd_z_in_the_move_picker_leaves_the_outline_behind_it_alone(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.simulate_input(" to Kyoto");
    settle(cx);

    cx.simulate_keystrokes("cmd-shift-m");
    cx.simulate_input("gro");
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip to Kyoto");
    assert!(view.read_with(cx, |view, _| view.picker.is_some()));
    cx.simulate_keystrokes("escape");
    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
}

#[gpui::test]
fn deleting_a_row_with_children_asks_first_and_keeps_it_when_cancelled(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Groceries", 0);

    cx.simulate_keystrokes("cmd-shift-backspace");
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");

    cx.simulate_keystrokes("cmd-shift-backspace");
    cx.simulate_prompt_answer("Delete all");
    settle(cx);
    assert_eq!(server.tree(), "Trip");
}

#[gpui::test]
fn marking_a_todo_done_with_open_todos_under_it_asks_first_and_cancelling_leaves_everything_and_the_caret_as_it_was(
    cx: &mut TestAppContext,
) {
    let server = half_bought();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Groceries", 3);

    cx.simulate_keystrokes("cmd-enter");
    assert_eq!(
        cx.pending_prompt(),
        Some((
            "Mark \u{201c}Groceries\u{201d} as done?".into(),
            "1 item under it isn\u{2019}t done yet. It\u{2019}ll be marked as done too.".into()
        ))
    );
    cx.simulate_prompt_answer("Cancel");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk [done]\n  Bread\nTrip");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
    assert_eq!(focused(&view, cx), Some(("Groceries".into(), 3, false)));

    cx.simulate_keystrokes("cmd-enter");
    cx.simulate_prompt_answer("Mark all as done");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries [done]\n  Milk [done]\n  Bread [done]\nTrip"
    );
    assert_eq!(server.sent().len(), 1);
}

#[gpui::test]
fn the_check_counts_every_open_todo_under_the_row_even_deeper_ones_hidden_in_a_collapsed_row_so_nothing_out_of_sight_is_completed_without_asking(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip").collapsed(),
        seed("Tickets").under(1).done(),
        seed("Packing").under(1),
        seed("Socks").under(3),
    ]));
    let (view, cx) = open(cx, &server, false);
    let trip = key_of(&view, cx, "Trip");

    view.update_in(cx, |view, window, cx| view.check_row(&trip, window, cx));
    assert_eq!(
        cx.pending_prompt(),
        Some((
            "Mark \u{201c}Trip\u{201d} as done?".into(),
            "2 items under it aren\u{2019}t done yet. They\u{2019}ll be marked as done too.".into()
        ))
    );
    cx.simulate_prompt_answer("Mark all as done");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Trip [done] [collapsed]\n  Tickets [done]\n  Packing [done]\n    Socks [done]"
    );
}

#[gpui::test]
fn marking_done_asks_nothing_when_everything_under_it_is_done_and_reopening_never_asks_and_keeps_the_todos_under_it_done(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip"),
        seed("Tickets").under(1).done(),
        seed("Home"),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", 0);

    cx.simulate_keystrokes("cmd-enter");
    assert!(!cx.has_pending_prompt());
    settle(cx);
    assert_eq!(server.tree(), "Trip [done]\n  Tickets [done]\nHome");

    cx.simulate_keystrokes("cmd-enter");
    assert!(!cx.has_pending_prompt());
    settle(cx);
    assert_eq!(server.tree(), "Trip\n  Tickets [done]\nHome");
}

#[gpui::test]
fn reopening_never_asks_even_a_todo_completed_before_completing_took_the_todos_under_it_along(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip").done(),
        seed("Visa").under(1),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", 0);

    cx.simulate_keystrokes("cmd-enter");
    assert!(!cx.has_pending_prompt());
    settle(cx);

    assert_eq!(server.tree(), "Trip\n  Visa");
}

#[gpui::test]
fn cmd_z_after_marking_a_todo_done_reopens_only_what_that_completed(cx: &mut TestAppContext) {
    let server = half_bought();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Groceries", 0);
    cx.simulate_keystrokes("cmd-enter");
    cx.simulate_prompt_answer("Mark all as done");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries [done]\n  Milk [done]\n  Bread [done]\nTrip"
    );
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();

    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk [done]\n  Bread\nTrip");
}

#[gpui::test]
fn adding_a_row_under_a_done_todo_reopens_every_done_todo_above_it_and_cmd_z_removes_the_row_and_completes_them_again(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip").done(),
        seed("Packing").under(1).done(),
        seed("Socks").under(2).done(),
        seed("Home"),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Socks", END);

    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Trip\n  Packing\n    Socks [done]\n    \nHome"
    );
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();

    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Trip [done]\n  Packing [done]\n    Socks [done]\nHome"
    );
}

#[gpui::test]
fn marking_done_asks_nothing_while_a_failed_save_pauses_editing_since_the_answer_could_not_be_saved(
    cx: &mut TestAppContext,
) {
    let server = half_bought();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", 0);
    server.fail_with(500);
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert!(view.read_with(cx, |view, _| view.sync.failed));

    focus(&view, cx, "Groceries", 0);
    cx.simulate_keystrokes("cmd-enter");

    assert!(!cx.has_pending_prompt());
}

#[gpui::test]
fn a_failed_save_pauses_editing_so_later_edits_are_not_applied_on_a_stale_outline(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);
    server.fail_with(500);

    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert!(view.read_with(cx, |view, _| view.sync.failed));

    let sent = server.sent().len();
    cx.simulate_keystrokes("tab");
    cx.simulate_input("x");
    cx.simulate_keystrokes("up");
    cx.simulate_input("x");
    settle(cx);
    assert_eq!(server.sent().len(), sent);
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milk", "  Bread", "Trip"]
    );
}

#[gpui::test]
fn enter_while_an_ime_composition_is_open_confirms_it_instead_of_splitting(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    let field = view.read_with(cx, |view, _| view.title_field.clone());
    field.update_in(cx, |field, window, cx| {
        field.replace_and_mark_text_in_range(None, "きょうと", None, window, cx);
    });

    cx.simulate_keystrokes("enter");
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "京都", window, cx);
    });
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip京都");
}

#[gpui::test]
fn an_ime_conversion_is_taken_back_in_one_undo_step(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    let field = view.read_with(cx, |view, _| view.title_field.clone());
    for composing in ["k", "き", "きょ", "きょう", "きょうと"] {
        field.update_in(cx, |field, window, cx| {
            field.replace_and_mark_text_in_range(None, composing, None, window, cx);
        });
        cx.executor().advance_clock(Duration::from_secs(2));
    }
    field.update_in(cx, |field, window, cx| {
        field.replace_text_in_range(None, "京都", window, cx);
    });
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip京都");

    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
}

#[gpui::test]
fn slash_opens_the_filter_only_outside_a_title_so_it_can_still_be_typed(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);

    cx.simulate_input("/");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip/");
    assert!(!view.read_with(cx, |view, _| view.filtering));

    view.update_in(cx, super::OutlineView::blur_editor);
    cx.simulate_input("/");
    assert!(view.read_with(cx, |view, _| view.filtering));
    cx.simulate_input("bre");
    assert_eq!(titles(&view, cx), ["Groceries", "  Bread"]);

    cx.simulate_keystrokes("escape");
    assert!(!view.read_with(cx, |view, _| view.filtering));
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milk", "  Bread", "Trip/"]
    );
}

#[gpui::test]
fn a_row_added_during_a_filter_stays_visible_until_the_query_changes(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    view.update_in(cx, super::OutlineView::open_filter);
    cx.simulate_input("bread");
    focus(&view, cx, "Bread", END);

    cx.simulate_keystrokes("enter");
    cx.simulate_input("Jam");
    settle(cx);
    assert_eq!(titles(&view, cx), ["Groceries", "  Bread", "  Jam"]);

    view.update_in(cx, super::OutlineView::open_filter);
    cx.simulate_keystrokes("backspace");

    assert_eq!(titles(&view, cx), ["Groceries", "  Bread"]);
}

#[gpui::test]
fn cmd_up_and_cmd_down_collapse_and_expand_the_row(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Groceries", 0);

    cx.simulate_keystrokes("cmd-up");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries [collapsed]\n  Milk\n  Bread\nTrip"
    );
    assert_eq!(titles(&view, cx), ["Groceries", "Trip"]);

    cx.simulate_keystrokes("cmd-down");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
}

#[gpui::test]
fn alt_shift_down_moves_the_row_below_its_next_sibling(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 1);

    cx.simulate_keystrokes("alt-shift-down");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Bread\n  Milk\nTrip");

    cx.simulate_keystrokes("cmd-shift-up");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 1, false)));
}

#[gpui::test]
fn a_refresh_picks_up_changes_made_on_another_device_but_never_while_edits_are_unsaved(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    change_elsewhere(
        &server,
        "PATCH",
        "/api/v1/todos/4",
        Some(serde_json::json!({ "title": "Trip to Kyoto" })),
    );

    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milk", "  Bread", "Trip to Kyoto"]
    );

    focus(&view, cx, "Milk", END);
    cx.simulate_input("s");
    change_elsewhere(
        &server,
        "PATCH",
        "/api/v1/todos/3",
        Some(serde_json::json!({ "title": "Rye" })),
    );
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milks", "  Bread", "Trip to Kyoto"]
    );
}

#[gpui::test]
fn clicks_and_typing_that_reach_a_row_another_device_deleted_before_the_next_frame_are_ignored_instead_of_crashing(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    let trip = key_of(&view, cx, "Trip");

    view.update_in(cx, |view, window, cx| {
        view.outline = remove(&view.outline, &trip).outline;
        view.check_row(&trip, window, cx);
        view.toggle_row(&trip, window, cx);
        view.title_field.update(cx, |field, cx| {
            field.replace_text_in_range(None, "s", window, cx);
        });
    });
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn a_refresh_that_finds_the_sign_in_expired_asks_for_the_sign_in_screen_before_an_edit_can_fail_on_a_stale_outline(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    let signed_out = Rc::new(std::cell::Cell::new(false));
    let seen = signed_out.clone();
    cx.update(|_, cx| {
        cx.subscribe(&view, move |_, event, _| {
            if matches!(event, OutlineEvent::SignedOut) {
                seen.set(true);
            }
        })
        .detach();
    });

    server.expire_sign_in();
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();

    assert!(signed_out.get());
}

#[gpui::test]
fn switching_to_another_app_and_back_keeps_the_title_being_edited_so_typing_carries_on_without_a_click(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    focus(&view, cx, "Trip", END);

    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.simulate_input(" to Kyoto");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip to Kyoto");
}

#[gpui::test]
fn a_rename_made_on_another_device_while_away_keeps_the_caret_after_the_same_text_so_typing_does_not_land_mid_word(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    focus(&view, cx, "Milk", END);

    cx.deactivate_window();
    change_elsewhere(
        &server,
        "PATCH",
        "/api/v1/todos/2",
        Some(serde_json::json!({ "title": "Oat milk" })),
    );
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.simulate_input(" 2L");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Oat milk 2L\n  Bread\nTrip");
}

#[gpui::test]
fn adding_a_row_right_after_typing_saves_the_typing_first_so_a_failed_add_cannot_take_it_down_too(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.simulate_input(" to Kyoto");
    server.fail_creates_with(500);

    view.update_in(cx, super::OutlineView::add_item);
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip to Kyoto");
}

#[gpui::test]
fn enter_right_after_typing_saves_the_typing_before_the_new_row_so_a_failed_add_cannot_take_it_down_too(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.simulate_input(" to Kyoto");
    server.fail_creates_with(500);

    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip to Kyoto");
}

#[gpui::test]
fn switching_to_another_app_with_the_empty_filter_open_keeps_it_open_so_a_search_term_copied_there_can_be_typed_on_return(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    view.update_in(cx, super::OutlineView::open_filter);
    cx.run_until_parked();

    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.simulate_input("milk");

    assert_eq!(titles(&view, cx), ["Groceries", "  Milk"]);
}

#[gpui::test]
fn clicking_a_letter_puts_the_caret_on_its_nearer_side_so_typing_lands_where_the_user_aimed(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk"),
        seed("Home"),
        seed("Tea"),
    ]));
    let (_, cx) = open(cx, &server, true);
    let glyph = 15. * 0.6;
    let at = |cx: &mut VisualTestContext, selector: &'static str, glyphs: f32| {
        let title = cx.debug_bounds(selector).expect("the title was drawn");
        gpui::point(
            title.origin.x + px(4. + glyph * glyphs),
            title.origin.y + title.size.height / 2.,
        )
    };

    let right_half_of_i = at(cx, "title-t1", 1.75);
    cx.simulate_click(right_half_of_i, gpui::Modifiers::none());
    cx.simulate_input("s");
    let left_half_of_m = at(cx, "title-t2", 2.25);
    cx.simulate_click(left_half_of_m, gpui::Modifiers::none());
    cx.simulate_input("x");
    let right_half_of_the_last_letter = at(cx, "title-t3", 2.9);
    cx.simulate_click(right_half_of_the_last_letter, gpui::Modifiers::none());
    cx.simulate_input("s");
    settle(cx);

    assert_eq!(server.tree(), "Mislk\nHoxme\nTeas");
}

#[gpui::test]
fn a_click_on_the_first_letter_of_a_link_after_text_opens_the_link_like_the_rest_of_it(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("See https://example.com")]));
    let (_, cx) = open(cx, &server, true);
    let title = cx.debug_bounds("title-t1").expect("the title was drawn");
    let glyph = 15. * 0.6;
    let at = gpui::point(
        title.origin.x + px(4. + glyph * 4.5),
        title.origin.y + title.size.height / 2.,
    );

    cx.simulate_click(at, gpui::Modifiers::none());

    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/"));
}

#[gpui::test]
fn a_pasted_url_with_japanese_is_encoded_so_it_stays_one_link(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
        " https://ja.wikipedia.org/wiki/京都\n".to_string(),
    ));

    cx.simulate_keystrokes("space cmd-v");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread\nTrip https://ja.wikipedia.org/wiki/%E4%BA%AC%E9%83%BD"
    );
}

#[gpui::test]
fn enter_on_a_parent_whose_children_are_all_hidden_as_completed_adds_a_sibling_the_user_can_see(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip"),
        seed("Tickets").under(1).done(),
        seed("Home"),
    ]));
    let (view, cx) = open(cx, &server, false);
    focus(&view, cx, "Trip", END);

    cx.simulate_keystrokes("enter");
    cx.simulate_input("Hotel");
    settle(cx);

    assert_eq!(server.tree(), "Trip\n  Tickets [done]\nHotel\nHome");
    assert_eq!(titles(&view, cx), ["Trip", "Hotel", "Home"]);
}

#[gpui::test]
fn ctrl_space_opens_a_collapsed_row_to_show_what_is_under_it(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Archive").collapsed(),
        seed("Old item").under(1),
    ]));
    let (view, cx) = open(cx, &server, true);
    assert_eq!(titles(&view, cx), ["Archive"]);
    focus(&view, cx, "Archive", 0);

    cx.simulate_keystrokes("ctrl-space");
    settle(cx);

    assert_eq!(server.tree(), "Archive\n  Old item");
    assert_eq!(titles(&view, cx), ["Archive", "  Old item"]);
}
