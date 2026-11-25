use std::sync::Arc;
use std::time::Duration;

use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};

use nestodo_desktop::thread::{Entry, TimelineItem};

use super::*;
use crate::ui::fake_backend::{FakeServer, TempFile, png, seed};
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
        cx.bind_keys(crate::ui::icon_picker::bindings());
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
        visible_rows(&view.outline, true)
            .into_iter()
            .find(|row| view.outline.item(&row.key).title == title)
            .map_or_else(|| panic!("no row titled {title:?}"), |row| row.key)
    })
}

fn focus(view: &Entity<OutlineView>, cx: &mut VisualTestContext, title: &str, pos: usize) {
    let key = key_of(view, cx, title);
    view.update_in(cx, |view, window, cx| {
        view.focus_row(Focus { key, pos }, window, cx);
    });
    cx.run_until_parked();
}

fn focused(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Option<(String, usize)> {
    view.read_with(cx, |view, cx| {
        view.focused(cx)
            .map(|focus| (view.outline.item(&focus.key).title.clone(), focus.pos))
    })
}

fn settle(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
}

fn matches(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, _| {
        view.hits(&view.rows())
            .into_iter()
            .map(|key| view.outline.item(&key).title.clone())
            .collect()
    })
}

fn current_match(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |view, _| {
        view.current_match(&view.hits(&view.rows()))
            .map(|key| view.outline.item(&key).title.clone())
    })
}

fn searching(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |view, _| view.searching)
}

fn search_focused(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> bool {
    let field = view.read_with(cx, |view, _| view.search_field.clone());
    cx.update(|window, cx| field.focus_handle(cx).is_focused(window))
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
    assert_eq!(focused(&view, cx), Some(("Xead".into(), 1)));
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
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 1)));

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
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5)));
}

#[gpui::test]
fn backspace_at_the_start_joins_the_row_onto_the_one_above(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("backspace");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  MilkBread\nTrip");
    assert_eq!(focused(&view, cx), Some(("MilkBread".into(), 4)));
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
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5)));
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
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 2)));
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
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 3)));
    cx.simulate_keystrokes("up");
    assert_eq!(focused(&view, cx), Some(("Groceries".into(), 3)));
    cx.simulate_keystrokes("ctrl-n ctrl-n ctrl-n");
    assert_eq!(focused(&view, cx), Some(("Trip".into(), 3)));
    cx.simulate_keystrokes("down");
    assert_eq!(focused(&view, cx), Some(("Trip".into(), 4)));
}

#[gpui::test]
fn up_and_down_keep_the_caret_column_in_characters_across_japanese_and_ascii(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("牛乳を買う"), seed("buy milk")]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "牛乳を買う", "牛乳".len());

    cx.simulate_keystrokes("down");

    assert_eq!(focused(&view, cx), Some(("buy milk".into(), 2)));
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
    assert_eq!(focused(&view, cx), Some(("Row 30".into(), 0)));
    assert!(on_screen(&view, cx, "title-t30"));

    press(cx, "ctrl-p", 27);
    assert_eq!(focused(&view, cx), Some(("Row 3".into(), 0)));
    assert!(on_screen(&view, cx, "title-t3"));
}

#[gpui::test]
fn enter_in_the_search_steps_through_the_matches_and_scrolls_each_into_view_while_every_row_stays_in_place(
    cx: &mut TestAppContext,
) {
    let server = long_list();
    let (view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(600.), px(400.)));
    view.update_in(cx, OutlineView::open_search);
    cx.simulate_input("row 4");
    assert_eq!(titles(&view, cx).len(), 40);
    assert_eq!(matches(&view, cx), ["Row 4", "Row 40"]);
    assert_eq!(current_match(&view, cx), Some("Row 4".into()));

    cx.simulate_keystrokes("enter");
    assert_eq!(current_match(&view, cx), Some("Row 40".into()));
    assert!(on_screen(&view, cx, "title-t40"));
    assert!(search_focused(&view, cx));

    cx.simulate_keystrokes("enter");
    assert_eq!(current_match(&view, cx), Some("Row 4".into()));
    assert!(on_screen(&view, cx, "title-t4"));

    cx.simulate_keystrokes("shift-enter");
    assert_eq!(current_match(&view, cx), Some("Row 40".into()));

    click_on(cx, "previous-match");
    assert_eq!(current_match(&view, cx), Some("Row 4".into()));
    assert!(search_focused(&view, cx));
}

#[gpui::test]
fn escape_closes_the_search_and_puts_the_caret_on_the_current_match_so_what_was_found_can_be_edited(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    view.update_in(cx, OutlineView::open_search);
    cx.simulate_input("EAD");
    assert_eq!(current_match(&view, cx), Some("Bread".into()));

    cx.simulate_keystrokes("escape");

    assert!(!searching(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 2)));
    assert_eq!(matches(&view, cx), [] as [String; 0]);
}

#[gpui::test]
fn reopening_the_search_selects_what_was_searched_so_the_next_term_replaces_it(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    view.update_in(cx, OutlineView::open_search);
    cx.simulate_input("milk");
    focus(&view, cx, "Trip", END);

    view.update_in(cx, OutlineView::open_search);
    cx.simulate_input("bread");

    assert!(search_focused(&view, cx));
    assert_eq!(matches(&view, cx), ["Bread"]);
}

#[gpui::test]
fn editing_the_current_match_so_it_no_longer_matches_lets_enter_carry_on_from_that_row_instead_of_starting_over(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Bread flour"),
        seed("Buy bread"),
        seed("Milk"),
        seed("Bread rolls"),
    ]));
    let (view, cx) = open(cx, &server, true);
    view.update_in(cx, OutlineView::open_search);
    cx.simulate_input("bread");
    cx.simulate_keystrokes("enter");
    assert_eq!(current_match(&view, cx), Some("Buy bread".into()));

    focus(&view, cx, "Buy bread", END);
    press(cx, "backspace", "bread".len());
    assert_eq!(matches(&view, cx), ["Bread flour", "Bread rolls"]);
    assert_eq!(current_match(&view, cx), None);

    view.update_in(cx, OutlineView::open_search);
    cx.simulate_keystrokes("enter");
    assert_eq!(current_match(&view, cx), Some("Bread rolls".into()));
}

