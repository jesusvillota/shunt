//! The Models dialog (`M`): list the live models of each CLI platform and
//! switch them on or off for the `goal` tool and `/delegate-cli`. It changes
//! only `~/.shunt/models.json`; gateway routing is untouched.

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::{
    models_catalog::PLATFORMS,
    models_prefs::{ModelPrefs, Resolved},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Catalog {
    Loading,
    Ready(Vec<String>),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelsDialog {
    pub prefs: ModelPrefs,
    /// One per entry of [`PLATFORMS`].
    pub catalogs: Vec<Catalog>,
    pub tab: usize,
    pub at: usize,
}

/// What a key press asks the app to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Close,
    /// The preferences changed: write them.
    Save(ModelPrefs),
}

impl ModelsDialog {
    pub fn new(prefs: ModelPrefs) -> Self {
        Self {
            prefs,
            catalogs: vec![Catalog::Loading; PLATFORMS.len()],
            tab: 0,
            at: 0,
        }
    }

    pub fn platform(&self) -> &'static str {
        PLATFORMS[self.tab]
    }

    /// The current tab's models with their resolved state.
    pub fn rows(&self) -> Vec<Resolved> {
        match &self.catalogs[self.tab] {
            Catalog::Ready(ids) => self.prefs.resolve(self.platform(), ids),
            _ => Vec::new(),
        }
    }

    pub fn loaded(&mut self, platform: &str, result: Result<Vec<String>, String>) {
        if let Some(i) = PLATFORMS.iter().position(|p| *p == platform) {
            self.catalogs[i] = result.map_or_else(Catalog::Failed, Catalog::Ready);
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let len = self.rows().len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Close,
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.switch_tab(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.switch_tab(PLATFORMS.len() - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.at = self.at.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.at = (self.at + 1).min(len.saturating_sub(1))
            }
            KeyCode::Char('f') => {
                self.prefs.filter = !self.prefs.filter;
                return Outcome::Save(self.prefs.clone());
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let (Catalog::Ready(ids), Some(row)) =
                    (&self.catalogs[self.tab], self.rows().get(self.at))
                {
                    self.prefs.set(self.platform(), ids, &row.id, !row.active);
                    return Outcome::Save(self.prefs.clone());
                }
            }
            _ => {}
        }
        Outcome::Stay
    }

    fn switch_tab(&mut self, step: usize) {
        self.tab = (self.tab + step) % PLATFORMS.len();
        self.at = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn press(dialog: &mut ModelsDialog, code: KeyCode) -> Outcome {
        dialog.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ready() -> ModelsDialog {
        let mut dialog = ModelsDialog::new(ModelPrefs::default());
        dialog.loaded("codex", Ok(vec!["gpt-6-sol".into(), "gpt-6.1-sol".into()]));
        dialog
    }

    #[test]
    fn space_flips_the_model_under_the_cursor_and_asks_to_save() {
        let mut dialog = ready();
        assert!(!dialog.rows()[0].active, "older sol starts off");
        let Outcome::Save(prefs) = press(&mut dialog, KeyCode::Char(' ')) else {
            panic!("a toggle must save");
        };
        assert!(prefs.overrides["codex"]["gpt-6-sol"]);
        assert!(dialog.rows()[0].active);
        assert_eq!(dialog.rows()[0].reason, "override");
    }

    #[test]
    fn f_flips_the_filter_switch() {
        let mut dialog = ready();
        let Outcome::Save(prefs) = press(&mut dialog, KeyCode::Char('f')) else {
            panic!("the switch must save");
        };
        assert!(!prefs.filter);
    }

    #[test]
    fn nothing_toggles_while_loading_or_after_a_failed_fetch() {
        let mut dialog = ModelsDialog::new(ModelPrefs::default());
        assert_eq!(press(&mut dialog, KeyCode::Char(' ')), Outcome::Stay);
        dialog.loaded("codex", Err("codex not found".into()));
        assert_eq!(press(&mut dialog, KeyCode::Enter), Outcome::Stay);
    }

    #[test]
    fn tabs_wrap_and_reset_the_cursor_and_esc_closes() {
        let mut dialog = ready();
        press(&mut dialog, KeyCode::Down);
        assert_eq!(dialog.at, 1);
        press(&mut dialog, KeyCode::Tab);
        assert_eq!((dialog.platform(), dialog.at), ("opencode", 0));
        press(&mut dialog, KeyCode::BackTab);
        press(&mut dialog, KeyCode::BackTab);
        assert_eq!(dialog.platform(), "agy");
        assert_eq!(press(&mut dialog, KeyCode::Esc), Outcome::Close);
    }
}
