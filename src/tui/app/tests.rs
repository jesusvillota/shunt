use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use super::*;
use crate::tui::model::{AccountDto, PoolResponse, ProviderDto};

fn key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn code(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn acct(name: &str, headroom: Option<i64>) -> AccountDto {
    AccountDto {
        name: name.into(),
        account_ref: Some(format!("ref-{name}")),
        has_state: true,
        headroom_secs: headroom,
        ..AccountDto::default()
    }
}

fn provider(name: &str, auth: &str, accounts: Vec<AccountDto>) -> ProviderDto {
    ProviderDto {
        provider: name.into(),
        auth: Some(auth.into()),
        accounts,
    }
}

fn app_with(providers: Vec<ProviderDto>) -> App {
    let mut app = App::new("http://x".into());
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers,
        sort_by_reset: false,
    })));
    app
}

fn claude(accounts: Vec<AccountDto>) -> App {
    app_with(vec![provider("claude", "claude_oauth", accounts)])
}

fn sel(app: &App) -> Option<(String, Option<String>)> {
    app.selected
        .as_ref()
        .map(|s| (s.provider.clone(), s.account.clone()))
}

#[test]
fn nothing_is_selected_until_the_operator_picks_a_line() {
    let mut app = claude(vec![acct("a", None), acct("b", None)]);
    assert_eq!(app.selected, None);
    assert_eq!(app.selected_index(), None);
    app.on_key(key('j'));
    assert_eq!(
        sel(&app),
        Some(("claude".into(), None)),
        "first line is the provider header"
    );
    app.on_key(key('j'));
    assert_eq!(sel(&app).unwrap().1.as_deref(), Some("ref-a"));
}

#[test]
fn escape_deselects_and_q_quits() {
    let mut app = claude(vec![acct("a", None)]);
    app.on_key(key('j'));
    assert_eq!(app.on_key(code(KeyCode::Esc)), Effect::None);
    assert_eq!(app.selected, None);
    assert_eq!(app.on_key(key('q')), Effect::Quit);
}

#[test]
fn clicking_a_line_selects_it_and_clicking_anywhere_else_deselects() {
    let mut app = claude(vec![acct("a", None), acct("b", None)]);
    app.list_area = Rect::new(0, 3, 80, 10);
    app.hits = vec![
        Some(Sel {
            provider: "claude".into(),
            account: None,
        }),
        Some(Sel {
            provider: "claude".into(),
            account: Some("ref-a".into()),
        }),
        Some(Sel {
            provider: "claude".into(),
            account: Some("ref-b".into()),
        }),
        None,
    ];
    let click = |column, row| MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    app.on_mouse(click(5, 5));
    assert_eq!(sel(&app).unwrap().1.as_deref(), Some("ref-b"));
    app.on_mouse(click(5, 6)); // blank line below the last account
    assert_eq!(app.selected, None);
    app.on_mouse(click(5, 4));
    assert!(app.selected.is_some());
    app.on_mouse(click(5, 1)); // above the list
    assert_eq!(app.selected, None);
}

#[test]
fn a_poll_that_removes_the_selected_account_clears_the_selection() {
    let mut app = claude(vec![acct("a", None), acct("b", None)]);
    app.on_key(key('j'));
    app.on_key(key('j'));
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers: vec![provider("claude", "claude_oauth", vec![acct("b", None)])],
        sort_by_reset: false,
    })));
    assert_eq!(app.selected, None);
}

#[test]
fn pause_needs_an_account_and_targets_its_ref() {
    let mut app = claude(vec![acct("a", None)]);
    assert_eq!(app.on_key(key('p')), Effect::None);
    assert!(matches!(app.current_notice(), Some(Notice::Info(_))));
    app.on_key(key('j')); // header
    assert_eq!(
        app.on_key(key('p')),
        Effect::None,
        "a header is not an account"
    );
    app.on_key(key('j'));
    assert_eq!(
        app.on_key(key('p')),
        Effect::SetPaused {
            provider: "claude".into(),
            account_ref: "ref-a".into(),
            label: "a".into(),
            paused: true
        }
    );
}