fn thread_of(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Option<String> {
    view.read_with(cx, |view, _| {
        view.thread
            .as_ref()
            .map(|thread| view.outline.item(&thread.key).title.clone())
    })
}

fn composer_focused(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> bool {
    let composer = view.read_with(cx, |view, _| view.composer.clone());
    cx.update(|window, cx| composer.focus_handle(cx).is_focused(window))
}

fn log(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, _| {
        view.thread
            .as_ref()
            .map(|thread| {
                thread
                    .entries()
                    .iter()
                    .map(|entry| match entry {
                        Entry::Logged(TimelineItem::Event { kind, .. }) => kind.label().to_string(),
                        Entry::Logged(TimelineItem::Comment(comment)) => comment.body.clone(),
                        Entry::Pending { body, .. } => format!("Sending {body}"),
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

fn comments_on(view: &Entity<OutlineView>, cx: &mut VisualTestContext, title: &str) -> u64 {
    let key = key_of(view, cx, title);
    view.read_with(cx, |view, _| view.outline.item(&key).comments)
}

fn click_on(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"));
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());
}

fn open_comments_on(view: &Entity<OutlineView>, cx: &mut VisualTestContext, title: &str) {
    focus(view, cx, title, 0);
    cx.simulate_keystrokes("shift-enter");
    settle(cx);
}

#[gpui::test]
fn shift_enter_opens_the_items_comments_beside_the_outline_and_enter_posts_one_there_with_the_line_breaks_shift_enter_made(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("shift-enter");
    settle(cx);
    assert_eq!(thread_of(&view, cx).as_deref(), Some("Bread"));
    assert!(composer_focused(&view, cx));
    assert_eq!(log(&view, cx), ["Created"]);

    cx.simulate_input("near the station");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("opens at 7");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread [comment: near the station\\nopens at 7]\nTrip"
    );
    assert_eq!(log(&view, cx), ["Created", "near the station\nopens at 7"]);
    assert_eq!(comments_on(&view, cx, "Bread"), 1);
    assert!(composer_focused(&view, cx));
}

#[gpui::test]
fn cmd_enter_posts_too_for_those_used_to_it_from_other_apps_and_neither_key_posts_an_empty_comment(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    cx.simulate_keystrokes("enter");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert_eq!(log(&view, cx), ["Created"]);
    assert!(composer_focused(&view, cx));

    cx.simulate_input("opens at 7");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert_eq!(log(&view, cx), ["Created", "opens at 7"]);
    assert!(composer_focused(&view, cx));
}

#[gpui::test]
fn the_comment_count_on_the_row_goes_up_as_soon_as_a_comment_is_posted_before_the_server_answers(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    view.update_in(cx, |view, _, cx| {
        view.composer
            .update(cx, |field, cx| field.set_text("near the station", 0, cx));
        view.post_comment(cx);
    });

    assert_eq!(comments_on(&view, cx, "Bread"), 1);
    assert_eq!(log(&view, cx), ["Created", "Sending near the station"]);
    settle(cx);
    assert_eq!(log(&view, cx), ["Created", "near the station"]);
}

#[gpui::test]
fn clicking_a_rows_comment_button_opens_its_comments_ready_to_type(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);

    click_on(cx, "comments-t3");

    assert_eq!(thread_of(&view, cx).as_deref(), Some("Bread"));
    assert!(composer_focused(&view, cx));
}

#[gpui::test]
fn escape_in_the_comment_box_goes_back_to_the_row_with_the_pane_still_open_and_escape_on_the_row_closes_it(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    cx.simulate_keystrokes("escape");
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5)));
    assert_eq!(thread_of(&view, cx).as_deref(), Some("Bread"));

    cx.simulate_keystrokes("escape");
    assert_eq!(thread_of(&view, cx), None);
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5)));
}

#[gpui::test]
fn moving_the_caret_to_another_row_switches_the_open_pane_to_it_without_taking_the_keyboard_from_the_title(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("up");
    assert_eq!(thread_of(&view, cx).as_deref(), Some("Milk"));
    assert!(!composer_focused(&view, cx));
    cx.simulate_input("s");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milks\n  Bread\nTrip");
}

#[gpui::test]
fn a_comment_of_only_spaces_is_not_posted_and_others_are_sent_without_their_surrounding_spaces(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    cx.simulate_input("   ");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);

    cx.simulate_input("called them ");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread [comment: called them]\nTrip"
    );
}

#[gpui::test]
fn a_comment_written_on_a_row_the_server_has_not_created_yet_is_sent_after_it_under_its_new_id(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);

    view.update_in(cx, |view, window, cx| {
        view.add_item(window, cx);
        let fresh = view.editing.clone().unwrap();
        view.open_comments(&fresh, window, cx);
        view.composer
            .update(cx, |field, cx| field.set_text("booked", 0, cx));
        view.post_comment(cx);
    });
    settle(cx);

    let sent: Vec<String> = server
        .sent()
        .iter()
        .map(|request| format!("{} {}", request.method, request.path))
        .collect();
    assert_eq!(
        sent,
        ["POST /api/v1/todos", "POST /api/v1/todos/5/comments"]
    );
    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread\nTrip\n [comment: booked]"
    );
    assert_eq!(comments_on(&view, cx, ""), 1);
    settle(cx);
    assert_eq!(log(&view, cx), ["Created", "booked"]);
}

#[gpui::test]
fn backspace_never_joins_or_deletes_a_row_with_comments_so_its_log_is_not_thrown_away(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Buy "),
        seed("milk").comment("the low-fat one"),
        seed("").comment("ask Kim"),
    ]));
    let (view, cx) = open(cx, &server, true);
    let before = server.tree();

    focus(&view, cx, "milk", 0);
    cx.simulate_keystrokes("backspace");
    focus(&view, cx, "", 0);
    cx.simulate_keystrokes("backspace");
    settle(cx);

    assert_eq!(server.tree(), before);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn cmd_z_after_deleting_a_row_brings_back_the_same_todo_with_its_comments_instead_of_an_empty_copy(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip"),
        seed("Hotel").under(1).comment("booked"),
        seed("Home"),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Hotel", 0);

    cx.simulate_keystrokes("cmd-shift-backspace");
    settle(cx);
    assert_eq!(server.tree(), "Trip\nHome");

    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(server.tree(), "Trip\n  Hotel [comment: booked]\nHome");
    let last = server.sent().pop().unwrap();
    assert_eq!(
        (last.method, last.path.as_str(), last.body),
        (
            "POST",
            "/api/v1/todos/2/restore",
            Some(serde_json::json!({ "parentId": 1, "index": 0 }))
        )
    );
    assert_eq!(key_of(&view, cx, "Hotel"), "t2");
    assert_eq!(comments_on(&view, cx, "Hotel"), 1);
    assert_eq!(focused(&view, cx), Some(("Hotel".into(), 0)));
}

#[gpui::test]
fn cmd_z_after_joining_rows_brings_back_the_joined_todo_itself_so_its_id_and_history_survive(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);

    cx.simulate_keystrokes("backspace");
    settle(cx);
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert!(
        server
            .sent()
            .iter()
            .any(|request| request.path == "/api/v1/todos/3/restore")
    );
}

#[gpui::test]
fn the_search_matches_only_what_the_outline_shows_so_words_found_only_in_a_comment_count_as_no_match(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Health"),
        seed("Dentist").under(1).comment("Only weekday mornings"),
        seed("Weekday chores"),
    ]));
    let (view, cx) = open(cx, &server, true);
    view.update_in(cx, OutlineView::open_search);

    cx.simulate_input("weekday");
    settle(cx);

    assert_eq!(matches(&view, cx), ["Weekday chores"]);
    assert_eq!(current_match(&view, cx), Some("Weekday chores".into()));
}

#[gpui::test]
fn a_comment_can_be_edited_in_place_with_enter_while_escape_leaves_it_as_it_was(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("call on Monday"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");

    click_on(cx, "edit-comment-1");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("never mind");
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert_eq!(server.tree(), "Dentist [comment: call on Monday]");
    assert!(composer_focused(&view, cx));

    click_on(cx, "edit-comment-1");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("no answer, try Friday");
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(server.tree(), "Dentist [comment: no answer, try Friday]");
    assert_eq!(log(&view, cx), ["Created", "no answer, try Friday"]);
}

#[gpui::test]
fn deleting_a_comment_asks_first_quoting_it_and_cancelling_keeps_it(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist")
            .comment("call on Monday")
            .comment("booked for Friday"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");

    click_on(cx, "delete-comment-1");
    assert_eq!(
        cx.pending_prompt(),
        Some(("Delete this comment?".into(), "call on Monday".into()))
    );
    cx.simulate_prompt_answer("Cancel");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Dentist [comment: call on Monday] [comment: booked for Friday]"
    );

    click_on(cx, "delete-comment-1");
    cx.simulate_prompt_answer("Delete");
    settle(cx);
    assert_eq!(server.tree(), "Dentist [comment: booked for Friday]");
    assert_eq!(comments_on(&view, cx, "Dentist"), 1);
    assert_eq!(log(&view, cx), ["Created", "booked for Friday"]);
}

#[gpui::test]
fn completing_the_todo_while_its_comments_are_open_adds_the_completion_to_the_log_at_once(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("cmd-enter");

    assert_eq!(log(&view, cx), ["Created", "Completed"]);
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread [done]\nTrip");
}

#[gpui::test]
fn a_comment_the_server_refuses_pauses_editing_like_any_failed_save_and_nothing_more_is_posted(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");
    server.fail_with(500);

    cx.simulate_input("near the station");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert!(view.read_with(cx, |view, _| view.sync.failed));
    assert_eq!(log(&view, cx), ["Created", "Sending near the station"]);

    cx.simulate_input("again");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);
    assert_eq!(server.sent().len(), 1);
}

