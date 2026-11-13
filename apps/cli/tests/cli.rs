use std::process::{Command, Output};

use mockito::{Matcher, Mock, Server, ServerGuard};
use serde_json::{Value, json};

fn server() -> ServerGuard {
    let mut server = Server::new();
    server
        .mock("HEAD", "/")
        .match_header("cf-access-metadata-request", "true")
        .with_status(200)
        .create();
    server
}

fn nestodo(server: &ServerGuard, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nestodo"))
        .args(args)
        .env("NESTODO_BASE_URL", server.url())
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn api(server: &mut ServerGuard, method: &str, path: impl Into<Matcher>) -> Mock {
    server
        .mock(method, path)
        .match_header("origin", Matcher::Missing)
        .with_header("content-type", "application/json")
}

fn untouched(server: &mut ServerGuard, method: &str) -> Mock {
    server.mock(method, Matcher::Any).expect(0).create()
}

fn nestodo_sending_nothing(args: &[&str]) -> Output {
    let mut server = Server::new();
    let mocks = ["HEAD", "GET", "POST", "PATCH"].map(|method| untouched(&mut server, method));
    let output = nestodo(&server, args);
    for mock in mocks {
        mock.assert();
    }
    output
}

fn item(id: u64, parent: Option<u64>, position: usize, title: &str, done: bool) -> Value {
    json!({
        "id": id,
        "parentId": parent,
        "position": position,
        "title": title,
        "completedAt": done.then_some("2026-10-20 09:00:00"),
        "collapsed": false,
    })
}

fn stored(id: u64, parent: Option<u64>, position: usize, title: &str, done: bool) -> String {
    json!({ "todo": item(id, parent, position, title, done) }).to_string()
}

fn outline(server: &mut ServerGuard) -> Mock {
    let body = json!({ "todos": [
        item(1, None, 0, "Work", false),
        item(4, Some(1), 0, "v2 release", false),
        item(7, Some(4), 0, "Show the CLI list as a tree", false),
        item(8, Some(4), 1, "Check the Android IME toolbar", false),
        item(9, Some(4), 2, "Build the web outline screen", false),
        item(10, Some(4), 3, "Draft the release notes", true),
        item(5, Some(1), 1, "Prepare for the 1on1", false),
        item(2, None, 1, "Chores", false),
        item(18, None, 2, "Book the dentist", false),
    ]});
    api(server, "GET", "/api/v1/todos")
        .with_body(body.to_string())
        .create()
}

fn patch(server: &mut ServerGuard, id: u64, body: Value, response: String) -> Mock {
    api(server, "PATCH", format!("/api/v1/todos/{id}").as_str())
        .match_body(Matcher::Json(body))
        .with_body(response)
        .create()
}

#[test]
fn lists_the_whole_outline_as_a_tree_with_the_ids_other_commands_take() {
    let mut server = server();
    outline(&mut server);

    let output = nestodo(&server, &["list"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Home\n \
         1 [ ] ├── Work  0/2\n \
         4 [ ] │   ├── v2 release  1/4\n \
         7 [ ] │   │   ├── Show the CLI list as a tree\n \
         8 [ ] │   │   ├── Check the Android IME toolbar\n \
         9 [ ] │   │   ├── Build the web outline screen\n\
         10 [x] │   │   └── Draft the release notes\n \
         5 [ ] │   └── Prepare for the 1on1\n \
         2 [ ] ├── Chores\n\
         18 [ ] └── Book the dentist\n"
    );
}

#[test]
fn points_to_todo_list_when_an_id_does_not_exist() {
    let mut server = server();
    outline(&mut server);

    let output = nestodo(&server, &["show", "42"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("todo 42 not found; run `nestodo list`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn adds_at_the_end_of_home_without_an_origin_header_or_an_extra_request() {
    let mut server = server();
    let list = untouched(&mut server, "GET");
    let create = api(&mut server, "POST", "/api/v1/todos")
        .match_body(Matcher::Json(json!({ "title": "Buy an umbrella" })))
        .with_status(201)
        .with_body(stored(19, None, 3, "Buy an umbrella", false))
        .create();

    let output = nestodo(&server, &["add", "Buy an umbrella"]);

    assert!(output.status.success(), "{}", stderr(&output));
    create.assert();
    list.assert();
    assert_eq!(stdout(&output), "19 [ ] Buy an umbrella\n  under Home\n");
}

#[test]
fn trims_the_title_because_the_server_does_not_trim_titles() {
    let mut server = server();
    outline(&mut server);
    let create = api(&mut server, "POST", "/api/v1/todos")
        .match_body(Matcher::Json(json!({ "title": "Buy an umbrella" })))
        .with_status(201)
        .with_body(stored(19, None, 3, "Buy an umbrella", false))
        .create();
    let edit = patch(
        &mut server,
        5,
        json!({ "title": "Buy an umbrella" }),
        stored(5, Some(1), 1, "Buy an umbrella", false),
    );

    for args in [
        &["add", "  Buy an umbrella\n"][..],
        &["edit", "5", "--title", "  Buy an umbrella\n"],
    ] {
        let output = nestodo(&server, args);
        assert!(output.status.success(), "{}", stderr(&output));
    }
    create.assert();
    edit.assert();
}

#[test]
fn adds_under_a_parent_and_says_where_it_landed() {
    let mut server = server();
    outline(&mut server);
    let create = api(&mut server, "POST", "/api/v1/todos")
        .match_body(Matcher::Json(
            json!({ "title": "API reference", "parentId": 7 }),
        ))
        .with_status(201)
        .with_body(stored(19, Some(7), 0, "API reference", false))
        .create();

    let output = nestodo(&server, &["add", "API reference", "-p", "7"]);

    assert!(output.status.success(), "{}", stderr(&output));
    create.assert();
    assert_eq!(
        stdout(&output),
        "19 [ ] API reference\n  under Home › Work › v2 release › Show the CLI list as a tree\n"
    );
}

#[test]
fn adds_right_after_a_todo_by_asking_for_the_slot_behind_it_under_its_parent() {
    let mut server = server();
    outline(&mut server);
    let create = api(&mut server, "POST", "/api/v1/todos")
        .match_body(Matcher::Json(
            json!({ "title": "Write the release announcement", "parentId": 4, "index": 3 }),
        ))
        .with_status(201)
        .with_body(stored(
            19,
            Some(4),
            3,
            "Write the release announcement",
            false,
        ))
        .create();

    let output = nestodo(
        &server,
        &["add", "Write the release announcement", "--after", "9"],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    create.assert();
}

#[test]
fn rejects_a_blank_title_itself_because_the_server_now_accepts_blank_outline_rows() {
    for args in [&["add", "   "][..], &["edit", "5", "--title", ""]] {
        let output = nestodo_sending_nothing(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert_eq!(stderr(&output), "error: Enter a title\n", "{args:?}");
    }
}

#[test]
fn shows_the_servers_validation_message() {
    let mut server = server();
    api(&mut server, "POST", "/api/v1/todos")
        .with_status(400)
        .with_body(
            r#"{"error":"Validation failed","issues":[{"slot":"body","issues":[{"message":"Title must be 2000 characters or fewer","path":"title"}]}]}"#,
        )
        .create();

    let output = nestodo(&server, &["add", &"x".repeat(2001)]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: Title must be 2000 characters or fewer\n"
    );
}

#[test]
fn replaces_control_characters_in_server_errors_so_a_response_cannot_drive_the_terminal() {
    let mut server = server();
    api(&mut server, "GET", "/api/v1/todos")
        .with_status(500)
        .with_body(r#"{"error":"internal","message":"\u001b]0;pwned\u0007Try\nagain"}"#)
        .create();

    let output = nestodo(&server, &["list"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: the server returned 500: \u{FFFD}]0;pwned\u{FFFD}Try again\n"
    );
}

#[test]
fn moves_several_todos_behind_an_anchor_in_the_order_given() {
    let mut server = server();
    outline(&mut server);
    let first = patch(
        &mut server,
        10,
        json!({ "parentId": 4, "index": 2 }),
        stored(10, Some(4), 2, "Draft the release notes", true),
    );
    let second = patch(
        &mut server,
        7,
        json!({ "parentId": 4, "index": 2 }),
        stored(7, Some(4), 2, "Show the CLI list as a tree", false),
    );

    let output = nestodo(&server, &["mv", "10", "7", "--after", "8"]);

    assert!(output.status.success(), "{}", stderr(&output));
    first.assert();
    second.assert();
    assert_eq!(
        stdout(&output),
        "Moved 10 → Home › Work › v2 release\nMoved 7 → Home › Work › v2 release\n"
    );
}

#[test]
fn moving_after_a_later_sibling_sends_the_index_it_ends_up_at_once_taken_out() {
    let mut server = server();
    outline(&mut server);
    let moved = patch(
        &mut server,
        7,
        json!({ "parentId": 4, "index": 2 }),
        stored(7, Some(4), 2, "Show the CLI list as a tree", false),
    );

    let output = nestodo(&server, &["mv", "7", "--after", "9"]);

    assert!(output.status.success(), "{}", stderr(&output));
    moved.assert();
}

#[test]
fn moving_to_the_current_parent_sends_the_end_index_since_a_bare_parent_id_moves_nothing() {
    let mut server = server();
    outline(&mut server);
    let same_parent = patch(
        &mut server,
        7,
        json!({ "parentId": 4, "index": 3 }),
        stored(7, Some(4), 3, "Show the CLI list as a tree", false),
    );
    let new_parent = patch(
        &mut server,
        5,
        json!({ "parentId": 4, "index": 4 }),
        stored(5, Some(4), 4, "Prepare for the 1on1", false),
    );

    let output = nestodo(&server, &["mv", "7", "5", "--to", "4"]);

    assert!(output.status.success(), "{}", stderr(&output));
    same_parent.assert();
    new_parent.assert();
    assert_eq!(
        stdout(&output),
        "Moved 7 → Home › Work › v2 release\nMoved 5 → Home › Work › v2 release\n"
    );
}

#[test]
fn moves_into_a_todo_by_appending_and_to_the_top_level_by_following_a_top_level_todo() {
    let mut server = server();
    outline(&mut server);
    let into = patch(
        &mut server,
        18,
        json!({ "parentId": 2, "index": 0 }),
        stored(18, Some(2), 0, "Book the dentist", false),
    );
    let out = patch(
        &mut server,
        9,
        json!({ "parentId": null, "index": 2 }),
        stored(9, None, 2, "Build the web outline screen", false),
    );

    let output = nestodo(&server, &["mv", "18", "--to", "2"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Moved 18 → Home › Chores\n");

    let output = nestodo(&server, &["mv", "9", "--after", "2"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Moved 9 → Home\n");

    into.assert();
    out.assert();
}

#[test]
fn refuses_to_move_a_todo_into_its_own_subtree_before_changing_anything() {
    let mut server = server();
    outline(&mut server);
    let none = untouched(&mut server, "PATCH");

    let output = nestodo(&server, &["mv", "18", "1", "--to", "7"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: cannot move 1 into its own subtree\n"
    );
    none.assert();
}

#[test]
fn needs_somewhere_to_move_to() {
    let mut server = server();
    let none = untouched(&mut server, "PATCH");

    let output = nestodo(&server, &["mv", "7"]);

    assert_eq!(output.status.code(), Some(2));
    none.assert();
}

#[test]
fn indents_to_the_end_of_the_todo_above_and_outdents_to_right_after_the_parent() {
    let mut server = server();
    outline(&mut server);
    let indent = patch(
        &mut server,
        9,
        json!({ "parentId": 8, "index": 0 }),
        stored(9, Some(8), 0, "Build the web outline screen", false),
    );
    let indent_after_children = patch(
        &mut server,
        5,
        json!({ "parentId": 4, "index": 4 }),
        stored(5, Some(4), 4, "Prepare for the 1on1", false),
    );
    let outdent = patch(
        &mut server,
        8,
        json!({ "parentId": 1, "index": 1 }),
        stored(8, Some(1), 1, "Check the Android IME toolbar", false),
    );

    let output = nestodo(&server, &["indent", "9"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "9 [ ] Build the web outline screen\n  under Home › Work › v2 release › Check the Android IME toolbar\n"
    );

    let output = nestodo(&server, &["indent", "5"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "5 [ ] Prepare for the 1on1\n  under Home › Work › v2 release\n"
    );

    let output = nestodo(&server, &["outdent", "8"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "8 [ ] Check the Android IME toolbar\n  under Home › Work\n"
    );

    indent.assert();
    indent_after_children.assert();
    outdent.assert();
}

#[test]
fn marks_done_and_undone_through_completed_so_the_server_keeps_the_first_completion_time() {
    let mut server = server();
    outline(&mut server);
    let done = patch(
        &mut server,
        7,
        json!({ "completed": true }),
        stored(7, Some(4), 0, "Show the CLI list as a tree", true),
    );
    let undone = patch(
        &mut server,
        10,
        json!({ "completed": false }),
        stored(10, Some(4), 3, "Draft the release notes", false),
    );

    let output = nestodo(&server, &["done", "7"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "7 [x] Show the CLI list as a tree\n");
    assert!(nestodo(&server, &["undo", "10"]).status.success());
    done.assert();
    undone.assert();
}

#[test]
fn refuses_to_complete_open_todos_under_a_todo_unless_told_to_with_r() {
    let mut server = server();
    outline(&mut server);
    let none = untouched(&mut server, "PATCH");

    let output = nestodo(&server, &["done", "18", "4"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: 4 has 3 open todos under it; pass -r to mark them done too\n"
    );
    none.assert();
}

#[test]
fn completes_a_whole_branch_with_one_request_because_the_server_completes_what_is_under_it() {
    let mut server = server();
    outline(&mut server);
    let parent = patch(
        &mut server,
        4,
        json!({ "completed": true }),
        stored(4, Some(1), 0, "v2 release", true),
    );
    let children = api(
        &mut server,
        "PATCH",
        Matcher::Regex(r"^/api/v1/todos/(7|8|9|10)$".into()),
    )
    .expect(0)
    .create();

    let output = nestodo(&server, &["done", "-r", "4"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "4 [x] v2 release  4/4\n");
    parent.assert();
    children.assert();
}

#[test]
fn checks_every_id_before_changing_any_so_a_typo_does_not_leave_half_the_work_done() {
    let mut server = server();
    outline(&mut server);
    let none = untouched(&mut server, "PATCH");

    let output = nestodo(&server, &["done", "7", "42"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("todo 42 not found"),
        "{}",
        stderr(&output)
    );
    none.assert();
}

#[test]
fn edits_only_the_title() {
    let mut server = server();
    outline(&mut server);
    let edit = patch(
        &mut server,
        5,
        json!({ "title": "Prepare for the 1on1" }),
        stored(5, Some(1), 1, "Prepare for the 1on1", false),
    );

    let output = nestodo(&server, &["edit", "5", "--title", "Prepare for the 1on1"]);

    assert!(output.status.success(), "{}", stderr(&output));
    edit.assert();
    assert_eq!(stdout(&output), "5 [ ] Prepare for the 1on1\n");
}

#[test]
fn edit_without_a_new_title_says_to_pass_one_and_sends_nothing() {
    let output = nestodo_sending_nothing(&["edit", "3"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).starts_with(
            "error: the following required arguments were not provided:\n  --title <TITLE>\n"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn deletes_a_todo_without_asking_for_confirmation_like_the_web_ui() {
    let mut server = server();
    outline(&mut server);
    let delete = api(&mut server, "DELETE", "/api/v1/todos/18")
        .with_status(204)
        .create();

    let output = nestodo(&server, &["rm", "18"]);

    assert!(output.status.success(), "{}", stderr(&output));
    delete.assert();
    assert_eq!(stdout(&output), "Deleted todo 18.\n");
}

#[test]
fn refuses_to_delete_todos_under_a_todo_unless_told_to_with_r() {
    let mut server = server();
    outline(&mut server);
    let none = untouched(&mut server, "DELETE");

    let output = nestodo(&server, &["rm", "18", "1"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: 1 has 6 todos under it; pass -r to delete them too\n"
    );
    none.assert();
}

#[test]
fn deletes_a_whole_branch_with_one_request_because_the_server_takes_the_subtree() {
    let mut server = server();
    outline(&mut server);
    let branch = api(&mut server, "DELETE", "/api/v1/todos/1?recursive=true")
        .with_status(204)
        .create();
    let below = api(
        &mut server,
        "DELETE",
        Matcher::Regex(r"^/api/v1/todos/(4|5|7|8|9|10)$".into()),
    )
    .expect(0)
    .create();

    let output = nestodo(&server, &["rm", "-r", "1", "7"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Deleted todo 1 and 6 under it.\n");
    branch.assert();
    below.assert();
}

#[test]
fn does_not_follow_a_redirect_to_the_access_login_page() {
    let mut server = server();
    api(&mut server, "GET", "/api/v1/todos")
        .with_status(302)
        .with_header(
            "location",
            "https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com",
        )
        .create();

    let output = nestodo(&server, &["list"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("refused the request (302)"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn refuses_plain_http_to_a_remote_host() {
    let output = Command::new(env!("CARGO_BIN_EXE_nestodo"))
        .args(["--base-url", "http://nestodo.example.com", "list"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn asks_for_a_server_instead_of_signing_in_to_one_the_user_did_not_choose() {
    for command in ["list", "login", "logout"] {
        let output = Command::new(env!("CARGO_BIN_EXE_nestodo"))
            .arg(command)
            .env_remove("NESTODO_BASE_URL")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{command}");
        assert!(
            stderr(&output).contains("NESTODO_BASE_URL"),
            "{command}: {}",
            stderr(&output)
        );
    }
}
