//! The Models dialog (`M`): list the live models of each CLI platform and
//! switch them on or off for the `goal` tool and `/delegate-cli`. It changes
//! only `~/.shunt/models.json`; gateway routing is untouched.

use std::collections::BTreeMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::{
    models_catalog::{RankerModel, PLATFORMS, RANKER_PLATFORMS},
    models_prefs::{ModelPrefs, OpenCodeRank, Ranker, Resolved},
    opencode_status::Table,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Catalog {
    Loading,
    Ready(Vec<String>),
    Failed(String),
}

/// The OpenCode free-model status, read from this machine's `opencode` log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpencodeStatus {
    /// Not read yet.
    Pending,
    Ready(Table),
    /// The log could not be read; every model shows unknown.
    Failed(String),
}

/// The OpenCode rerank run: idle, in flight, or failed (the old ranking stays).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ranking {
    Idle,
    Running,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RankerPicker {
    pub step: usize,
    pub at: usize,
    pub platform: usize,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelsDialog {
    pub prefs: ModelPrefs,
    /// One per entry of [`PLATFORMS`].
    pub catalogs: Vec<Catalog>,
    pub tab: usize,
    pub at: usize,
    /// The OpenCode log status, refreshed while the dialog is open.
    pub opencode: OpencodeStatus,
    /// The OpenCode rerank run, and its error when it failed.
    pub ranking: Ranking,
    pub picker: Option<RankerPicker>,
    pub ranker_catalogs: BTreeMap<String, Result<Vec<RankerModel>, String>>,
}

/// What a key press asks the app to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Stay,
    Close,
    /// The preferences changed: write them.
    Save(ModelPrefs),
    /// Rerank the OpenCode models (only offered on the `opencode` tab).
    Rerank,
}

impl ModelsDialog {
    pub fn new(prefs: ModelPrefs) -> Self {
        Self {
            prefs,
            catalogs: vec![Catalog::Loading; PLATFORMS.len()],
            tab: 0,
            at: 0,
            opencode: OpencodeStatus::Pending,
            ranking: Ranking::Idle,
            picker: None,
            ranker_catalogs: BTreeMap::new(),
        }
    }