#[gpui::test]
fn switching_back_to_the_app_reloads_the_open_log_so_a_comment_added_from_an_assistant_meanwhile_appears(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Dentist")]));
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    open_comments_on(&view, cx, "Dentist");

    cx.deactivate_window();
    change_elsewhere(
        &server,
        "POST",
        "/api/v1/todos/1/comments",
        Some(serde_json::json!({ "body": "booked for Friday" })),
    );
    cx.update(|window, _| window.activate_window());
    settle(cx);

    assert_eq!(log(&view, cx), ["Created", "booked for Friday"]);
}

#[gpui::test]
fn switching_back_to_the_app_leaves_the_log_alone_while_a_comment_is_unsaved_so_it_still_shows_as_sending(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Dentist")]));
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    open_comments_on(&view, cx, "Dentist");
    server.fail_with(500);
    cx.simulate_input("booked for Friday");
    cx.simulate_keystrokes("cmd-enter");
    settle(cx);

    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    settle(cx);

    assert_eq!(server.timelines(), [1]);
    assert_eq!(log(&view, cx), ["Created", "Sending booked for Friday"]);
}

#[gpui::test]
fn a_comment_an_assistant_wrote_through_mcp_is_marked_so_it_is_not_mistaken_for_the_members_own(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist")
            .comment_via_mcp("found two clinics open on Saturday")
            .comment("booked the first one"),
    ]));
    let (view, cx) = open(cx, &server, true);

    open_comments_on(&view, cx, "Dentist");

    assert!(cx.debug_bounds("via-mcp-1").is_some());
    assert!(cx.debug_bounds("via-mcp-2").is_none());
}

#[gpui::test]
fn deleting_the_row_whose_comments_are_open_moves_the_pane_to_the_row_the_caret_lands_on(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("cmd-shift-backspace");
    settle(cx);

    assert_eq!(focused(&view, cx), Some(("Milk".into(), 4)));
    assert_eq!(thread_of(&view, cx).as_deref(), Some("Milk"));
}

#[gpui::test]
fn a_log_that_fails_to_load_offers_to_try_again_instead_of_showing_an_empty_history(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Dentist").comment("booked")]));
    server.fail_timelines(true);
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    assert_eq!(log(&view, cx), [] as [String; 0]);

    server.fail_timelines(false);
    click_on(cx, "retry-comments");
    settle(cx);

    assert_eq!(log(&view, cx), ["Created", "booked"]);
}

#[gpui::test]
fn cmd_z_in_the_comment_box_leaves_the_outline_alone_and_posting_a_comment_keeps_the_rows_undo_steps(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.simulate_input(" to Kyoto");
    settle(cx);

    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("booked");
    cx.simulate_keystrokes("cmd-enter cmd-z");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread\nTrip to Kyoto [comment: booked]"
    );

    cx.simulate_keystrokes("escape cmd-z");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread\nTrip [comment: booked]"
    );
}

#[gpui::test]
fn the_pane_closes_when_its_todo_is_deleted_on_another_device_so_no_comment_is_sent_to_a_todo_that_is_gone(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Trip");

    change_elsewhere(&server, "DELETE", "/api/v1/todos/4", None);
    view.update(cx, super::OutlineView::refresh);
    cx.run_until_parked();

    assert_eq!(thread_of(&view, cx), None);
}

fn composer_text(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |view, cx| view.composer.read(cx).text().to_string())
}

fn edit_of(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Option<(u64, String)> {
    view.read_with(cx, |view, cx| {
        let id = view.thread.as_ref()?.editing?;
        Some((id, view.comment_field.read(cx).text().to_string()))
    })
}

fn post_then_visit(view: &Entity<OutlineView>, cx: &mut VisualTestContext, keys: &[&str]) {
    view.update_in(cx, |view, window, cx| {
        view.composer
            .update(cx, |field, cx| field.set_text("near the station", 0, cx));
        view.post_comment(cx);
        for key in keys {
            view.focus_row(
                Focus {
                    key: (*key).to_string(),
                    pos: 0,
                },
                window,
                cx,
            );
        }
    });
}

#[gpui::test]
fn an_unposted_comment_and_an_edit_in_progress_wait_for_their_todo_while_the_caret_visits_another_row(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("call on Monday"),
        seed("Milk"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    cx.simulate_input("asked for a Saturday");
    click_on(cx, "edit-comment-1");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("call on Tuesday");

    focus(&view, cx, "Milk", 0);
    settle(cx);
    assert_eq!(composer_text(&view, cx), "");
    assert_eq!(edit_of(&view, cx), None);

    focus(&view, cx, "Dentist", 0);
    settle(cx);
    assert_eq!(composer_text(&view, cx), "asked for a Saturday");
    assert_eq!(edit_of(&view, cx), Some((1, "call on Tuesday".into())));
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);

    click_on(cx, "comment-editor");
    cx.simulate_keystrokes("enter");
    click_on(cx, "composer");
    cx.simulate_keystrokes("enter");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Dentist [comment: call on Tuesday] [comment: asked for a Saturday]\nMilk"
    );
}

#[gpui::test]
fn a_comment_still_sending_when_the_caret_moves_away_and_back_stays_in_the_log_until_it_is_saved(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    post_then_visit(&view, cx, &["t2", "t3"]);

    assert_eq!(log(&view, cx), ["Sending near the station"]);
    settle(cx);
    assert_eq!(log(&view, cx), ["Created", "near the station"]);
}

#[gpui::test]
fn a_comment_saved_while_the_pane_shows_another_todo_is_not_left_sending_when_its_todo_comes_back(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Bread");

    post_then_visit(&view, cx, &["t2"]);
    settle(cx);
    assert_eq!(thread_of(&view, cx).as_deref(), Some("Milk"));
    focus(&view, cx, "Bread", 0);
    settle(cx);

    assert_eq!(log(&view, cx), ["Created", "near the station"]);
}

#[gpui::test]
fn a_row_the_server_has_not_created_yet_shows_no_made_up_creation_time_until_the_server_answers(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);

    view.update_in(cx, |view, window, cx| {
        view.add_item(window, cx);
        let fresh = view.editing.clone().unwrap();
        view.open_comments(&fresh, window, cx);
    });
    assert_eq!(log(&view, cx), [] as [String; 0]);
    settle(cx);
    settle(cx);

    assert_eq!(log(&view, cx), ["Created"]);
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
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 5)));
}

#[gpui::test]
fn escape_closes_the_move_picker_and_returns_to_where_the_caret_was(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 2);

    cx.simulate_keystrokes("cmd-shift-m escape");

    assert!(view.read_with(cx, |view, _| view.picker.is_none()));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));
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
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));
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
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 0)));
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

fn icon_picker_open(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |view, _| view.icon_picker.is_some())
}

fn last_sent(server: &FakeServer) -> Option<(&'static str, String, Option<serde_json::Value>)> {
    server
        .sent()
        .pop()
        .map(|request| (request.method, request.path, request.body))
}

#[gpui::test]
fn cmd_shift_e_opens_the_icon_picker_and_a_chosen_emoji_is_saved_at_once_with_the_caret_back_in_the_title(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 2);

    cx.simulate_keystrokes("cmd-shift-e");
    assert!(icon_picker_open(&view, cx));
    assert!(cx.debug_bounds("remove-icon").is_none());
    assert!(cx.debug_bounds("icon-choice-selected").is_none());
    click_on(cx, "icon-choice-8");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Groceries\n  Milk [icon: \u{1F6D2}]\n  Bread\nTrip"
    );
    assert_eq!(
        last_sent(&server),
        Some((
            "PATCH",
            "/api/v1/todos/2".into(),
            Some(serde_json::json!({ "icon": "\u{1F6D2}" }))
        ))
    );
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));
}

