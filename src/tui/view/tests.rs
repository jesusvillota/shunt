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
    })));
    app
}

fn provider(name: &str, auth: &str, accounts: Vec<AccountDto>) -> ProviderDto {
    ProviderDto {
        provider: name.into(),
        auth: Some(auth.into()),
        accounts,
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
        screen.contains("ranking: balanced · 2 accounts"),
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
