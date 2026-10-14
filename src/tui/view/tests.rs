use ratatui::{backend::TestBackend, style::Modifier, Terminal};

use super::*;
use crate::tui::{
    add::Target,
    model::{AccountDto, PoolResponse, ProviderDto, Snapshot},
};

const NOW: u64 = 1_000;

fn draw(app: &mut App, width: u16, height: u16) -> (String, Terminal<TestBackend>) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| render(f, app, NOW)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, terminal)
}

fn has_reversed_cell(terminal: &Terminal<TestBackend>) -> bool {
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .any(|cell| cell.modifier.contains(Modifier::REVERSED))
}

fn acct(name: &str) -> AccountDto {
    AccountDto {
        name: name.into(),
        account_ref: Some(format!("ref-{name}")),
        has_state: true,
        ..AccountDto::default()
    }
}

fn app(providers: Vec<ProviderDto>) -> App {
    let mut app = App::new("http://127.0.0.1:3001".into());
    app.on_poll(Ok(Snapshot::from_response(PoolResponse {
        providers,
        sort_by_reset: false,
        ..Default::default()
    })));
    app
}

fn provider(name: &str, auth: &str, accounts: Vec<AccountDto>) -> ProviderDto {
    ProviderDto {
        provider: name.into(),
        auth: Some(auth.into()),
        accounts,
        sort_by_weekly_reset: None,
    }
}

#[test]
fn until_formats_coarsely() {
    assert_eq!(until(1_000, 1_000), "now");
    assert_eq!(until(900, 1_000), "now");
    assert_eq!(until(1_030, 1_000), "1m");
    assert_eq!(until(1_000 + 3_600, 1_000), "1h");
    assert_eq!(until(1_000 + 3_660, 1_000), "1h 1m");
    assert_eq!(until(1_000 + 86_400 + 7_200, 1_000), "1d 2h");
}

#[test]
fn shows_each_provider_separately_with_ranks_and_both_reset_times() {
    let mut first = acct("work");
    first.plan = Some("max".into());
    first.utilization_5h = Some(0.5);
    first.reset_5h = Some(NOW + 2 * 3600 + 14 * 60);
    first.utilization_7d = Some(0.25);
    first.reset_7d = Some(NOW + 3 * 86_400 + 4 * 3600);
    let mut app = app(vec![
        provider("anthropic", "claude_oauth", vec![first, acct("spare")]),
        provider("codex", "chatgpt_oauth", vec![acct("gpt")]),
    ]);
    let (screen, _) = draw(&mut app, 110, 20);
    assert!(screen.contains("anthropic"), "{screen}");
    assert!(screen.contains("codex"), "{screen}");
    assert!(screen.contains("ON"), "{screen}");
    assert!(
        screen.contains("ranking: balanced     · 2 accounts"),
        "{screen}"
    );
    assert!(screen.contains("50% · 2h 14m"), "5h reset time:\n{screen}");
    assert!(screen.contains("25% · 3d 4h"), "7d reset time:\n{screen}");
    // No Peak column, no summary counters.
    assert!(!screen.contains("Peak"), "{screen}");
    assert!(!screen.contains("selectable"), "{screen}");
    let anthropic_at = screen.find("anthropic").unwrap();
    let codex_at = screen.find("codex").unwrap();
    assert!(anthropic_at < codex_at && screen[anthropic_at..codex_at].contains("spare"));
}