#[gpui::test]
fn the_icon_takes_the_bullets_place_so_titles_with_and_without_one_start_at_the_same_x(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk").icon("\u{1F95B}"),
        seed("Bread"),
    ]));
    let (_view, cx) = open(cx, &server, true);

    let icon = cx.debug_bounds("icon-t1").expect("the icon was not drawn");
    let title = cx.debug_bounds("title-t1").unwrap();
    assert!(icon.right() <= title.left());
    assert!((icon.center().y - title.center().y).abs() < px(0.5));
    assert_eq!(title.left(), cx.debug_bounds("title-t2").unwrap().left());
    assert!(cx.debug_bounds("icon-t2").is_none());
}

#[gpui::test]
fn clicking_a_parents_icon_folds_and_unfolds_it_like_its_bullet_while_a_leafs_icon_does_nothing(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Groceries").icon("\u{1F6D2}"),
        seed("Milk").under(1).icon("\u{1F95B}"),
    ]));
    let (view, cx) = open(cx, &server, true);

    click_on(cx, "icon-t2");
    settle(cx);
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(server.sent().len(), 0);

    click_on(cx, "icon-t1");
    settle(cx);
    assert!(cx.debug_bounds("title-t2").is_none());
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(
        server.tree(),
        "Groceries [icon: \u{1F6D2}] [collapsed]\n  Milk [icon: \u{1F95B}]"
    );

    click_on(cx, "icon-t1");
    settle(cx);
    assert!(cx.debug_bounds("title-t2").is_some());
}

#[gpui::test]
fn a_done_row_keeps_its_icon_with_a_check_on_its_corner_so_it_still_reads_as_done(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk").icon("\u{1F95B}").done(),
        seed("Bread").done(),
        seed("Eggs").icon("\u{1F95A}"),
    ]));
    let (_view, cx) = open(cx, &server, true);

    let icon = cx.debug_bounds("icon-t1").expect("the icon was not drawn");
    let badge = cx
        .debug_bounds("done-badge-t1")
        .expect("the check was not drawn");
    assert!(badge.center().x > icon.center().x && badge.center().y > icon.center().y);
    assert!(badge.left() < icon.right() && badge.top() < icon.bottom());
    assert!(cx.debug_bounds("done-badge-t2").is_none());
    assert!(cx.debug_bounds("done-badge-t3").is_none());
}

#[gpui::test]
fn the_row_action_opens_the_picker_with_the_rows_icon_marked(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk").icon("\u{1F95B}"),
        seed("Bread"),
    ]));
    let (view, cx) = open(cx, &server, true);

    click_on(cx, "set-icon-t1");

    assert!(icon_picker_open(&view, cx));
    assert_eq!(
        cx.debug_bounds("icon-choice-selected"),
        cx.debug_bounds("icon-choice-9")
    );
    assert!(cx.debug_bounds("remove-icon").is_some());
}

#[gpui::test]
fn the_row_action_opens_the_picker_for_any_row_and_remove_icon_sends_null_so_the_icon_is_gone_everywhere(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk").icon("\u{1F95B}"),
        seed("Bread"),
    ]));
    let (view, cx) = open(cx, &server, true);

    click_on(cx, "set-icon-t1");
    click_on(cx, "remove-icon");
    settle(cx);

    assert_eq!(server.tree(), "Milk\nBread");
    assert_eq!(
        last_sent(&server),
        Some((
            "PATCH",
            "/api/v1/todos/1".into(),
            Some(serde_json::json!({ "icon": null }))
        ))
    );
    assert!(cx.debug_bounds("icon-t1").is_none());
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 4)));
}

#[gpui::test]
fn a_typed_icon_from_a_later_release_is_never_drawn_but_still_counts_as_an_icon_so_backspace_keeps_it_and_it_can_be_removed(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Buy "),
        seed("milk").icon("line:cart:green"),
    ]));
    let (view, cx) = open(cx, &server, true);
    assert!(cx.debug_bounds("icon-t2").is_none());

    focus(&view, cx, "milk", 0);
    cx.simulate_keystrokes("backspace");
    settle(cx);
    assert_eq!(server.tree(), "Buy \nmilk [icon: line:cart:green]");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);

    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "remove-icon");
    settle(cx);
    assert_eq!(server.tree(), "Buy \nmilk");
}

#[gpui::test]
fn backspace_never_joins_or_deletes_a_row_with_an_icon_so_the_icon_is_not_thrown_away(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Buy "),
        seed("milk").icon("\u{1F95B}"),
        seed("").icon("\u{1F6D2}"),
    ]));
    let (view, cx) = open(cx, &server, true);
    let before = server.tree();

    focus(&view, cx, "milk", 0);
    cx.simulate_keystrokes("backspace");
    focus(&view, cx, "", 0);
    cx.simulate_keystrokes("backspace");
    settle(cx);

    assert_eq!(server.tree(), before);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn enter_in_other_saves_one_typed_emoji_without_its_surrounding_spaces_and_refuses_anything_else_without_sending_it(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Bread", 0);
    cx.simulate_keystrokes("cmd-shift-e");

    for refused in ["ab", "\u{2764}", "\u{1F6D2}\u{1F6D2}"] {
        cx.simulate_keystrokes("cmd-a");
        cx.simulate_input(refused);
        assert!(cx.debug_bounds("icon-refused").is_none());
        cx.simulate_keystrokes("enter");
        assert!(
            cx.debug_bounds("icon-refused").is_some(),
            "{refused:?} was not refused"
        );
        assert!(icon_picker_open(&view, cx));
    }
    settle(cx);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);

    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input(" \u{1F9B7} ");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(
        server.tree(),
        "Groceries\n  Milk\n  Bread [icon: \u{1F9B7}]\nTrip"
    );
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Bread".into(), 0)));
}

#[gpui::test]
fn escape_or_a_click_outside_closes_the_icon_picker_without_a_change_and_returns_to_where_the_caret_was(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Milk", 2);

    cx.simulate_keystrokes("cmd-shift-e escape");
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));

    cx.simulate_keystrokes("cmd-shift-e");
    let outside = gpui::point(px(3.), px(3.));
    cx.simulate_mouse_move(outside, None, gpui::Modifiers::none());
    cx.simulate_click(outside, gpui::Modifiers::none());
    settle(cx);

    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn cmd_z_takes_an_icon_change_back_in_one_step_restoring_the_old_icon_or_none_on_the_server(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Milk").icon("\u{1F95B}"),
        seed("Bread"),
    ]));
    let (view, cx) = open(cx, &server, true);

    focus(&view, cx, "Bread", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "icon-choice-8");
    settle(cx);
    click_on(cx, "set-icon-t1");
    click_on(cx, "icon-choice-0");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Milk [icon: \u{1F3E0}]\nBread [icon: \u{1F6D2}]"
    );

    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Milk [icon: \u{1F95B}]\nBread [icon: \u{1F6D2}]"
    );
    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(server.tree(), "Milk [icon: \u{1F95B}]\nBread");
}

#[gpui::test]
fn cmd_z_in_the_icon_picker_leaves_the_outline_behind_it_alone(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);
    cx.simulate_input(" to Kyoto");
    settle(cx);

    cx.simulate_keystrokes("cmd-shift-e");
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip to Kyoto");
    assert!(icon_picker_open(&view, cx));
}

#[gpui::test]
fn enter_keeps_the_icon_on_the_row_that_was_split_so_the_new_row_does_not_look_like_a_copy(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Book hotel").icon("\u{1F3E8}")]));
    let (view, cx) = open(cx, &server, true);

    focus(&view, cx, "Book hotel", 4);
    cx.simulate_keystrokes("enter");
    focus(&view, cx, "Book", 0);
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.tree(), "\nBook [icon: \u{1F3E8}]\n hotel");
}