#[test]
fn provider_switch_pauses_what_is_live_and_resumes_only_those() {
    let mut deliberate = acct("deliberate", None);
    deliberate.paused = true;
    let mut app = claude(vec![acct("a", None), deliberate, acct("b", None)]);
    app.on_key(key('j'));
    let Effect::SetProvider {
        on, account_refs, ..
    } = app.on_key(key('o'))
    else {
        panic!("expected a provider switch");
    };
    assert!(!on);
    assert_eq!(
        account_refs,
        ["ref-a", "ref-b"],
        "the already-paused account is left alone"
    );

    // Everything is now paused: the provider reads as off, and `o` resumes only
    // the two the switch paused.
    let paused = |n: &str| AccountDto {
        paused: true,
        ..acct(n, None)
    };
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers: vec![provider(
            "claude",
            "claude_oauth",
            vec![paused("a"), paused("deliberate"), paused("b")],
        )],
        sort_by_reset: false,
    })));
    let Effect::SetProvider {
        on, account_refs, ..
    } = app.on_key(key('o'))
    else {
        panic!("expected a provider switch");
    };
    assert!(on);
    assert_eq!(account_refs, ["ref-a", "ref-b"]);
}

#[test]
fn switching_a_provider_on_without_memory_resumes_every_paused_account() {
    let paused = |n: &str| AccountDto {
        paused: true,
        ..acct(n, None)
    };
    let mut app = claude(vec![paused("a"), paused("b")]);
    app.on_key(key('j'));
    let Effect::SetProvider {
        on, account_refs, ..
    } = app.on_key(key('o'))
    else {
        panic!("expected a provider switch");
    };
    assert!(on);
    assert_eq!(account_refs, ["ref-a", "ref-b"]);
}

#[test]
fn balanced_to_custom_starts_from_the_live_order() {
    let mut app = claude(vec![
        acct("low", Some(10)),
        acct("high", Some(9_000)),
        acct("mid", Some(500)),
    ]);
    app.on_key(key('j'));
    let effect = app.on_key(key('m'));
    assert_eq!(
        effect,
        Effect::SetRanks {
            provider: "claude".into(),
            ranks: vec![("high".into(), 1), ("mid".into(), 2), ("low".into(), 3)],
            pool_names: vec!["low".into(), "high".into(), "mid".into()],
        }
    );
    // The list already shows the explicit order, before the gateway reloads.
    let provider = app.snapshot.provider("claude").unwrap();
    assert_eq!(provider.mode(), RankMode::Custom);
    let names: Vec<_> = provider
        .ordered(false)
        .iter()
        .map(|(_, r)| r.account.name.clone())
        .collect();
    assert_eq!(names, ["high", "mid", "low"]);
}

#[test]
fn custom_to_balanced_clears_ranks() {
    let mut a = acct("a", None);
    a.priority = Some(1);
    let mut b = acct("b", None);
    b.priority = Some(2);
    let mut app = claude(vec![a, b]);
    app.on_key(key('j'));
    assert_eq!(
        app.on_key(key('m')),
        Effect::ClearRanks {
            provider: "claude".into()
        }
    );
    assert_eq!(
        app.snapshot.provider("claude").unwrap().mode(),
        RankMode::Balanced
    );
}

#[test]
fn ranking_needs_two_accounts() {
    let mut app = claude(vec![acct("only", None)]);
    app.on_key(key('j'));
    assert_eq!(app.on_key(key('m')), Effect::None);
    assert!(matches!(app.current_notice(), Some(Notice::Info(t)) if t.contains("two accounts")));
}

fn custom_app() -> App {
    let ranked = |n: &str, p: u32| AccountDto {
        priority: Some(p),
        ..acct(n, None)
    };
    claude(vec![ranked("a", 1), ranked("b", 2), ranked("c", 3)])
}

#[test]
fn shift_arrows_and_capitals_move_an_account_through_the_ranking() {
    let mut app = custom_app();
    app.on_key(key('j'));
    app.on_key(key('j')); // account a (rank 1)
    app.on_key(key('j')); // account b (rank 2)
    let Effect::SetRanks { ranks, .. } =
        app.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT))
    else {
        panic!("expected a ranking write");
    };
    assert_eq!(
        ranks,
        vec![("b".into(), 1), ("a".into(), 2), ("c".into(), 3)]
    );
    // The cursor stays on b, now first, and the next move is computed from the new order.
    assert_eq!(sel(&app).unwrap().1.as_deref(), Some("ref-b"));
    assert_eq!(app.selected_index(), Some(1));
    let Effect::SetRanks { ranks, .. } = app.on_key(key('J')) else {
        panic!("expected a ranking write");
    };
    assert_eq!(
        ranks,
        vec![("a".into(), 1), ("b".into(), 2), ("c".into(), 3)]
    );
}