fn family_account() -> AccountDto {
    serde_json::from_str(r#"{"name":"weekly","account_ref":"ref-weekly","has_state":true,"quota_buckets":[{"label":"Gemini Models · weekly","remaining":0.0,"reset_time":"1970-01-02T00:16:40Z"},{"label":"Claude + GPT Models · weekly","remaining":1.0,"reset_time":"1970-01-03T02:16:40Z"}]}"#).unwrap()
}

#[test]
fn weekly_only_quota_shows_families_remaining_resets_and_account_hit_targets() {
    let mut app = app(vec![provider(
        "agy",
        "antigravity_oauth",
        vec![family_account()],
    )]);
    let (screen, terminal) = draw(&mut app, 120, 15);
    assert!(
        screen.contains("Gemini Models · weekly: 0% left · resets in 1d"),
        "{screen}"
    );
    assert!(
        screen.contains("Claude + GPT Models · weekly: 100% left · resets in 2d 2h"),
        "{screen}"
    );
    assert!(!screen.contains("Models · 5h"), "{screen}");
    assert!(!screen.contains('█'), "no invented usage bars: {screen}");
    let y = screen
        .lines()
        .position(|line| line.contains("Gemini Models"))
        .unwrap() as u16;
    assert_eq!(terminal.backend().buffer()[(48, y)].fg, Color::Red);
    assert_eq!(terminal.backend().buffer()[(54, y + 1)].fg, Color::Green);
    for hit in &app.hits[1..4] {
        assert_eq!(hit.as_ref().unwrap().account.as_deref(), Some("ref-weekly"));
    }
}

#[test]
fn family_quota_shows_only_reported_windows_and_tolerates_missing_values() {
    let mut account = family_account();
    account.quota_buckets[0].label = "Gemini Models · 5h".into();
    account.quota_buckets[0].remaining = None;
    account.quota_buckets[0].reset_time = None;
    account.quota_buckets[1].reset_time = Some("invalid".into());
    let mut app = app(vec![provider("agy", "antigravity_oauth", vec![account])]);
    let screen = draw(&mut app, 120, 15).0;
    assert!(screen.contains("Gemini Models · 5h: —"), "{screen}");
    assert!(
        screen.contains("Claude + GPT Models · weekly: 100% left"),
        "{screen}"
    );
    assert!(!screen.contains("resets in"), "{screen}");
}

#[test]
fn claude_and_codex_rows_keep_their_window_rendering() {
    for auth in ["claude_oauth", "chatgpt_oauth"] {
        let mut account = acct("legacy");
        account.utilization_5h = Some(0.5);
        account.reset_5h = Some(NOW + 3600);
        account.utilization_7d = Some(0.25);
        account.reset_7d = Some(NOW + 86400);
        let before = draw(
            &mut app(vec![provider("provider", auth, vec![account.clone()])]),
            140,
            15,
        )
        .0;
        assert!(before.contains("50% · 1h"), "{before}");
        assert!(before.contains("25% · 1d"), "{before}");
        account.quota_buckets = family_account().quota_buckets;
        let after = draw(
            &mut app(vec![provider("provider", auth, vec![account])]),
            140,
            15,
        )
        .0;
        assert_eq!(before, after, "{auth} rendering changed");
    }
}

/// Opt-in acceptance check. Reads a real gateway through the production pool client.
#[tokio::test]
#[ignore = "requires SHUNT_TUI_LIVE_URL and SHUNT_ADMIN_TOKEN for a running gateway"]
async fn live_antigravity_quota_snapshot() {
    let url = std::env::var("SHUNT_TUI_LIVE_URL").expect("live gateway URL");
    let token = std::env::var("SHUNT_ADMIN_TOKEN").expect("admin token");
    let client = crate::tui::client::Client::new(&url, "x-shunt-admin-token", &token).unwrap();
    let snapshot = client.fetch_pool().await.unwrap();
    let providers: Vec<_> = snapshot
        .providers
        .iter()
        .filter(|p| p.auth.as_deref() == Some("antigravity_oauth"))
        .collect();
    assert!(!providers.is_empty(), "live gateway must have Antigravity");
    let mut account_count = 0;
    for provider in providers {
        assert!(!provider.rows.is_empty());
        for row in &provider.rows {
            assert!(
                !row.account.quota_buckets.is_empty(),
                "{} has no quota",
                row.account.name
            );
            for bucket in &row.account.quota_buckets {
                assert!(bucket.remaining.is_some());
                assert!(bucket.reset_secs().is_some());
            }
            account_count += 1;
        }
    }
    let mut app = App::new(url);
    app.on_poll(Ok(snapshot));
    let mut terminal = Terminal::new(TestBackend::new(150, 60)).unwrap();
    terminal
        .draw(|frame| render(frame, &mut app, now_secs()))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let screen = (0..60)
        .map(|y| {
            (0..150)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("Gemini Models"));
    assert!(screen.contains("Claude + GPT Models"));
    assert!(screen.contains("% left · resets in"));
    println!("LIVE_ANTIGRAVITY_ACCOUNTS={account_count}\n{screen}");
}

#[test]
fn custom_ranking_shows_the_chosen_numbers() {
    let rank = |n: &str, p: u32| AccountDto {
        priority: Some(p),
        ..acct(n)
    };
    let mut app = app(vec![provider(
        "anthropic",
        "claude_oauth",
        vec![rank("second", 2), rank("first", 1)],
    )]);
    let (screen, _) = draw(&mut app, 110, 12);
    assert!(screen.contains("custom order"), "{screen}");
    let first = screen.lines().position(|l| l.contains("first")).unwrap();
    let second = screen.lines().position(|l| l.contains("second")).unwrap();
    assert!(first < second);
    assert!(screen
        .lines()
        .nth(first)
        .unwrap()
        .trim_start()
        .starts_with('1'));
    assert!(screen
        .lines()
        .nth(second)
        .unwrap()
        .trim_start()
        .starts_with('2'));
}

#[test]
fn weekly_reset_header_and_rows_follow_the_pool_flag_but_custom_stays_custom() {
    let mut later = acct("later");
    later.reset_7d = Some(8_000);
    let mut sooner = acct("sooner");
    sooner.reset_7d = Some(2_000);
    let mut ranked = acct("ranked");
    ranked.priority = Some(1);
    let mut app = app(vec![
        provider("anthropic", "claude_oauth", vec![later, sooner]),
        provider("codex", "chatgpt_oauth", vec![ranked, acct("next")]),
    ]);
    app.snapshot.providers[0].weekly_reset = true;
    app.snapshot.providers[1].weekly_reset = true;
    let (screen, _) = draw(&mut app, 120, 18);
    assert!(screen.contains("ranking: weekly reset"), "{screen}");
    assert!(screen.contains("ranking: custom order"), "{screen}");
    assert!(screen.find("sooner").unwrap() < screen.find("later").unwrap());
}

#[test]
fn help_describes_the_three_mode_cycle_and_global_scope() {
    let mut app = app(vec![]);
    app.show_help = true;
    let (screen, _) = draw(&mut app, 120, 34);
    assert!(
        screen.contains("custom order → balanced → weekly reset → custom order"),
        "{screen}"
    );
    assert!(screen.contains("ALL pooled providers"), "{screen}");
    assert!(screen.contains("until gateway restart"), "{screen}");
}

#[test]
fn a_provider_with_every_account_paused_reads_off() {
    let paused = AccountDto {
        paused: true,
        ..acct("a")
    };
    let mut app = app(vec![provider("anthropic", "claude_oauth", vec![paused])]);
    let (screen, _) = draw(&mut app, 100, 10);
    assert!(screen.contains("OFF"), "{screen}");
}

#[test]
fn no_row_is_highlighted_until_one_is_selected_and_deselecting_clears_it() {
    let mut app = app(vec![provider(
        "anthropic",
        "claude_oauth",
        vec![acct("a"), acct("b")],
    )]);
    let (_, terminal) = draw(&mut app, 100, 12);
    assert!(
        !has_reversed_cell(&terminal),
        "nothing selected, nothing highlighted"
    );

    app.selected = Some(Sel {
        provider: "anthropic".into(),
        account: Some("ref-a".into()),
    });
    let (_, terminal) = draw(&mut app, 100, 12);
    assert!(has_reversed_cell(&terminal));

    app.selected = None;
    let (_, terminal) = draw(&mut app, 100, 12);
    assert!(!has_reversed_cell(&terminal));
}

#[test]
fn the_drawn_list_records_what_each_line_is_for_mouse_clicks() {
    let mut app = app(vec![provider(
        "anthropic",
        "claude_oauth",
        vec![acct("a"), acct("b")],
    )]);
    draw(&mut app, 100, 12);
    assert_eq!(
        app.hits[0],
        Some(Sel {
            provider: "anthropic".into(),
            account: None
        })
    );
    assert_eq!(
        app.hits[1].as_ref().unwrap().account.as_deref(),
        Some("ref-a")
    );
    assert_eq!(
        app.hits[2].as_ref().unwrap().account.as_deref(),
        Some("ref-b")
    );
    assert!(
        app.list_area.y >= 3,
        "the list sits below the header and column headings"
    );
}

#[test]
fn a_long_list_scrolls_to_keep_the_selection_in_view() {
    let accounts: Vec<_> = (0..30).map(|i| acct(&format!("acct-{i:02}"))).collect();
    let mut app = app(vec![provider("anthropic", "claude_oauth", accounts)]);
    app.selected = Some(Sel {
        provider: "anthropic".into(),
        account: Some("ref-acct-29".into()),
    });
    app.reveal = true;
    let (screen, _) = draw(&mut app, 100, 12);
    assert!(screen.contains("acct-29"), "{screen}");
    assert!(!screen.contains("acct-00"), "{screen}");
    assert!(app.scroll > 0);
}

#[test]
fn empty_and_unreachable_states_explain_themselves() {
    let mut waiting = App::new("http://x".into());
    assert!(draw(&mut waiting, 90, 10)
        .0
        .contains("Waiting for the first poll"));
    let mut down = App::new("http://x".into());
    down.on_poll(Err("gateway unreachable".into()));
    let screen = draw(&mut down, 90, 10).0;
    assert!(screen.contains("Cannot reach the gateway"), "{screen}");
    assert!(screen.contains("gateway unreachable"), "{screen}");
}

#[test]
fn a_provider_with_no_accounts_says_how_to_add_one() {
    let mut app = app(vec![provider("codex", "chatgpt_oauth", vec![])]);
    let screen = draw(&mut app, 100, 10).0;
    assert!(
        screen.contains("no accounts · press a to add one"),
        "{screen}"
    );
}

#[test]
fn the_add_dialog_shows_the_authorize_url_and_what_to_paste() {
    let mut app = app(vec![provider("anthropic", "claude_oauth", vec![acct("a")])]);
    let mut flow = crate::tui::add::AddFlow::new(
        vec![Target {
            provider: "anthropic".into(),
            kind: "claude",
        }],
        None,
    )
    .unwrap();
    flow.name = "pool-b".into();
    flow.url = "https://claude.ai/oauth/authorize?x=1".into();
    flow.step = Step::Code;
    flow.error = Some("invalid code".into());
    app.dialog = Some(flow);
    let screen = draw(&mut app, 100, 24).0;
    assert!(screen.contains("Adding pool-b to anthropic"), "{screen}");
    assert!(
        screen.contains("https://claude.ai/oauth/authorize?x=1"),
        "{screen}"
    );
    assert!(screen.contains("code#state"), "{screen}");
    assert!(screen.contains("invalid code"), "{screen}");
}

#[test]
fn the_delete_confirmation_names_the_account_and_requires_y_or_n() {
    let mut app = app(vec![provider(
        "anthropic",
        "claude_oauth",
        vec![acct("work")],
    )]);
    app.delete_confirm = Some(DeleteConfirm {
        provider: "anthropic".into(),
        kind: "claude",
        name: "work".into(),
    });
    let screen = draw(&mut app, 100, 16).0;
    assert!(screen.contains("Delete account"), "{screen}");
    assert!(screen.contains("work"), "{screen}");
    assert!(screen.contains("from anthropic?"), "{screen}");
    assert!(screen.contains("y: delete"), "{screen}");
    assert!(screen.contains("n / Esc: cancel"), "{screen}");
}

#[test]
fn help_and_tiny_terminals_do_not_panic() {
    let mut app = app(vec![provider("anthropic", "claude_oauth", vec![acct("a")])]);
    app.show_help = true;
    draw(&mut app, 100, 30);
    draw(&mut app, 20, 6);
    draw(&mut app, 1, 1);
    app.show_help = false;
    app.selected = Some(Sel {
        provider: "anthropic".into(),
        account: None,
    });
    app.reveal = true;
    draw(&mut app, 20, 2);
}

#[test]
fn a_notice_keeps_the_hints_visible_on_their_own_line() {
    let mut app = app(vec![provider("anthropic", "claude_oauth", vec![acct("a")])]);
    app.selected = Some(Sel {
        provider: "anthropic".into(),
        account: Some("ref-a".into()),
    });
    app.notify(Notice::Info("paused a".into()));
    let screen = draw(&mut app, 110, 12).0;
    assert!(screen.contains("paused a"), "{screen}");
    assert!(screen.contains("p pause"), "{screen}");
    let notice_line = screen.lines().position(|l| l.contains("paused a")).unwrap();
    let hint_line = screen.lines().position(|l| l.contains("p pause")).unwrap();
    assert_ne!(notice_line, hint_line, "{screen}");
}

#[test]
fn requests_cell_shows_ok_fail_and_latency_once_attempted() {
    let mut a = acct("a");
    assert_eq!(requests_cell(&a), "–");
    a.requests_attempted = 12;
    a.requests_succeeded = 10;
    a.requests_failed = 2;
    assert_eq!(requests_cell(&a), "10/2");
    a.mean_latency_ms = Some(419.6);
    assert_eq!(requests_cell(&a), "10/2 · 420ms");
}

#[test]
fn a_hidden_provider_is_not_drawn_and_the_footer_carries_its_count() {
    let mut app = app(vec![
        provider("anthropic", "claude_oauth", vec![acct("a")]),
        provider("antigravity", "antigravity_oauth", vec![acct("g")]),
    ]);
    app.hidden.insert("antigravity".into());
    let (screen, _) = draw(&mut app, 110, 12);
    assert!(screen.contains("anthropic"), "{screen}");
    assert!(!screen.contains("▾ antigravity"), "{screen}");
    assert!(!screen.contains("hidden (antigravity)"), "{screen}");
    assert!(screen.contains("u unhide (1)"), "{screen}");
}

#[test]
fn hiding_everything_explains_how_to_get_it_back() {
    let mut app = app(vec![provider("anthropic", "claude_oauth", vec![acct("a")])]);
    app.hidden.insert("anthropic".into());
    let screen = draw(&mut app, 100, 10).0;
    assert!(screen.contains("All providers hidden"), "{screen}");
}

#[test]
fn the_unhide_picker_lists_hidden_providers() {
    let mut app = app(vec![
        provider("anthropic", "claude_oauth", vec![acct("a")]),
        provider("codex", "chatgpt_oauth", vec![acct("c")]),
    ]);
    app.hidden.insert("codex".into());
    app.unhide = Some(crate::tui::app::UnhidePicker { at: 0 });
    let (screen, _) = draw(&mut app, 100, 16);
    assert!(screen.contains("Unhide provider"), "{screen}");
    assert!(screen.contains("codex"), "{screen}");
    assert!(screen.contains("Enter unhide"), "{screen}");
}