#[gpui::test]
fn cmd_z_after_deleting_a_row_brings_it_back_with_its_icon(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Trip"),
        seed("Hotel").under(1).icon("\u{1F3E8}"),
    ]));
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Hotel", 0);

    cx.simulate_keystrokes("cmd-shift-backspace");
    settle(cx);
    assert_eq!(server.tree(), "Trip");
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Trip\n  Hotel [icon: \u{1F3E8}]");
    assert!(cx.debug_bounds("icon-t2").is_some());
}

fn icon_picker(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Entity<IconPicker> {
    view.read_with(cx, |view, _| view.icon_picker.clone())
        .expect("the icon picker is open")
}

fn right_click_on(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
        .center();
    cx.simulate_mouse_down(at, MouseButton::Right, gpui::Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, gpui::Modifiers::none());
}

fn choose_file(cx: &mut VisualTestContext, file: &TempFile) {
    click_on(cx, "add-image");
    let path = file.0.clone();
    cx.simulate_path_prompt_response(move |_| Some(vec![path]));
    settle(cx);
}

fn name_typed(
    view: &Entity<OutlineView>,
    cx: &mut VisualTestContext,
) -> (String, std::ops::Range<usize>) {
    let field = icon_picker(view, cx).read_with(cx, |picker, _| picker.name_field());
    field.read_with(cx, |field, _| (field.text().to_string(), field.selection()))
}

fn image_problem(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Option<String> {
    icon_picker(view, cx).read_with(cx, |picker, _| picker.problem().map(str::to_owned))
}

fn image_names(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |view, _| {
        view.icons
            .list
            .iter()
            .map(|image| image.name.clone())
            .collect()
    })
}

fn icon_of(view: &Entity<OutlineView>, cx: &mut VisualTestContext, key: &str) -> Option<String> {
    view.read_with(cx, |view, _| view.outline.item(key).icon.clone())
}

#[gpui::test]
fn an_image_icon_is_drawn_in_place_of_the_bullet_once_loaded_and_one_missing_from_the_list_draws_nothing_but_still_counts_as_an_icon(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![
            seed("Feed Mike").icon("image:img1"),
            seed("Feed Pochi").icon("image:gone"),
        ])
        .with_images(&["Mike"]),
    );
    let (view, cx) = open(cx, &server, true);
    settle(cx);

    let icon = cx.debug_bounds("icon-t1").expect("the image was not drawn");
    let title = cx.debug_bounds("title-t1").unwrap();
    assert!(icon.right() <= title.left());
    assert!((icon.center().y - title.center().y).abs() < px(0.5));
    assert!(cx.debug_bounds("icon-t2").is_none());

    focus(&view, cx, "Feed Pochi", 0);
    cx.simulate_keystrokes("backspace");
    settle(cx);
    assert_eq!(
        server.tree(),
        "Feed Mike [icon: image:img1]\nFeed Pochi [icon: image:gone]"
    );
}

#[gpui::test]
fn a_todo_showing_an_image_the_icon_list_did_not_have_fetches_the_list_again_so_the_image_appears(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Feed the cat")]));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    assert_eq!(server.image_lists(), 1);

    let id = server.store_image("Mike", png(8, 8));
    change_elsewhere(
        &server,
        "PATCH",
        "/api/v1/todos/1",
        Some(serde_json::json!({ "icon": format!("image:{id}") })),
    );
    server.fail_image_lists(1);
    view.update(cx, OutlineView::refresh);
    settle(cx);

    assert_eq!(server.image_lists(), 3);
    assert!(cx.debug_bounds("icon-t1").is_some());
}

#[gpui::test]
fn a_failing_icon_list_leaves_the_outline_working_with_images_undrawn_and_is_not_asked_for_again_in_a_loop(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![seed("Feed Mike").icon("image:img1"), seed("Milk")])
            .with_images(&["Mike"]),
    );
    server.fail_image_lists(usize::MAX);
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    view.update(cx, OutlineView::refresh);
    settle(cx);

    assert_eq!(titles(&view, cx), ["Feed Mike", "Milk"]);
    assert!(cx.debug_bounds("icon-t1").is_none());
    assert_eq!(server.image_lists(), 2);
    assert_eq!(server.image_downloads(), 0);

    focus(&view, cx, "Milk", END);
    cx.simulate_input(" and eggs");
    settle(cx);
    assert_eq!(server.tree(), "Feed Mike [icon: image:img1]\nMilk and eggs");
}

#[gpui::test]
fn an_image_is_downloaded_once_and_kept_by_id_so_refreshing_the_todos_does_not_download_it_again(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![seed("Feed Mike").icon("image:img1")])
            .with_images(&["Mike", "Pochi"]),
    );
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    assert_eq!(server.image_downloads(), 2);

    view.update(cx, OutlineView::refresh);
    settle(cx);

    assert_eq!(server.image_lists(), 2);
    assert_eq!(server.image_downloads(), 2);
    assert!(cx.debug_bounds("icon-t1").is_some());
}

#[gpui::test]
fn the_picker_shows_my_icons_in_the_order_they_were_added_then_the_add_tile_above_the_emoji_with_the_rows_image_marked(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![seed("Walk Pochi").icon("image:img2"), seed("Milk")])
            .with_images(&["Mike", "Pochi"]),
    );
    let (_view, cx) = open(cx, &server, true);
    settle(cx);

    click_on(cx, "set-icon-t1");

    let mike = cx.debug_bounds("my-icon-0").unwrap();
    let pochi = cx.debug_bounds("my-icon-1").unwrap();
    let add = cx.debug_bounds("add-image").unwrap();
    let first_emoji = cx.debug_bounds("icon-choice-0").unwrap();
    assert!(mike.right() <= pochi.left());
    assert!(pochi.right() <= add.left());
    assert_eq!(mike.top(), add.top());
    assert!(add.bottom() <= first_emoji.top());
    assert_eq!(cx.debug_bounds("icon-choice-selected"), Some(pochi));
}

#[gpui::test]
fn the_add_tile_is_there_before_any_image_was_added_so_the_first_one_can_be(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);

    cx.simulate_keystrokes("cmd-shift-e");

    assert!(cx.debug_bounds("add-image").is_some());
    assert!(cx.debug_bounds("my-icon-0").is_none());
}

#[gpui::test]
fn choosing_an_image_saves_it_as_the_rows_icon_at_once_like_an_emoji_with_the_caret_back_in_the_title(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Feed the cat")]).with_images(&["Mike"]));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Feed the cat", 4);

    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "my-icon-0");
    settle(cx);

    assert_eq!(server.tree(), "Feed the cat [icon: image:img1]");
    assert_eq!(
        last_sent(&server),
        Some((
            "PATCH",
            "/api/v1/todos/1".into(),
            Some(serde_json::json!({ "icon": "image:img1" }))
        ))
    );
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Feed the cat".into(), 4)));
    assert!(cx.debug_bounds("icon-t1").is_some());
}

#[gpui::test]
fn adding_an_image_sends_a_128_pixel_square_named_after_the_file_and_puts_it_on_the_row_as_one_undo_step(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Mike.png", &png(300, 200));

    choose_file(cx, &photo);

    assert!(cx.debug_bounds("image-add-view").is_some());
    assert_eq!(name_typed(&view, cx), ("Mike".to_string(), 0..4));
    assert_eq!(server.image_calls(), [] as [String; 0]);

    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.image_calls(), ["POST Mike"]);
    let sent = image::load_from_memory(&server.image_bytes("img1").unwrap()).unwrap();
    assert_eq!((sent.width(), sent.height()), (128, 128));
    assert_eq!(
        server.tree(),
        "Groceries\n  Milk [icon: image:img1]\n  Bread\nTrip"
    );
    assert!(!icon_picker_open(&view, cx));
    assert!(cx.debug_bounds("icon-t2").is_some());

    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(server.images().len(), 1);
    assert_eq!(image_names(&view, cx), ["Mike"]);
}

