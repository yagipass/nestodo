use std::process::{Command, Output};

fn nestodo(args: &[&str]) -> Output {
    let base_url = std::env::var("NESTODO_E2E_BASE_URL").expect(
        "set NESTODO_E2E_BASE_URL to a running `vp dev` server, e.g. http://localhost:5173",
    );
    Command::new(env!("CARGO_BIN_EXE_nestodo"))
        .args(args)
        .env("NESTODO_BASE_URL", base_url)
        .env("NO_COLOR", "1")
        .output()
        .unwrap()
}

fn ok(args: &[&str]) -> String {
    let output = nestodo(args);
    assert!(
        output.status.success(),
        "nestodo {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn failure(args: &[&str]) -> String {
    let output = nestodo(args);
    assert_eq!(
        output.status.code(),
        Some(1),
        "nestodo {args:?} should fail"
    );
    String::from_utf8(output.stderr).unwrap()
}

fn id_of(row: &str) -> String {
    row.split_whitespace().next().unwrap().to_string()
}

fn tree_under(id: &str) -> Vec<String> {
    ok(&["list", id])
        .lines()
        .skip(1)
        .map(|line| line.trim_start().split_once(' ').unwrap().1.to_string())
        .collect()
}

#[test]
#[ignore = "needs a running `vp dev` server; run with NESTODO_E2E_BASE_URL=... cargo test -- --ignored"]
fn an_outline_can_be_built_rearranged_completed_and_pruned_against_the_real_server() {
    let title = format!("cli e2e {}", std::process::id());
    let root = id_of(&ok(&["add", &title]));
    let a = id_of(&ok(&["add", "a", "-p", &root]));
    let b = id_of(&ok(&["add", "b", "-p", &root]));
    let added = ok(&["add", "c", "--after", &a]);
    assert!(
        added.ends_with(&format!("  under Home › {title}\n")),
        "{added}"
    );
    let c = id_of(&added);
    assert_eq!(tree_under(&root), ["[ ] ├── a", "[ ] ├── c", "[ ] └── b"]);

    ok(&["mv", &a, "--after", &c]);
    assert_eq!(tree_under(&root), ["[ ] ├── c", "[ ] ├── a", "[ ] └── b"]);

    ok(&["indent", &a]);
    assert_eq!(
        tree_under(&root),
        ["[ ] ├── c  0/1", "[ ] │   └── a", "[ ] └── b"]
    );
    ok(&["outdent", &a]);
    assert_eq!(tree_under(&root), ["[ ] ├── c", "[ ] ├── a", "[ ] └── b"]);

    ok(&["mv", &b, &c, "--to", &a]);
    assert_eq!(
        tree_under(&root),
        ["[ ] └── a  0/2", "[ ]     ├── b", "[ ]     └── c"]
    );

    assert_eq!(
        failure(&["done", &a]),
        format!("error: {a} has 2 open todos under it; pass -r to mark them done too\n")
    );
    ok(&["done", "-r", &a]);
    assert_eq!(
        tree_under(&root),
        ["[x] └── a  2/2", "[x]     ├── b", "[x]     └── c"],
        "finishing a todo finishes the todos under it"
    );
    assert_eq!(
        ok(&["list", &root, "--active"]),
        format!("Home › {title}  (#{root})\nNo todos here yet.\n")
    );
    ok(&["undo", &a]);
    assert_eq!(
        tree_under(&root),
        ["[ ] └── a  2/2", "[x]     ├── b", "[x]     └── c"],
        "reopening a todo leaves the todos under it finished"
    );

    ok(&["edit", &b, "--title", "b renamed"]);
    let shown = ok(&["show", &b]);
    assert!(shown.contains("Title:     b renamed"), "{shown}");
    assert!(
        shown.contains(&format!("Parent:    Home › {title} › a")),
        "{shown}"
    );

    assert_eq!(
        failure(&["rm", &a]),
        format!("error: {a} has 2 todos under it; pass -r to delete them too\n")
    );
    assert_eq!(
        ok(&["rm", "-r", &a]),
        format!("Deleted todo {a} and 2 under it.\n")
    );
    assert!(failure(&["show", &b]).contains(&format!("todo {b} not found")));
    ok(&["rm", &root]);
}

#[test]
#[ignore = "needs a running `vp dev` server; run with NESTODO_E2E_BASE_URL=... cargo test -- --ignored"]
fn blank_titles_are_refused_here_and_long_ones_by_the_server() {
    assert_eq!(failure(&["add", "   "]), "error: Enter a title\n");
    let long = "x".repeat(2001);
    assert_eq!(
        failure(&["add", &long]),
        "error: Title must be 2000 characters or fewer\n"
    );
}