    pub fn platform(&self) -> &'static str {
        PLATFORMS[self.tab]
    }

    /// The current tab's live model ids, once the catalog is loaded.
    pub fn ready_ids(&self) -> Option<Vec<String>> {
        match &self.catalogs[self.tab] {
            Catalog::Ready(ids) => Some(ids.clone()),
            _ => None,
        }
    }

    /// The rerank left: show `ranking…` until the answer comes back.
    pub fn start_ranking(&mut self) {
        self.ranking = Ranking::Running;
    }

    /// A finished rerank: store the rank on success, show the error otherwise
    /// (the old ranking is left in place).
    pub fn on_ranked(&mut self, result: Result<OpenCodeRank, String>) {
        match result {
            Ok(rank) => {
                self.prefs.opencode_rank = Some(rank);
                self.ranking = Ranking::Idle;
            }
            Err(error) => self.ranking = Ranking::Failed(error),
        }
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

    pub fn ranker_loaded(&mut self, platform: &str, result: Result<Vec<RankerModel>, String>) {
        self.loaded(
            platform,
            result
                .clone()
                .map(|models| models.into_iter().map(|m| m.id).collect()),
        );
        self.ranker_catalogs.insert(platform.into(), result);
    }

    pub fn picker_models(&self) -> &[RankerModel] {
        self.picker
            .as_ref()
            .and_then(|picker| self.ranker_catalogs.get(RANKER_PLATFORMS[picker.platform]))
            .and_then(|result| result.as_ref().ok())
            .map_or(&[], Vec::as_slice)
    }

    pub fn picker_rows(&self) -> Vec<String> {
        let Some(picker) = &self.picker else {
            return Vec::new();
        };
        match picker.step {
            0 => RANKER_PLATFORMS.map(String::from).to_vec(),
            1 => std::iter::once("CLI default model".into())
                .chain(self.picker_models().iter().map(|m| {
                    if m.efforts.is_empty() {
                        m.id.clone()
                    } else {
                        format!("{} · {}", m.id, m.efforts.join(" / "))
                    }
                }))
                .collect(),
            _ => std::iter::once(if RANKER_PLATFORMS[picker.platform] == "opencode" {
                "CLI default effort (ignored on opencode)".into()
            } else {
                "CLI default effort".into()
            })
            .chain(
                self.picker_models()
                    .iter()
                    .find(|m| Some(&m.id) == picker.model.as_ref())
                    .into_iter()
                    .flat_map(|m| m.efforts.clone()),
            )
            .collect(),
        }
    }

    fn picker_key(&mut self, key: KeyEvent) -> Outcome {
        let rows = self.picker_rows();
        let models = self.picker_models().to_vec();
        let picker = self.picker.as_mut().expect("picker is open");
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.picker = None,
            KeyCode::Up | KeyCode::Char('k') => picker.at = picker.at.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                picker.at = (picker.at + 1).min(rows.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                match picker.step {
                    0 => picker.platform = picker.at,
                    1 => {
                        picker.model = picker
                            .at
                            .checked_sub(1)
                            .and_then(|at| models.get(at))
                            .map(|m| m.id.clone())
                    }
                    _ => {
                        self.prefs.ranker = Ranker::Selection {
                            platform: RANKER_PLATFORMS[picker.platform].into(),
                            model: picker.model.clone(),
                            effort: (picker.at > 0).then(|| rows[picker.at].clone()),
                        };
                        self.picker = None;
                        return Outcome::Save(self.prefs.clone());
                    }
                }
                picker.step += 1;
                picker.at = 0;
            }
            _ => {}
        }
        Outcome::Stay
    }

    /// Store a fresh read of the OpenCode log.
    pub fn set_opencode(&mut self, result: Result<Table, String>) {
        self.opencode = result.map_or_else(OpencodeStatus::Failed, OpencodeStatus::Ready);
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        if self.picker.is_some() {
            return self.picker_key(key);
        }
        let len = self.rows().len();
        match key.code {
            KeyCode::Char('s') => self.picker = Some(RankerPicker::default()),
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
            KeyCode::Char('r')
                if self.platform() == "opencode" && self.ranking != Ranking::Running =>
            {
                return Outcome::Rerank
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
    fn ranker_picker_saves_the_catalog_model_and_its_effort_as_an_object() {
        let mut dialog = ready();
        dialog.ranker_loaded(
            "codex",
            Ok(vec![RankerModel {
                id: "gpt-6-sol".into(),
                efforts: vec!["low".into(), "high".into()],
            }]),
        );
        press(&mut dialog, KeyCode::Char('s'));
        assert_eq!(dialog.picker_rows(), RANKER_PLATFORMS);
        press(&mut dialog, KeyCode::Down); // codex
        press(&mut dialog, KeyCode::Enter);
        assert!(dialog.picker_rows()[1].contains("gpt-6-sol · low / high"));
        press(&mut dialog, KeyCode::Down); // catalog model
        press(&mut dialog, KeyCode::Enter);
        assert_eq!(dialog.picker_rows(), ["CLI default effort", "low", "high"]);
        press(&mut dialog, KeyCode::Down);
        press(&mut dialog, KeyCode::Down);
        let Outcome::Save(prefs) = press(&mut dialog, KeyCode::Enter) else {
            panic!("choice must save")
        };
        assert!(dialog.picker.is_none());
        let dir = std::env::temp_dir().join(format!("shunt-ranker-picker-{}", std::process::id()));
        let path = dir.join("models.json");
        crate::models_prefs::save_to(&path, &prefs).unwrap();
        assert_eq!(crate::models_prefs::load_from(&path), prefs);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            json["ranker"],
            serde_json::json!({"platform":"codex","model":"gpt-6-sol","effort":"high"})
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn canceling_the_ranker_picker_never_rewrites_a_legacy_choice() {
        let mut dialog = ready();
        for step in 0..3 {
            press(&mut dialog, KeyCode::Char('s'));
            for _ in 0..step {
                press(&mut dialog, KeyCode::Enter);
            }
            assert_eq!(press(&mut dialog, KeyCode::Esc), Outcome::Stay);
            assert_eq!(dialog.prefs.ranker, Ranker::from("claude"));
            assert!(dialog.picker.is_none());
        }
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

    #[test]
    fn r_only_reranks_on_the_opencode_tab_with_a_loaded_catalog() {
        let mut dialog = ready();
        assert_eq!(
            press(&mut dialog, KeyCode::Char('r')),
            Outcome::Stay,
            "codex has no ranking"
        );
        dialog.loaded("opencode", Ok(vec!["opencode/a".into()]));
        press(&mut dialog, KeyCode::Tab);
        assert_eq!(press(&mut dialog, KeyCode::Char('r')), Outcome::Rerank);
        dialog.start_ranking();
        assert_eq!(
            press(&mut dialog, KeyCode::Char('r')),
            Outcome::Stay,
            "one rerank runs at a time"
        );
    }

    #[test]
    fn a_failed_rerank_keeps_the_old_ranking_and_shows_the_error() {
        use crate::models_prefs::OpenCodeRank;
        let mut dialog = ready();
        dialog.prefs.opencode_rank = Some(OpenCodeRank {
            ranked_at: "2026-10-17T14:15:37Z".into(),
            order: vec!["opencode/a".into()],
        });
        let previous = dialog.prefs.clone();
        struct Fake(Result<String, String>);
        impl crate::rerank::Runner for Fake {
            fn run(&self, _: &[String], _: std::time::Duration) -> Result<String, String> {
                self.0.clone()
            }
        }
        let dir = std::env::temp_dir().join(format!("shunt-dialog-rerank-{}", std::process::id()));
        for answer in [
            Err("claude timed out".to_string()),
            Ok("{not json}".to_string()),
            Ok("{}".to_string()),
        ] {
            let error = crate::rerank::run(
                &dir.join("criteria.md"),
                &dialog.prefs.ranker,
                &["opencode/a".into()],
                &Fake(answer),
                std::time::SystemTime::now(),
            )
            .unwrap_err();
            dialog.start_ranking();
            assert_eq!(dialog.ranking, Ranking::Running);
            dialog.on_ranked(Err(error.clone()));
            assert_eq!(dialog.ranking, Ranking::Failed(error));
            assert_eq!(dialog.prefs, previous, "old ranking stays");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_successful_rerank_stores_the_new_rank() {
        use crate::models_prefs::OpenCodeRank;
        let mut dialog = ready();
        dialog.on_ranked(Ok(OpenCodeRank {
            ranked_at: "2026-10-17T15:00:00Z".into(),
            order: vec!["opencode/b".into(), "opencode/a".into()],
        }));
        assert_eq!(dialog.ranking, Ranking::Idle);
        let rank = dialog.prefs.opencode_rank.expect("stored");
        assert_eq!(rank.order, ["opencode/b", "opencode/a"]);
    }
}