#[gpui::test]
fn a_file_that_is_not_an_image_keeps_the_grid_saying_it_could_not_be_read_and_sends_nothing(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let notes = TempFile::with("notes.txt", b"not a picture");

    choose_file(cx, &notes);

    assert!(cx.debug_bounds("image-unreadable").is_some());
    assert!(cx.debug_bounds("image-add-view").is_none());
    assert!(cx.debug_bounds("icon-choice-0").is_some());
    assert!(icon_picker_open(&view, cx));
    assert_eq!(server.image_calls(), [] as [String; 0]);
}

#[gpui::test]
fn an_empty_name_asks_for_one_and_sends_nothing(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Mike.png", &png(40, 40));
    choose_file(cx, &photo);

    cx.simulate_keystrokes("backspace");
    cx.simulate_input("   ");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(image_problem(&view, cx).as_deref(), Some("Enter a name"));
    assert!(cx.debug_bounds("image-add-view").is_some());
    assert_eq!(server.image_calls(), [] as [String; 0]);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn a_51st_image_shows_the_servers_reason_and_stays_in_the_add_view_with_the_row_unchanged(
    cx: &mut TestAppContext,
) {
    let names: Vec<String> = (1..=50).map(|n| format!("Pet {n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let server = Arc::new(FakeServer::with(vec![seed("Milk")]).with_images(&names));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Tama.png", &png(40, 40));
    choose_file(cx, &photo);

    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.image_calls(), ["POST Tama"]);
    assert_eq!(
        image_problem(&view, cx).as_deref(),
        Some("You can add up to 50 images")
    );
    assert!(cx.debug_bounds("image-add-view").is_some());
    assert_eq!(server.tree(), "Milk");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn back_leaves_the_add_view_for_the_grid_and_escape_closes_the_whole_picker_without_adding_anything(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 2);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Mike.png", &png(40, 40));

    choose_file(cx, &photo);
    click_on(cx, "image-cancel");
    assert!(cx.debug_bounds("image-add-view").is_none());
    assert!(cx.debug_bounds("icon-choice-0").is_some());

    choose_file(cx, &photo);
    cx.simulate_keystrokes("escape");
    settle(cx);

    assert!(!icon_picker_open(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 2)));
    assert_eq!(server.image_calls(), [] as [String; 0]);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

fn save_name_before_anything_answers(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.dispatch_action(Box::new(crate::ui::icon_picker::SaveImageName), cx);
    });
}

fn close_icon_picker_before_anything_answers(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.dispatch_action(Box::new(crate::ui::icon_picker::CloseIconPicker), cx);
    });
}

fn title_focused(view: &Entity<OutlineView>, cx: &mut VisualTestContext) -> bool {
    let field = view.read_with(cx, |view, _| view.title_field.clone());
    cx.update(|window, cx| field.focus_handle(cx).is_focused(window))
}

fn click_before_anything_answers(cx: &mut VisualTestContext, selector: &'static str) {
    let position = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
        .center();
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: gpui::Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        window.dispatch_event(
            gpui::PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: gpui::Modifiers::none(),
                click_count: 1,
            }),
            cx,
        );
    });
}

#[gpui::test]
fn closing_the_picker_while_a_new_image_uploads_still_adds_it_to_my_icons_at_once_but_leaves_the_row_alone(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Tama.png", &png(40, 40));
    choose_file(cx, &photo);

    save_name_before_anything_answers(cx);
    close_icon_picker_before_anything_answers(cx);
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(server.image_calls(), [] as [String; 0]);
    settle(cx);

    assert_eq!(server.image_calls(), ["POST Tama"]);
    assert_eq!(image_names(&view, cx), ["Tama"]);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
    assert!(!icon_picker_open(&view, cx));
    cx.simulate_keystrokes("cmd-shift-e");
    assert!(cx.debug_bounds("my-icon-0").is_some());
}

#[gpui::test]
fn going_back_to_the_grid_while_a_new_image_uploads_shows_it_in_my_icons_without_putting_it_on_the_row(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    let photo = TempFile::with("Tama.png", &png(40, 40));
    choose_file(cx, &photo);

    save_name_before_anything_answers(cx);
    click_before_anything_answers(cx, "image-cancel");
    assert_eq!(server.image_calls(), [] as [String; 0]);
    settle(cx);

    assert_eq!(server.image_calls(), ["POST Tama"]);
    assert_eq!(image_names(&view, cx), ["Tama"]);
    assert!(icon_picker_open(&view, cx));
    assert!(cx.debug_bounds("image-add-view").is_none());
    assert!(cx.debug_bounds("my-icon-0").is_some());
    assert_eq!(image_problem(&view, cx), None);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip");
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn right_clicking_an_image_renames_it_and_returns_to_the_grid_with_the_new_name(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Milk")]).with_images(&["Mike"]));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");

    right_click_on(cx, "my-icon-0");
    assert!(cx.debug_bounds("image-edit-view").is_some());
    assert_eq!(name_typed(&view, cx).0, "Mike");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Tama");
    cx.simulate_keystrokes("enter");
    settle(cx);

    assert_eq!(server.image_calls(), ["PATCH img1 Tama"]);
    assert_eq!(image_names(&view, cx), ["Tama"]);
    assert!(cx.debug_bounds("image-edit-view").is_none());
    assert!(cx.debug_bounds("my-icon-0").is_some());
    assert!(icon_picker_open(&view, cx));
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
    cx.simulate_keystrokes("escape");
    assert!(!icon_picker_open(&view, cx));
}

#[gpui::test]
fn closing_the_picker_while_an_image_is_being_renamed_still_shows_the_new_name_and_leaves_the_keyboard_in_the_title(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Milk")]).with_images(&["Mike"]));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    right_click_on(cx, "my-icon-0");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("Tama");

    save_name_before_anything_answers(cx);
    close_icon_picker_before_anything_answers(cx);
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(server.image_calls(), [] as [String; 0]);
    settle(cx);

    assert_eq!(server.image_calls(), ["PATCH img1 Tama"]);
    assert_eq!(image_names(&view, cx), ["Tama"]);
    assert!(title_focused(&view, cx));
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 0)));
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
}

#[gpui::test]
fn deleting_an_image_says_how_many_todos_lose_it_then_takes_it_off_them_here_without_a_patch_since_the_server_already_did(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![
            seed("Feed Mike").icon("image:img1"),
            seed("Brush Mike").icon("image:img1"),
            seed("Milk").icon("\u{1F95B}"),
        ])
        .with_images(&["Mike"]),
    );
    let (view, cx) = open(cx, &server, true);
    settle(cx);

    click_on(cx, "set-icon-t3");
    right_click_on(cx, "my-icon-0");
    click_on(cx, "delete-image");
    click_on(cx, "delete-image-back");
    assert!(cx.debug_bounds("image-edit-view").is_some());
    click_on(cx, "delete-image");
    assert_eq!(
        icon_picker(&view, cx).read_with(cx, |picker, _| picker.warning()),
        Some(
            "It is the icon of 2 todos. Deleting it removes the icon from them and cannot be undone."
                .into()
        )
    );
    click_on(cx, "delete-image-confirm");
    settle(cx);

    assert_eq!(server.image_calls(), ["DELETE img1"]);
    assert_eq!(server.sent(), [] as [nestodo_desktop::wire::Request; 0]);
    assert_eq!(icon_of(&view, cx, "t1"), None);
    assert_eq!(icon_of(&view, cx, "t2"), None);
    assert_eq!(icon_of(&view, cx, "t3").as_deref(), Some("\u{1F95B}"));
    assert_eq!(
        server.tree(),
        "Feed Mike\nBrush Mike\nMilk [icon: \u{1F95B}]"
    );
    assert!(icon_picker_open(&view, cx));
    assert!(cx.debug_bounds("my-icon-0").is_none());
    assert!(cx.debug_bounds("image-delete-view").is_none());
    assert_eq!(image_names(&view, cx), [] as [String; 0]);
}

