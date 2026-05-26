//! Tests for the `memory` feature.
//!
//! Run with: cargo test --features memory
//!
//! Each test injects its own temp `root` via the public `Mem` fields, so they
//! need no env, no clock, no rig, and run fully in parallel. Paths are built
//! from the public `root` field (Mem's own helpers are private).

use crate::agent::memory::{MAX_INJECT_BYTES, Mem, WriteMode, WriteTarget};
use std::fs;
use std::path::PathBuf;

fn fresh(tag: &str) -> Mem {
    let root = std::env::temp_dir().join(format!(
        "zsmem-{}-{}-{:?}",
        tag,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    Mem {
        root,
        today: "2026-05-25".into(),
        yesterday: "2026-05-24".into(),
    }
}
fn cleanup(m: &Mem) {
    let _ = fs::remove_dir_all(&m.root);
}
fn memory_md(m: &Mem) -> PathBuf {
    m.root.join("MEMORY.md")
}
fn scratchpad(m: &Mem) -> PathBuf {
    m.root.join("SCRATCHPAD.md")
}
fn daily(m: &Mem, d: &str) -> PathBuf {
    m.root.join("daily").join(format!("{d}.md"))
}

// ---- store: write / context_block -------------------------------------------

#[test]
fn empty_store_returns_none() {
    let m = fresh("empty");
    assert!(m.context_block().is_none());
    cleanup(&m);
}

#[test]
fn long_term_always_injected() {
    let m = fresh("lt");
    m.write(
        WriteTarget::LongTerm,
        "- never push to main",
        WriteMode::Append,
        None,
    )
    .unwrap();
    assert!(m.context_block().unwrap().contains("never push to main"));
    cleanup(&m);
}

#[test]
fn append_keeps_single_trailing_newline_and_overwrite_replaces() {
    let m = fresh("w");
    m.write(WriteTarget::LongTerm, "a", WriteMode::Append, None)
        .unwrap();
    m.write(WriteTarget::LongTerm, "b", WriteMode::Append, None)
        .unwrap();
    assert_eq!(fs::read_to_string(memory_md(&m)).unwrap(), "a\nb\n");
    m.write(WriteTarget::LongTerm, "new", WriteMode::Overwrite, None)
        .unwrap();
    assert_eq!(fs::read_to_string(memory_md(&m)).unwrap(), "new");
    cleanup(&m);
}

#[test]
fn append_to_file_without_trailing_newline_inserts_one() {
    let m = fresh("nl");
    fs::write(memory_md(&m), "no newline").unwrap();
    m.write(WriteTarget::LongTerm, "next", WriteMode::Append, None)
        .unwrap();
    assert_eq!(
        fs::read_to_string(memory_md(&m)).unwrap(),
        "no newline\nnext\n"
    );
    cleanup(&m);
}

#[test]
fn scratchpad_write_then_inject_open_items_only() {
    let m = fresh("sp");
    m.write(
        WriteTarget::Scratchpad,
        "- [ ] first task",
        WriteMode::Append,
        None,
    )
    .unwrap();
    m.write(
        WriteTarget::Scratchpad,
        "- [x] closed task",
        WriteMode::Append,
        None,
    )
    .unwrap();
    assert!(scratchpad(&m).exists());
    let b = m.context_block().unwrap();
    assert!(b.contains("first task"));
    assert!(!b.contains("closed task"));
    m.write(
        WriteTarget::Scratchpad,
        "- [ ] only this",
        WriteMode::Overwrite,
        None,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(scratchpad(&m)).unwrap(),
        "- [ ] only this"
    );
    cleanup(&m);
}

#[test]
fn scratchpad_filter_handles_indent_and_star_bullets() {
    let m = fresh("spf");
    fs::write(
        scratchpad(&m),
        "- [ ] open one\n- [x] closed\n  - [ ] indented open\n* [ ] star open\nplain line\n",
    )
    .unwrap();
    let b = m.context_block().unwrap();
    assert!(b.contains("open one") && b.contains("indented open") && b.contains("star open"));
    assert!(!b.contains("closed") && !b.contains("plain line"));
    cleanup(&m);
}

#[test]
fn daily_order_yesterday_before_today() {
    let m = fresh("ord");
    m.write(WriteTarget::Daily, "TODAYMARK", WriteMode::Append, None)
        .unwrap();
    fs::create_dir_all(daily(&m, &m.yesterday).parent().unwrap()).unwrap();
    fs::write(daily(&m, &m.yesterday), "YESTMARK").unwrap();
    let b = m.context_block().unwrap();
    assert!(b.find("YESTMARK").unwrap() < b.find("TODAYMARK").unwrap());
    assert!(b.contains("(today)"));
    cleanup(&m);
}

#[test]
fn notes_never_injected_but_searchable() {
    let m = fresh("note");
    m.write(
        WriteTarget::Note,
        "jose for edge compat",
        WriteMode::Overwrite,
        Some("auth"),
    )
    .unwrap();
    assert!(!m.context_block().unwrap_or_default().contains("jose"));
    assert!(m.search("jose").iter().any(|h| h.contains("jose")));
    cleanup(&m);
}

#[test]
fn note_name_traversal_rejected() {
    let m = fresh("trav");
    for bad in ["../escape", "sub/dir", ".hidden", "a.b", "", "  "] {
        assert!(
            m.write(WriteTarget::Note, "x", WriteMode::Overwrite, Some(bad))
                .is_err(),
            "should reject note name {bad:?}"
        );
    }
    assert!(
        m.write(
            WriteTarget::Note,
            "x",
            WriteMode::Overwrite,
            Some("good-name")
        )
        .is_ok()
    );
    cleanup(&m);
}

#[test]
fn context_block_truncates_cjk_without_panic() {
    let m = fresh("cjk");
    m.write(
        WriteTarget::LongTerm,
        &"記憶實作".repeat(MAX_INJECT_BYTES),
        WriteMode::Overwrite,
        None,
    )
    .unwrap();
    let b = m.context_block().unwrap();
    assert!(b.contains("[memory truncated]"));
    assert!(b.len() <= MAX_INJECT_BYTES + 128);
    cleanup(&m);
}

// ---- search (single-term, Vec<String>) --------------------------------------

#[test]
fn search_returns_surrounding_context_and_merges() {
    let m = fresh("ctx");
    m.write(
        WriteTarget::Note,
        "intro\nblah\nwe chose jose\nbecause edge is incompatible\nunrelated tail",
        WriteMode::Overwrite,
        Some("auth"),
    )
    .unwrap();
    let e = m
        .search("jose")
        .into_iter()
        .find(|h| h.contains("auth"))
        .unwrap();
    assert!(e.contains("we chose jose"));
    assert!(e.contains("because edge is incompatible")); // +1 line
    assert!(e.contains("blah")); // -1 line
    assert!(!e.contains("unrelated tail")); // outside window
    cleanup(&m);
}

#[test]
fn search_caps_at_max_blocks() {
    let m = fresh("cap");
    let body = (0..5)
        .map(|i| format!("hit{i}\na\nb\nc\nd"))
        .collect::<Vec<_>>()
        .join("\n");
    m.write(WriteTarget::Note, &body, WriteMode::Overwrite, Some("many"))
        .unwrap();
    let e = m
        .search("hit")
        .into_iter()
        .find(|h| h.contains("many"))
        .unwrap();
    assert!(e.contains("hit0") && e.contains("hit1") && e.contains("hit2"));
    assert!(!e.contains("hit3") && !e.contains("hit4")); // capped at 3
    cleanup(&m);
}

#[test]
fn search_filename_match_falls_back_to_preview() {
    let m = fresh("fn");
    m.write(
        WriteTarget::Note,
        "first line\nsecond line",
        WriteMode::Overwrite,
        Some("websocket-fix"),
    )
    .unwrap();
    let e = m
        .search("websocket")
        .into_iter()
        .find(|h| h.contains("websocket-fix"))
        .expect("hit");
    assert!(e.contains("(filename match)"));
    assert!(e.contains("first line"));
    cleanup(&m);
}

#[test]
fn search_clean_miss_returns_empty() {
    let m = fresh("miss");
    m.write(
        WriteTarget::Note,
        "body text",
        WriteMode::Overwrite,
        Some("misc"),
    )
    .unwrap();
    assert!(m.search("nonexistent-xyz").is_empty());
    cleanup(&m);
}

#[test]
fn search_is_literal_not_regex() {
    let m = fresh("lit");
    m.write(
        WriteTarget::Note,
        "formula a+b=c",
        WriteMode::Overwrite,
        Some("math"),
    )
    .unwrap();
    assert!(m.search("a+b").iter().any(|h| h.contains("a+b=c")));
    cleanup(&m);
}