#[test]
fn moving_past_either_end_does_nothing_and_balanced_mode_refuses() {
    let mut app = custom_app();
    app.on_key(key('j'));
    app.on_key(key('j')); // rank 1
    assert_eq!(app.on_key(key('K')), Effect::None);

    let mut balanced = claude(vec![acct("a", None), acct("b", None)]);
    balanced.on_key(key('j'));
    balanced.on_key(key('j'));
    assert_eq!(balanced.on_key(key('J')), Effect::None);
    assert!(matches!(balanced.current_notice(), Some(Notice::Info(t)) if t.contains("custom")));
}

#[test]
fn the_overlay_yields_to_the_gateway_once_it_reports_the_same_ranking() {
    let mut app = claude(vec![acct("a", Some(1)), acct("b", Some(2))]);
    app.on_key(key('j'));
    app.on_key(key('m'));
    assert!(app.overlay.is_some());
    // A stale poll (reload not yet seen) keeps the overlay applied…
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers: vec![provider(
            "claude",
            "claude_oauth",
            vec![acct("a", Some(1)), acct("b", Some(2))],
        )],
        sort_by_reset: false,
    })));
    assert!(app.overlay.is_some());
    assert_eq!(
        app.snapshot.provider("claude").unwrap().mode(),
        RankMode::Custom
    );
    // …and the poll that reflects the write retires it.
    let ranked = |n: &str, p: u32, h: i64| AccountDto {
        priority: Some(p),
        ..acct(n, Some(h))
    };
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers: vec![provider(
            "claude",
            "claude_oauth",
            vec![ranked("a", 2, 1), ranked("b", 1, 2)],
        )],
        sort_by_reset: false,
    })));
    assert!(app.overlay.is_none());
}

#[test]
fn add_opens_for_the_selected_provider_and_skips_ones_the_api_cannot_add_to() {
    let mut app = app_with(vec![
        provider("kimi", "kimi_oauth", vec![acct("k", None)]),
        provider("codex", "chatgpt_oauth", vec![acct("c", None)]),
        provider("claude", "claude_oauth", vec![acct("a", None)]),
    ]);
    app.on_key(key('a'));
    let dialog = app.dialog.as_ref().expect("dialog opens");
    assert_eq!(
        dialog
            .choices
            .iter()
            .map(|t| t.provider.as_str())
            .collect::<Vec<_>>(),
        ["codex", "claude"]
    );
    assert_eq!(dialog.step, crate::tui::add::Step::Provider { at: 0 });
    assert_eq!(app.on_key(code(KeyCode::Esc)), Effect::MouseCapture(true));
    assert!(app.dialog.is_none());

    // With a provider selected the dialog goes straight to the name.
    app.on_key(key('j'));
    app.on_key(key('j'));
    app.on_key(key('j'));
    app.on_key(key('j'));
    assert_eq!(app.selected.as_ref().unwrap().provider, "codex");
    app.on_key(key('a'));
    assert_eq!(app.dialog.as_ref().unwrap().target().provider, "codex");

    let mut kimi_only = app_with(vec![provider("kimi", "kimi_oauth", vec![])]);
    kimi_only.on_key(key('a'));
    assert!(kimi_only.dialog.is_none());
    assert!(matches!(kimi_only.current_notice(), Some(Notice::Info(_))));
}

#[test]
fn a_failed_poll_keeps_the_last_snapshot() {
    let mut app = claude(vec![acct("a", None)]);
    app.on_poll(Err("gateway unreachable".into()));
    assert_eq!(app.entries().len(), 2);
    assert_eq!(app.poll_error.as_deref(), Some("gateway unreachable"));
    app.on_poll(Ok(Snapshot::default()));
    assert!(app.poll_error.is_none());
}

#[test]
fn help_swallows_the_key_that_closes_it() {
    let mut app = claude(vec![acct("a", None)]);
    app.on_key(key('?'));
    assert!(app.show_help);
    assert_eq!(app.on_key(key('q')), Effect::None);
    assert!(!app.show_help);
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.on_key(ctrl_c), Effect::Quit);
}