#[gpui::test]
fn closing_the_picker_while_an_image_is_being_deleted_still_forgets_it_here_clears_its_rows_without_a_patch_and_forgets_undo(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(
        FakeServer::with(vec![seed("Feed Mike").icon("image:img1"), seed("Milk")])
            .with_images(&["Mike"]),
    );
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "my-icon-0");
    settle(cx);
    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "icon-choice-0");
    settle(cx);
    let sent = server.sent().len();
    cx.simulate_keystrokes("cmd-shift-e");
    right_click_on(cx, "my-icon-0");
    click_on(cx, "delete-image");

    click_before_anything_answers(cx, "delete-image-confirm");
    close_icon_picker_before_anything_answers(cx);
    assert!(!icon_picker_open(&view, cx));
    assert_eq!(server.image_calls(), [] as [String; 0]);
    settle(cx);

    assert_eq!(server.image_calls(), ["DELETE img1"]);
    assert_eq!(image_names(&view, cx), [] as [String; 0]);
    assert_eq!(icon_of(&view, cx, "t1"), None);
    assert_eq!(server.sent().len(), sent);
    assert!(title_focused(&view, cx));
    cx.simulate_keystrokes("cmd-z");
    settle(cx);
    assert_eq!(server.sent().len(), sent);
    assert_eq!(server.tree(), "Feed Mike\nMilk [icon: \u{1F3E0}]");
    assert!(!view.read_with(cx, |view, _| view.save_failed()));
}

#[gpui::test]
fn after_deleting_an_image_undo_cannot_put_it_back_on_a_row_so_the_server_never_refuses_it_and_pauses_editing(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Milk")]).with_images(&["Mike"]));
    let (view, cx) = open(cx, &server, true);
    settle(cx);
    focus(&view, cx, "Milk", 0);
    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "my-icon-0");
    settle(cx);
    cx.simulate_keystrokes("cmd-shift-e");
    click_on(cx, "icon-choice-0");
    settle(cx);
    let sent = server.sent().len();

    cx.simulate_keystrokes("cmd-shift-e");
    right_click_on(cx, "my-icon-0");
    click_on(cx, "delete-image");
    assert_eq!(
        icon_picker(&view, cx).read_with(cx, |picker, _| picker.warning()),
        Some("Deleting it cannot be undone.".into())
    );
    click_on(cx, "delete-image-confirm");
    settle(cx);
    cx.simulate_keystrokes("escape");
    cx.simulate_keystrokes("cmd-z");
    settle(cx);

    assert_eq!(server.sent().len(), sent);
    assert_eq!(server.tree(), "Milk [icon: \u{1F3E0}]");
    assert!(!view.read_with(cx, |view, _| view.save_failed()));
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
    assert_eq!(focused(&view, cx), Some(("Groceries".into(), 3)));

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
fn slash_opens_the_search_only_outside_a_title_so_it_can_still_be_typed(cx: &mut TestAppContext) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    focus(&view, cx, "Trip", END);

    cx.simulate_input("/");
    settle(cx);
    assert_eq!(server.tree(), "Groceries\n  Milk\n  Bread\nTrip/");
    assert!(!searching(&view, cx));

    view.update_in(cx, super::OutlineView::blur_editor);
    cx.simulate_input("/");
    assert!(searching(&view, cx));
    cx.simulate_input("bre");
    assert_eq!(matches(&view, cx), ["Bread"]);
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milk", "  Bread", "Trip/"]
    );
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
    assert_eq!(focused(&view, cx), Some(("Milk".into(), 1)));
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
fn the_add_button_floats_in_the_window_corner_and_steps_aside_for_the_comments_pane_so_the_pane_never_covers_it(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(900.), px(600.)));
    let corner = cx
        .debug_bounds("add-item")
        .expect("the add button was drawn");
    assert_eq!(
        (corner.right(), corner.bottom()),
        (px(900. - 16.), px(600. - 16.))
    );

    open_comments_on(&view, cx, "Trip");
    let pane = cx.debug_bounds("thread").expect("the pane was drawn");
    let beside = cx
        .debug_bounds("add-item")
        .expect("the add button was drawn");
    assert_eq!(
        (beside.right(), beside.bottom()),
        (pane.left() - px(16.), corner.bottom())
    );

    click_on(cx, "add-item");
    settle(cx);
    assert_eq!(
        titles(&view, cx),
        ["Groceries", "  Milk", "  Bread", "Trip", ""]
    );
    assert_eq!(focused(&view, cx), Some((String::new(), 0)));
}

#[gpui::test]
fn a_click_on_the_add_button_adds_a_row_and_never_reaches_the_row_drawn_under_it(
    cx: &mut TestAppContext,
) {
    let server = long_list();
    let (view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(400.), px(400.)));
    let button = cx
        .debug_bounds("add-item")
        .expect("the add button was drawn");
    let overlap = (1..=40)
        .find_map(|n| {
            let selector: &'static str = Box::leak(format!("comments-t{n}").into_boxed_str());
            cx.debug_bounds(selector)
                .map(|comments| comments.intersect(&button))
                .filter(|overlap| overlap.size.width > px(0.) && overlap.size.height > px(0.))
        })
        .expect("a row's comment button lies under the add button");

    cx.simulate_click(overlap.center(), gpui::Modifiers::none());
    settle(cx);

    assert_eq!(thread_of(&view, cx), None);
    assert_eq!(titles(&view, cx).len(), 41);
    assert_eq!(focused(&view, cx), Some((String::new(), 0)));
}

#[gpui::test]
fn the_last_row_scrolls_clear_of_the_add_button_so_no_todo_is_stuck_under_it(
    cx: &mut TestAppContext,
) {
    let server = long_list();
    let (_view, cx) = open(cx, &server, true);
    cx.simulate_resize(gpui::size(px(400.), px(400.)));

    cx.simulate_event(gpui::ScrollEvent {
        position: gpui::point(px(100.), px(200.)),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-5000.))),
        ..gpui::ScrollEvent::default()
    });
    settle(cx);

    let button = cx
        .debug_bounds("add-item")
        .expect("the add button was drawn");
    let last = cx
        .debug_bounds("title-t40")
        .expect("the last row was drawn");
    assert!(
        last.bottom() <= button.top(),
        "the last row ends at {:?}, below the button's top at {:?}",
        last.bottom(),
        button.top()
    );
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
fn switching_to_another_app_with_the_empty_search_open_keeps_it_open_so_a_search_term_copied_there_can_be_typed_on_return(
    cx: &mut TestAppContext,
) {
    let server = groceries();
    let (view, cx) = open(cx, &server, true);
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    view.update_in(cx, super::OutlineView::open_search);
    cx.run_until_parked();

    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.simulate_input("milk");

    assert_eq!(matches(&view, cx), ["Milk"]);
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

#[gpui::test]
fn clicking_a_links_label_in_a_comment_opens_its_address_and_clicking_the_text_beside_it_opens_nothing(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("see [the form](https://example.com/form) first"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let text = cx
        .debug_bounds("comment-text-1-1")
        .expect("the comment was drawn");
    let glyph = 14.5 * 0.6;
    let at = |glyphs: f32| {
        gpui::point(
            text.origin.x + px(glyph * glyphs),
            text.origin.y + text.size.height / 2.,
        )
    };

    cx.simulate_click(at(1.5), gpui::Modifiers::none());
    assert_eq!(cx.opened_url(), None);

    cx.simulate_click(at(6.5), gpui::Modifiers::none());
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/form"));
}

#[gpui::test]
fn every_kind_of_markdown_in_a_comment_is_drawn_inside_the_pane_so_nothing_a_member_writes_is_cut_off_or_missing(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![seed("Dentist").comment(
        "# Plan\n\n**call** the *clinic*, ~~email~~ `now`\n\n1. book\n   - Saturday\n2. pay\n\n> ask about parking\n\n---\n\n```\nopen 9-17\n```",
    )]));
    let (view, cx) = open(cx, &server, true);

    open_comments_on(&view, cx, "Dentist");

    let pane = cx.debug_bounds("thread").expect("the pane was drawn");
    for part in 1..=7 {
        let selector: &'static str = Box::leak(format!("comment-text-1-{part}").into_boxed_str());
        let drawn = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} was not drawn"));
        assert!(drawn.size.height > px(0.), "{selector} has no height");
        assert!(
            drawn.left() >= pane.left() && drawn.right() <= pane.right(),
            "{selector} leaves the pane"
        );
    }
    assert!(cx.debug_bounds("comment-text-1-8").is_none());
}

