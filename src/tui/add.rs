//! The add-account dialog: pick a provider, name the account, authorize in the
//! browser, paste the code. A small state machine; the runtime performs the two
//! gateway calls it asks for and reports their results back.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// A provider the dialog can add an account to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub provider: String,
    /// The admin API's account family: `claude`, `codex` or `antigravity`.
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Provider {
        at: usize,
    },
    Name,
    /// Waiting for the gateway to hand back the authorize URL.
    Starting,
    /// The URL is out; waiting for the operator to paste what the browser shows.
    Code,
    /// The code is with the gateway, which is exchanging it.
    Submitting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    None,
    Cancel,
    /// Put the authorize URL on the clipboard.
    CopyUrl(String),
    Start {
        target: Target,
        name: String,
    },
    Complete {
        target: Target,
        name: String,
        code: String,
    },
}

#[derive(Debug, Clone)]
pub struct AddFlow {
    pub choices: Vec<Target>,
    pub chosen: usize,
    pub step: Step,
    pub name: String,
    pub code: String,
    /// The authorize URL, once the gateway has issued it.
    pub url: String,
    /// The last failure, shown inside the dialog so it can be retried.
    pub error: Option<String>,
}

/// Account names the store accepts (`[a-z0-9-]+`).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl AddFlow {
    /// Open the dialog. With one choice, or a provider already selected, the
    /// provider step is skipped.
    pub fn new(choices: Vec<Target>, preferred: Option<&str>) -> Option<Self> {
        if choices.is_empty() {
            return None;
        }
        let preselected = preferred.and_then(|p| choices.iter().position(|t| t.provider == p));
        let skip = preselected.is_some() || choices.len() == 1;
        let chosen = preselected.unwrap_or(0);
        Some(Self {
            choices,
            chosen,
            step: if skip {
                Step::Name
            } else {
                Step::Provider { at: 0 }
            },
            name: String::new(),
            code: String::new(),
            url: String::new(),
            error: None,
        })
    }

    pub fn target(&self) -> &Target {
        &self.choices[self.chosen]
    }

    /// Whether the terminal's mouse capture should be off, so the operator can
    /// select and copy the authorize URL with the mouse.
    pub fn wants_text_selection(&self) -> bool {
        matches!(self.step, Step::Code | Step::Submitting)
    }

    pub fn on_paste(&mut self, text: &str) {
        match self.step {
            Step::Name => self.name.push_str(text.trim()),
            Step::Code => self.code.push_str(text.trim()),
            _ => {}
        }
    }

    pub fn on_started(&mut self, result: Result<String, String>) {
        if self.step != Step::Starting {
            return;
        }
        match result {
            Ok(url) => {
                self.error = None;
                self.url = url;
                self.step = Step::Code;
            }
            Err(error) => {
                self.error = Some(error);
                self.step = Step::Name;
            }
        }
    }

    /// `Err` keeps the dialog open on the code step so a wrong paste can be
    /// retried (the gateway allows a few attempts per login).
    pub fn on_completed(&mut self, result: Result<(), String>) -> bool {
        if self.step != Step::Submitting {
            return false;
        }
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                self.code.clear();
                // Back to the code step with the URL still on screen, so a wrong
                // paste is retried against the same login (the gateway allows a
                // few attempts; once it expires it says so and the operator
                // cancels and starts again).
                self.step = Step::Code;
                false
            }
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> Effect {
        if key.kind == KeyEventKind::Release {
            return Effect::None;
        }
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return Effect::Cancel;
        }
        match self.step.clone() {
            Step::Provider { at } => self.provider_key(key, at),
            Step::Name => self.name_key(key),
            Step::Code => self.code_key(key),
            Step::Starting | Step::Submitting => Effect::None,
        }
    }

    fn provider_key(&mut self, key: KeyEvent, at: usize) -> Effect {
        let last = self.choices.len() - 1;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.step = Step::Provider {
                    at: at.saturating_sub(1),
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.step = Step::Provider {
                    at: (at + 1).min(last),
                }
            }
            KeyCode::Enter => {
                self.chosen = at;
                self.step = Step::Name;
            }
            _ => {}
        }
        Effect::None
    }

    fn name_key(&mut self, key: KeyEvent) -> Effect {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.name.push(c);
                self.error = None;
            }
            KeyCode::Backspace => {
                self.name.pop();
            }
            KeyCode::Enter => {
                if !valid_name(&self.name) {
                    self.error =
                        Some("name must be lowercase letters, digits and hyphens".to_string());
                } else {
                    self.error = None;
                    self.step = Step::Starting;
                    return Effect::Start {
                        target: self.target().clone(),
                        name: self.name.clone(),
                    };
                }
            }
            _ => {}
        }
        Effect::None
    }

    fn code_key(&mut self, key: KeyEvent) -> Effect {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.code.push(c);
                self.error = None;
            }
            KeyCode::Backspace => {
                self.code.pop();
            }
            KeyCode::Tab => return Effect::CopyUrl(self.url.clone()),
            KeyCode::Enter if !self.code.trim().is_empty() => {
                self.step = Step::Submitting;
                return Effect::Complete {
                    target: self.target().clone(),
                    name: self.name.clone(),
                    code: self.code.clone(),
                };
            }
            _ => {}
        }
        Effect::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_text(flow: &mut AddFlow, text: &str) {
        for c in text.chars() {
            flow.on_key(key(KeyCode::Char(c)));
        }
    }

    fn two() -> Vec<Target> {
        vec![
            Target {
                provider: "anthropic".into(),
                kind: "claude",
            },
            Target {
                provider: "codex".into(),
                kind: "codex",
            },
        ]
    }

    #[test]
    fn nothing_to_add_to_means_no_dialog() {
        assert!(AddFlow::new(vec![], None).is_none());
    }

    #[test]
    fn provider_step_is_skipped_when_it_is_already_known() {
        assert_eq!(AddFlow::new(two(), Some("codex")).unwrap().step, Step::Name);
        assert_eq!(
            AddFlow::new(two()[..1].to_vec(), None).unwrap().step,
            Step::Name
        );
        let f = AddFlow::new(two(), None).unwrap();
        assert_eq!(f.step, Step::Provider { at: 0 });
    }

    #[test]
    fn full_flow_start_authorize_complete() {
        let mut f = AddFlow::new(two(), None).unwrap();
        f.on_key(key(KeyCode::Down));
        f.on_key(key(KeyCode::Enter));
        assert_eq!(f.target().provider, "codex");
        type_text(&mut f, "pool-b");
        let effect = f.on_key(key(KeyCode::Enter));
        assert_eq!(
            effect,
            Effect::Start {
                target: two()[1].clone(),
                name: "pool-b".into()
            }
        );
        assert_eq!(f.step, Step::Starting);
        f.on_started(Ok("https://auth.example/x".into()));
        assert_eq!(f.step, Step::Code);
        assert_eq!(f.url, "https://auth.example/x");
        assert!(f.wants_text_selection());
        type_text(&mut f, "abc#def");
        let effect = f.on_key(key(KeyCode::Enter));
        assert_eq!(
            effect,
            Effect::Complete {
                target: two()[1].clone(),
                name: "pool-b".into(),
                code: "abc#def".into()
            }
        );
        assert!(f.on_completed(Ok(())));
    }

    #[test]
    fn bad_names_never_reach_the_gateway() {
        let mut f = AddFlow::new(two(), Some("anthropic")).unwrap();
        assert_eq!(f.on_key(key(KeyCode::Enter)), Effect::None);
        assert!(f.error.is_some());
        type_text(&mut f, "Bad_Name");
        // Uppercase and underscore were typed; Enter must still refuse.
        assert_eq!(f.on_key(key(KeyCode::Enter)), Effect::None);
        assert_eq!(f.step, Step::Name);
        assert!(valid_name("pool-2") && !valid_name("") && !valid_name("a b"));
    }

    #[test]
    fn a_failed_start_returns_to_the_name_and_a_failed_code_can_be_retried() {
        let mut f = AddFlow::new(two(), Some("anthropic")).unwrap();
        type_text(&mut f, "x");
        f.on_key(key(KeyCode::Enter));
        f.on_started(Err("gateway answered 400".into()));
        assert_eq!(f.step, Step::Name);
        assert_eq!(f.error.as_deref(), Some("gateway answered 400"));
        f.on_key(key(KeyCode::Enter));
        f.on_started(Ok("u".into()));
        type_text(&mut f, "wrong");
        f.on_key(key(KeyCode::Enter));
        assert_eq!(f.step, Step::Submitting);
        assert!(!f.on_completed(Err("invalid code".into())));
        assert_eq!(f.step, Step::Code);
        assert_eq!(f.url, "u", "the URL stays on screen for the retry");
        assert!(f.code.is_empty());
    }

    #[test]
    fn escape_and_ctrl_c_cancel_at_any_step_but_keys_are_ignored_while_busy() {
        let mut f = AddFlow::new(two(), Some("anthropic")).unwrap();
        assert_eq!(f.on_key(key(KeyCode::Esc)), Effect::Cancel);
        type_text(&mut f, "x");
        f.on_key(key(KeyCode::Enter));
        assert_eq!(f.step, Step::Starting);
        assert_eq!(f.on_key(key(KeyCode::Char('z'))), Effect::None);
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(f.on_key(ctrl_c), Effect::Cancel);
    }

    #[test]
    fn tab_copies_the_authorize_url_without_leaving_the_dialog() {
        let mut f = AddFlow::new(two(), Some("anthropic")).unwrap();
        type_text(&mut f, "x");
        f.on_key(key(KeyCode::Enter));
        f.on_started(Ok("https://auth.example/y".into()));
        assert_eq!(
            f.on_key(key(KeyCode::Tab)),
            Effect::CopyUrl("https://auth.example/y".into())
        );
        assert_eq!(f.step, Step::Code);
    }

    #[test]
    fn pasted_text_lands_in_the_active_field() {
        let mut f = AddFlow::new(two(), Some("anthropic")).unwrap();
        f.on_paste("  pool-c \n");
        assert_eq!(f.name, "pool-c");
    }
}