fn code_comment() -> &'static str {
    Box::leak(format!("```\n{}\nend\n```", "x".repeat(300)).into_boxed_str())
}

#[gpui::test]
fn a_long_line_of_code_in_a_comment_stays_on_one_line_inside_the_pane_so_pasted_output_keeps_its_shape(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment(code_comment()),
    ]));
    let (view, cx) = open(cx, &server, true);

    open_comments_on(&view, cx, "Dentist");

    let pane = cx.debug_bounds("thread").expect("the pane was drawn");
    let block = cx
        .debug_bounds("comment-text-1-1")
        .expect("the code block was drawn");
    let lines = cx
        .debug_bounds("comment-text-1-1-lines")
        .expect("the code was drawn");
    assert_eq!(lines.size.height, px(40.));
    assert!(lines.size.width > block.size.width);
    assert!(block.right() <= pane.right());
}

#[gpui::test]
fn scrolling_sideways_over_a_code_block_moves_only_its_code_and_scrolling_up_still_moves_the_log(
    cx: &mut TestAppContext,
) {
    let mut dentist = seed("Dentist");
    for _ in 0..40 {
        dentist = dentist.comment("called again");
    }
    let server = Arc::new(FakeServer::with(vec![dentist.comment(code_comment())]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let placed = |cx: &mut VisualTestContext| {
        let block = cx
            .debug_bounds("comment-text-41-1")
            .expect("the code block was drawn");
        let lines = cx
            .debug_bounds("comment-text-41-1-lines")
            .expect("the code was drawn");
        (block.origin, lines.origin.x)
    };
    let (block, code) = placed(cx);
    let over = gpui::point(block.x + px(40.), block.y + px(20.));
    let scroll = |cx: &mut VisualTestContext, x: f32, y: f32| {
        cx.simulate_event(gpui::ScrollEvent {
            position: over,
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(x), px(y))),
            ..gpui::ScrollEvent::default()
        });
        settle(cx);
    };

    scroll(cx, -120., 0.);
    assert_eq!(placed(cx), (block, code - px(120.)));

    scroll(cx, 120., 0.);
    assert_eq!(placed(cx), (block, code));

    scroll(cx, 0., 120.);
    let (moved, still) = placed(cx);
    assert_eq!(
        (moved.x, moved.y, still),
        (block.x, block.y + px(120.), code)
    );
}

const GLYPH: f32 = 14.5 * 0.6;

fn on_glyph(block: Bounds<Pixels>, glyphs: f32, row: f32) -> gpui::Point<Pixels> {
    gpui::point(
        block.origin.x + px(GLYPH * glyphs),
        block.origin.y + px(24. * row + 12.),
    )
}

fn drag(cx: &mut VisualTestContext, from: gpui::Point<Pixels>, to: gpui::Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, gpui::Modifiers::none());
}

fn copied(cx: &mut VisualTestContext) -> Option<String> {
    cx.simulate_keystrokes("cmd-c");
    cx.read_from_clipboard().and_then(|item| item.text())
}

fn drawn(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} was not drawn"))
}

#[gpui::test]
fn dragging_from_one_paragraph_into_the_next_copies_only_the_text_passed_over_as_it_reads_without_markdown_marks(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("call **the** clinic\n\nthen book a slot"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let first = drawn(cx, "comment-text-1-1");
    let second = drawn(cx, "comment-text-1-2");

    drag(cx, on_glyph(first, 5.2, 0.), on_glyph(second, 9.2, 0.));

    assert_eq!(copied(cx).as_deref(), Some("the clinic\nthen book"));
}

#[gpui::test]
fn double_clicking_a_word_in_a_comment_selects_that_word_so_a_name_or_number_can_be_copied_in_one_go(
    cx: &mut TestAppContext,
) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("call 0123456 before noon"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let at = on_glyph(drawn(cx, "comment-text-1-1"), 6.2, 0.);

    cx.simulate_event(MouseDownEvent {
        position: at,
        button: MouseButton::Left,
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(at, MouseButton::Left, gpui::Modifiers::none());

    assert_eq!(copied(cx).as_deref(), Some("0123456"));
}

#[gpui::test]
fn cmd_a_selects_all_of_the_comment_clicked_last_and_none_of_the_others(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist")
            .comment("called once")
            .comment("# Plan\n\n- call **the** clinic\n- pay"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");

    let heading = on_glyph(drawn(cx, "comment-text-2-1"), 1.2, 0.);
    cx.simulate_click(heading, gpui::Modifiers::none());
    cx.simulate_keystrokes("cmd-a");

    assert_eq!(copied(cx).as_deref(), Some("Plan\ncall the clinic\npay"));
}

#[gpui::test]
fn a_drag_that_ends_on_a_link_selects_up_to_it_instead_of_opening_it(cx: &mut TestAppContext) {
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment("see [the form](https://example.com/form) first"),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let text = drawn(cx, "comment-text-1-1");

    drag(cx, on_glyph(text, 0.2, 0.), on_glyph(text, 8.2, 0.));

    assert_eq!(cx.opened_url(), None);
    assert_eq!(copied(cx).as_deref(), Some("see the "));
}

#[gpui::test]
fn holding_the_mouse_above_the_log_keeps_scrolling_and_selecting_so_a_long_comment_can_be_copied_whole(
    cx: &mut TestAppContext,
) {
    let notes: Vec<String> = (1..=80).map(|n| format!("note {n}")).collect();
    let server = Arc::new(FakeServer::with(vec![
        seed("Dentist").comment(Box::leak(notes.join("\n").into_boxed_str())),
    ]));
    let (view, cx) = open(cx, &server, true);
    open_comments_on(&view, cx, "Dentist");
    let log = drawn(cx, "thread-log");
    let text = drawn(cx, "comment-text-1-1");
    assert!(
        text.top() < log.top(),
        "the comment should not fit in the log"
    );
    let last_line = on_glyph(text, 0.2, 79.);
    assert!(log.contains(&last_line), "the log should start at its end");
    let above = gpui::point(last_line.x, log.top() - px(100.));

    cx.simulate_mouse_down(last_line, MouseButton::Left, gpui::Modifiers::none());
    cx.simulate_mouse_move(above, MouseButton::Left, gpui::Modifiers::none());
    let mut tops = Vec::new();
    for _ in 0..500 {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.run_until_parked();
        let top = drawn(cx, "comment-text-1-1").top();
        if tops.last() == Some(&top) {
            break;
        }
        tops.push(top);
    }
    cx.simulate_mouse_up(above, MouseButton::Left, gpui::Modifiers::none());

    assert!(
        tops.len() > 3,
        "the log stopped scrolling while the mouse was held still"
    );
    assert!(tops.windows(2).all(|pair| pair[1] > pair[0]));
    let expected = notes[..79].join("\n") + "\n";
    assert_eq!(copied(cx), Some(expected));
}
