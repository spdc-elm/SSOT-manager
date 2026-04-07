use std::fs;
use std::io::{self, Stdout};
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::Widget;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};

use crate::config::{
    Config, EditableConfigDocument, EditableProfile, EditableRule, MaterializationMode,
    load_editable_config, validate_and_write_editable_config, validate_editable_config,
};
use crate::inspection::{
    ProfileExplainView, ProfileShowView, explain_profile, list_profiles, show_profile,
};
use crate::paths::resolve_input_path;
use crate::prompt::build_profile_requirements;
use crate::reconcile::{
    apply_plan, apply_plan_force_with_backup, build_plan, can_force_with_backup, doctor_profile,
    undo_last_apply,
};
use crate::state::{ManagedState, StateStore};

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailView {
    Show,
    Plan,
    Doctor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorField {
    Name,
    SourceRoot,
    Requires,
    Rules,
}

impl EditorField {
    fn next(self) -> Self {
        match self {
            EditorField::Name => EditorField::SourceRoot,
            EditorField::SourceRoot => EditorField::Requires,
            EditorField::Requires => EditorField::Rules,
            EditorField::Rules => EditorField::Name,
        }
    }

    fn previous(self) -> Self {
        match self {
            EditorField::Name => EditorField::Rules,
            EditorField::SourceRoot => EditorField::Name,
            EditorField::Requires => EditorField::SourceRoot,
            EditorField::Rules => EditorField::Requires,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleField {
    Select,
    Mode,
    Enabled,
    Note,
    Destinations,
    Tags,
}

impl RuleField {
    fn next(self) -> Self {
        match self {
            RuleField::Select => RuleField::Mode,
            RuleField::Mode => RuleField::Enabled,
            RuleField::Enabled => RuleField::Note,
            RuleField::Note => RuleField::Destinations,
            RuleField::Destinations => RuleField::Tags,
            RuleField::Tags => RuleField::Select,
        }
    }

    fn previous(self) -> Self {
        match self {
            RuleField::Select => RuleField::Tags,
            RuleField::Mode => RuleField::Select,
            RuleField::Enabled => RuleField::Mode,
            RuleField::Note => RuleField::Enabled,
            RuleField::Destinations => RuleField::Note,
            RuleField::Tags => RuleField::Destinations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileDraft {
    original_name: Option<String>,
    name: String,
    source_root: Option<String>,
    requires: Vec<String>,
    rules: Vec<EditableRule>,
}

impl ProfileDraft {
    fn from_existing(name: &str, profile: &EditableProfile) -> Self {
        Self {
            original_name: Some(name.to_string()),
            name: name.to_string(),
            source_root: profile.source_root.clone(),
            requires: profile.requires.clone(),
            rules: profile.rules.clone(),
        }
    }

    fn new_empty() -> Self {
        Self {
            original_name: None,
            name: String::new(),
            source_root: None,
            requires: Vec::new(),
            rules: Vec::new(),
        }
    }

    fn to_editable_profile(&self) -> EditableProfile {
        EditableProfile {
            source_root: self
                .source_root
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            requires: self
                .requires
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            rules: self.rules.clone(),
        }
    }

    fn normalized(&self) -> Self {
        let mut draft = self.clone();
        draft.name = draft.name.trim().to_string();
        draft.source_root = draft
            .source_root
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        draft.requires = draft
            .requires
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        for rule in &mut draft.rules {
            rule.select = rule.select.trim().to_string();
            rule.mode = rule.mode.trim().to_string();
            rule.to = rule
                .to
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            rule.tags = rule
                .tags
                .iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            rule.note = rule
                .note
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
        draft
    }
}

#[derive(Debug, Clone)]
struct ProfileEditorState {
    draft: ProfileDraft,
    initial: ProfileDraft,
    selected_field: EditorField,
    overlay: EditorOverlay,
}

impl ProfileEditorState {
    fn is_dirty(&self) -> bool {
        self.draft.normalized() != self.initial.normalized()
    }
}

#[derive(Debug, Clone)]
enum EditorOverlay {
    None,
    TextInput(TextInputState),
    StringList(StringListEditorState),
    RuleList(RuleListEditorState),
    RuleEditor(RuleEditorState),
    Confirm(EditorConfirmState),
}

#[derive(Debug, Clone)]
struct TextInputState {
    title: String,
    value: String,
    cursor: usize,
    target: TextInputTarget,
    return_to: Box<EditorOverlay>,
    completion: Option<CompletionState>,
}

#[derive(Debug, Clone)]
struct CompletionState {
    matches: Vec<String>,
    selected: usize,
}

#[derive(Debug, Clone)]
enum TextInputTarget {
    ProfileName,
    ProfileSourceRoot,
    RequiresEntry {
        index: Option<usize>,
    },
    RuleSelect {
        rule_index: usize,
    },
    RuleNote {
        rule_index: usize,
    },
    RuleDestination {
        rule_index: usize,
        index: Option<usize>,
    },
    RuleTag {
        rule_index: usize,
        index: Option<usize>,
    },
}

#[derive(Debug, Clone)]
struct StringListEditorState {
    title: String,
    kind: StringListKind,
    selected: usize,
    return_to: Box<EditorOverlay>,
}

#[derive(Debug, Clone)]
enum StringListKind {
    Requires,
    RuleDestinations { rule_index: usize },
    RuleTags { rule_index: usize },
}

#[derive(Debug, Clone)]
struct RuleListEditorState {
    selected: usize,
}

#[derive(Debug, Clone)]
struct RuleEditorState {
    rule_index: usize,
    selected_field: RuleField,
}

#[derive(Debug, Clone)]
struct EditorConfirmState {
    title: String,
    message: String,
    action: EditorConfirmAction,
}

#[derive(Debug, Clone)]
enum EditorConfirmAction {
    ExitEditor,
    DeleteProfile { name: String },
}

impl DetailView {
    fn titles() -> [&'static str; 3] {
        ["Show", "Plan", "Doctor"]
    }

    fn index(self) -> usize {
        match self {
            DetailView::Show => 0,
            DetailView::Plan => 1,
            DetailView::Doctor => 2,
        }
    }

    fn next(self) -> Self {
        match self {
            DetailView::Show => DetailView::Plan,
            DetailView::Plan => DetailView::Doctor,
            DetailView::Doctor => DetailView::Show,
        }
    }

    fn previous(self) -> Self {
        match self {
            DetailView::Show => DetailView::Doctor,
            DetailView::Plan => DetailView::Show,
            DetailView::Doctor => DetailView::Plan,
        }
    }
}

pub struct TuiApp {
    config_doc: EditableConfigDocument,
    config: Config,
    store: StateStore,
    state: ManagedState,
    profiles: Vec<String>,
    selected_profile: usize,
    active_view: DetailView,
    message: String,
    force_apply_armed_for: Option<String>,
    editor: Option<ProfileEditorState>,
}

impl TuiApp {
    pub fn new(config_doc: EditableConfigDocument, store: StateStore) -> Result<Self> {
        let config = validate_editable_config(&config_doc)?;
        let state = store.load()?;
        let profiles = list_profiles(&config)
            .profiles
            .into_iter()
            .map(|profile| profile.name)
            .collect();

        Ok(Self {
            config_doc,
            config,
            store,
            state,
            profiles,
            selected_profile: 0,
            active_view: DetailView::Show,
            message: "Ready".to_string(),
            force_apply_armed_for: None,
            editor: None,
        })
    }

    fn selected_profile_name(&self) -> Option<&str> {
        self.profiles.get(self.selected_profile).map(String::as_str)
    }

    fn editing(&self) -> bool {
        self.editor.is_some()
    }

    fn reload_profiles(&mut self, selected_name: Option<&str>) {
        self.profiles = list_profiles(&self.config)
            .profiles
            .into_iter()
            .map(|profile| profile.name)
            .collect();

        if self.profiles.is_empty() {
            self.selected_profile = 0;
            return;
        }

        if let Some(name) = selected_name {
            if let Some(index) = self.profiles.iter().position(|profile| profile == name) {
                self.selected_profile = index;
                return;
            }
        }

        self.selected_profile = self
            .selected_profile
            .min(self.profiles.len().saturating_sub(1));
    }

    fn select_next(&mut self) {
        if self.editing() {
            self.message = "Exit edit mode before switching profiles".to_string();
            return;
        }
        self.force_apply_armed_for = None;
        if !self.profiles.is_empty() {
            self.selected_profile = (self.selected_profile + 1) % self.profiles.len();
        }
    }

    fn select_previous(&mut self) {
        if self.editing() {
            self.message = "Exit edit mode before switching profiles".to_string();
            return;
        }
        self.force_apply_armed_for = None;
        if !self.profiles.is_empty() {
            self.selected_profile = if self.selected_profile == 0 {
                self.profiles.len() - 1
            } else {
                self.selected_profile - 1
            };
        }
    }

    fn refresh(&mut self) -> Result<()> {
        if self.editing() {
            self.message = "Exit edit mode before refreshing state".to_string();
            return Ok(());
        }
        self.force_apply_armed_for = None;
        let selected = self.selected_profile_name().map(str::to_string);
        self.config_doc = load_editable_config(&self.config_doc.path)?;
        self.config = validate_editable_config(&self.config_doc)?;
        self.reload_profiles(selected.as_deref());
        self.state = self.store.load()?;
        self.message = "Refreshed state".to_string();
        Ok(())
    }

    fn apply_selected(&mut self) -> Result<()> {
        if self.editing() {
            self.message = "Exit edit mode before apply".to_string();
            return Ok(());
        }
        let Some(profile_name) = self.selected_profile_name().map(str::to_string) else {
            self.message = "No profile selected".to_string();
            return Ok(());
        };
        self.state = self.store.load()?;
        let plan = build_plan(&self.config, &profile_name, &self.state)?;
        let use_force = self.force_apply_armed_for.as_deref() == Some(profile_name.as_str());
        if !use_force && can_force_with_backup(&plan) {
            self.force_apply_armed_for = Some(profile_name.clone());
            self.message = format!(
                "Danger is forceable for '{}'. Press 'a' again to force overwrite with backup",
                profile_name
            );
            return Ok(());
        }
        let result = if use_force {
            let result = apply_plan_force_with_backup(plan, &self.state, &self.store);
            self.force_apply_armed_for = None;
            result?
        } else {
            apply_plan(plan, &self.state, &self.store)?
        };
        self.state = self.store.load()?;
        self.message = format!(
            "Applied '{}' with {} journal entries",
            profile_name,
            result.journal.entries.len()
        );
        Ok(())
    }

    fn undo(&mut self) -> Result<()> {
        if self.editing() {
            self.message = "Exit edit mode before undo".to_string();
            return Ok(());
        }
        self.force_apply_armed_for = None;
        let result = undo_last_apply(&self.store)?;
        self.state = self.store.load()?;
        self.message = format!(
            "Undid '{}' with {} reverted targets",
            result.profile_name,
            result.reverted_targets.len()
        );
        Ok(())
    }

    fn compile_required_compositions(&mut self) -> Result<()> {
        if self.editing() {
            self.message = "Exit edit mode before compiling prompt prerequisites".to_string();
            return Ok(());
        }
        self.force_apply_armed_for = None;
        let Some(profile_name) = self.selected_profile_name().map(str::to_string) else {
            self.message = "No profile selected".to_string();
            return Ok(());
        };

        let result = build_profile_requirements(&self.config, &profile_name)?;
        if result.built.is_empty() {
            self.message = format!("No prompt prerequisites for '{}'", profile_name);
        } else {
            self.message = format!(
                "Compiled {} prompt prerequisites for '{}'",
                result.built.len(),
                profile_name
            );
        }

        Ok(())
    }

    fn show_view(&self) -> Result<Option<ProfileShowView>> {
        self.selected_profile_name()
            .map(|profile_name| show_profile(&self.config, profile_name))
            .transpose()
    }

    fn plan_view(&self) -> Result<Option<ProfileExplainView>> {
        self.selected_profile_name()
            .map(|profile_name| explain_profile(&self.config, profile_name, &self.state))
            .transpose()
    }

    fn doctor_view(&self) -> Result<Option<String>> {
        self.selected_profile_name()
            .map(|profile_name| doctor_profile(&self.config, profile_name, &self.state))
            .transpose()
            .map(|report| {
                report.map(|report| {
                    if report.issues.is_empty() {
                        format!("Doctor OK for '{}'", report.profile_name)
                    } else {
                        let mut lines =
                            vec![format!("Doctor issues for '{}':", report.profile_name)];
                        for issue in report.issues {
                            lines.push(format!(
                                "- {} {}: {}",
                                issue.kind.as_str(),
                                issue.target.display(),
                                issue.message
                            ));
                        }
                        lines.join("\n")
                    }
                })
            })
    }

    fn begin_edit_selected(&mut self) -> Result<()> {
        let Some(profile_name) = self.selected_profile_name().map(str::to_string) else {
            self.message = "No profile selected".to_string();
            return Ok(());
        };
        let Some(profile) = self.config_doc.config.profiles.get(&profile_name).cloned() else {
            self.message = format!("Profile '{}' is missing from editable config", profile_name);
            return Ok(());
        };
        let draft = ProfileDraft::from_existing(&profile_name, &profile);
        self.editor = Some(ProfileEditorState {
            draft: draft.clone(),
            initial: draft,
            selected_field: EditorField::Name,
            overlay: EditorOverlay::None,
        });
        self.message = format!("Editing profile '{}'", profile_name);
        Ok(())
    }

    fn begin_create_profile(&mut self) {
        let draft = ProfileDraft::new_empty();
        self.editor = Some(ProfileEditorState {
            draft: draft.clone(),
            initial: draft,
            selected_field: EditorField::Name,
            overlay: EditorOverlay::None,
        });
        self.message = "Creating new profile".to_string();
    }

    fn prompt_delete_selected(&mut self) {
        let Some(profile_name) = self.selected_profile_name().map(str::to_string) else {
            self.message = "No profile selected".to_string();
            return;
        };
        if self.editing() {
            self.message = "Exit edit mode before deleting a profile".to_string();
            return;
        }
        self.editor = Some(ProfileEditorState {
            draft: ProfileDraft::new_empty(),
            initial: ProfileDraft::new_empty(),
            selected_field: EditorField::Name,
            overlay: EditorOverlay::Confirm(EditorConfirmState {
                title: "Delete Profile".to_string(),
                message: format!("Delete profile '{}'?", profile_name),
                action: EditorConfirmAction::DeleteProfile {
                    name: profile_name.clone(),
                },
            }),
        });
        self.message = format!("Confirm delete for '{}'", profile_name);
    }

    fn save_editor(&mut self) -> Result<()> {
        let Some(editor) = self.editor.as_ref() else {
            return Ok(());
        };
        let draft = editor.draft.normalized();
        if draft.name.is_empty() {
            self.message = "Error: profile name cannot be empty".to_string();
            return Ok(());
        }

        let mut document = self.config_doc.clone();
        if let Some(original_name) = &draft.original_name {
            document.config.profiles.remove(original_name);
        }
        if draft.original_name.as_deref() != Some(draft.name.as_str())
            && document.config.profiles.contains_key(&draft.name)
        {
            self.message = format!("Error: profile '{}' already exists", draft.name);
            return Ok(());
        }

        document
            .config
            .profiles
            .insert(draft.name.clone(), draft.to_editable_profile());

        validate_and_write_editable_config(&document)?;
        self.config_doc = load_editable_config(&document.path)?;
        self.config = validate_editable_config(&self.config_doc)?;
        self.reload_profiles(Some(draft.name.as_str()));
        self.editor = None;
        self.force_apply_armed_for = None;
        self.message = format!("Saved profile '{}'", draft.name);
        Ok(())
    }

    fn attempt_exit_editor(&mut self) -> Result<bool> {
        let Some(editor) = self.editor.as_mut() else {
            return Ok(true);
        };

        match &editor.overlay {
            EditorOverlay::TextInput(_)
            | EditorOverlay::StringList(_)
            | EditorOverlay::RuleList(_)
            | EditorOverlay::RuleEditor(_) => {
                editor.overlay = EditorOverlay::None;
                return Ok(false);
            }
            EditorOverlay::Confirm(_) => return Ok(false),
            EditorOverlay::None => {}
        }

        if editor.is_dirty() {
            editor.overlay = EditorOverlay::Confirm(EditorConfirmState {
                title: "Unsaved Changes".to_string(),
                message: "Press Enter to save, 'd' to discard, or Esc to cancel".to_string(),
                action: EditorConfirmAction::ExitEditor,
            });
            self.message = "Unsaved profile edits".to_string();
            return Ok(false);
        }

        self.editor = None;
        self.message = "Exited profile editor".to_string();
        Ok(false)
    }

    fn discard_editor(&mut self) {
        self.editor = None;
        self.message = "Discarded profile edits".to_string();
    }

    fn confirm_delete_profile(&mut self, profile_name: &str) -> Result<()> {
        let mut document = self.config_doc.clone();
        if document.config.profiles.remove(profile_name).is_none() {
            self.message = format!("Error: profile '{}' not found", profile_name);
            return Ok(());
        }

        validate_and_write_editable_config(&document)?;
        self.config_doc = load_editable_config(&document.path)?;
        self.config = validate_editable_config(&self.config_doc)?;
        self.editor = None;
        self.force_apply_armed_for = None;
        self.reload_profiles(None);
        self.message = format!("Deleted profile '{}'", profile_name);
        Ok(())
    }

    fn open_selected_editor_field(&mut self) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };

        match editor.selected_field {
            EditorField::Name => {
                editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                    "Profile Name",
                    editor.draft.name.clone(),
                    TextInputTarget::ProfileName,
                    EditorOverlay::None,
                ));
            }
            EditorField::SourceRoot => {
                editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                    "Profile Source Root",
                    editor.draft.source_root.clone().unwrap_or_default(),
                    TextInputTarget::ProfileSourceRoot,
                    EditorOverlay::None,
                ));
            }
            EditorField::Requires => {
                editor.overlay = EditorOverlay::StringList(StringListEditorState::new(
                    "Profile Requires",
                    StringListKind::Requires,
                    0,
                    EditorOverlay::None,
                ));
            }
            EditorField::Rules => {
                editor.overlay = EditorOverlay::RuleList(RuleListEditorState { selected: 0 });
            }
        }
    }

    fn handle_editor_key(&mut self, key: KeyCode) -> Result<bool> {
        if self.editor.is_none() {
            return Ok(false);
        }

        if self.handle_editor_overlay_key(key)? {
            return Ok(false);
        }

        match key {
            KeyCode::Esc | KeyCode::Char('q') => return self.attempt_exit_editor(),
            KeyCode::Char('s') => {
                return handle_app_action(self, |app| app.save_editor()).map(|_| false);
            }
            KeyCode::Char('a') | KeyCode::Char('c') | KeyCode::Char('u') | KeyCode::Char('r') => {
                self.message = "Exit edit mode before running reconcile actions".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.selected_field = editor.selected_field.next();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = self.editor.as_mut() {
                    editor.selected_field = editor.selected_field.previous();
                }
            }
            KeyCode::Enter => self.open_selected_editor_field(),
            _ => {}
        }

        Ok(false)
    }

    fn handle_editor_overlay_key(&mut self, key: KeyCode) -> Result<bool> {
        let Some(editor) = self.editor.as_mut() else {
            return Ok(false);
        };

        match editor.overlay.clone() {
            EditorOverlay::None => Ok(false),
            EditorOverlay::TextInput(state) => {
                self.handle_text_input_key(state, key)?;
                Ok(true)
            }
            EditorOverlay::StringList(state) => {
                self.handle_string_list_key(state, key);
                Ok(true)
            }
            EditorOverlay::RuleList(state) => {
                self.handle_rule_list_key(state, key);
                Ok(true)
            }
            EditorOverlay::RuleEditor(state) => {
                self.handle_rule_editor_key(state, key);
                Ok(true)
            }
            EditorOverlay::Confirm(state) => {
                self.handle_confirm_key(state, key)?;
                Ok(true)
            }
        }
    }

    fn handle_text_input_key(&mut self, mut state: TextInputState, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Esc => {
                let return_to = (*state.return_to).clone();
                if let Some(editor) = self.editor.as_mut() {
                    editor.overlay = return_to;
                }
                return Ok(());
            }
            KeyCode::Tab => {
                self.autocomplete_text_input(&mut state)?;
            }
            KeyCode::Left => state.move_left(),
            KeyCode::Right => state.move_right(),
            KeyCode::Home => state.move_home(),
            KeyCode::End => state.move_end(),
            KeyCode::Backspace => {
                state.backspace();
            }
            KeyCode::Delete => {
                state.delete();
            }
            KeyCode::Char(c) if !c.is_control() => {
                state.insert(c);
            }
            KeyCode::Enter => {
                self.apply_text_input(state)?;
                return Ok(());
            }
            _ => {}
        }

        if let Some(editor) = self.editor.as_mut() {
            editor.overlay = EditorOverlay::TextInput(state);
        }

        Ok(())
    }

    fn apply_text_input(&mut self, state: TextInputState) -> Result<()> {
        let Some(editor) = self.editor.as_mut() else {
            return Ok(());
        };

        let return_to = (*state.return_to).clone();
        let trimmed = state.value.trim().to_string();
        match state.target {
            TextInputTarget::ProfileName => editor.draft.name = trimmed,
            TextInputTarget::ProfileSourceRoot => {
                editor.draft.source_root = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                };
            }
            TextInputTarget::RequiresEntry { index } => {
                if trimmed.is_empty() {
                    self.message = "Error: required composition name cannot be empty".to_string();
                    editor.overlay = EditorOverlay::TextInput(state);
                    return Ok(());
                }
                upsert_string_entry(&mut editor.draft.requires, index, trimmed);
                editor.overlay = overlay_with_selected(
                    return_to,
                    index.unwrap_or_else(|| editor.draft.requires.len().saturating_sub(1)),
                );
                return Ok(());
            }
            TextInputTarget::RuleSelect { rule_index } => {
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    rule.select = trimmed;
                }
            }
            TextInputTarget::RuleNote { rule_index } => {
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    rule.note = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    };
                }
            }
            TextInputTarget::RuleDestination { rule_index, index } => {
                if trimmed.is_empty() {
                    self.message = "Error: destination cannot be empty".to_string();
                    editor.overlay = EditorOverlay::TextInput(state);
                    return Ok(());
                }
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    upsert_string_entry(&mut rule.to, index, trimmed);
                    editor.overlay = overlay_with_selected(
                        return_to,
                        index.unwrap_or_else(|| rule.to.len().saturating_sub(1)),
                    );
                    return Ok(());
                }
            }
            TextInputTarget::RuleTag { rule_index, index } => {
                if trimmed.is_empty() {
                    self.message = "Error: tag cannot be empty".to_string();
                    editor.overlay = EditorOverlay::TextInput(state);
                    return Ok(());
                }
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    upsert_string_entry(&mut rule.tags, index, trimmed);
                    editor.overlay = overlay_with_selected(
                        return_to,
                        index.unwrap_or_else(|| rule.tags.len().saturating_sub(1)),
                    );
                    return Ok(());
                }
            }
        }

        editor.overlay = return_to;
        Ok(())
    }

    fn handle_string_list_key(&mut self, mut state: StringListEditorState, key: KeyCode) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };

        let values = match state.kind {
            StringListKind::Requires => &mut editor.draft.requires,
            StringListKind::RuleDestinations { rule_index } => {
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    &mut rule.to
                } else {
                    editor.overlay = EditorOverlay::None;
                    return;
                }
            }
            StringListKind::RuleTags { rule_index } => {
                if let Some(rule) = editor.draft.rules.get_mut(rule_index) {
                    &mut rule.tags
                } else {
                    editor.overlay = EditorOverlay::None;
                    return;
                }
            }
        };

        match key {
            KeyCode::Esc => {
                editor.overlay = (*state.return_to).clone();
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => state.selected = state.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected = state
                    .selected
                    .saturating_add(1)
                    .min(values.len().saturating_sub(1))
            }
            KeyCode::Char('J') => move_entry_down(values, &mut state.selected),
            KeyCode::Char('K') => move_entry_up(values, &mut state.selected),
            KeyCode::Backspace | KeyCode::Delete => {
                if state.selected < values.len() {
                    values.remove(state.selected);
                    state.selected = state.selected.min(values.len().saturating_sub(1));
                }
            }
            KeyCode::Char('a') => {
                editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                    state.title.clone(),
                    String::new(),
                    state.kind.add_target(None),
                    EditorOverlay::StringList(state.clone()),
                ));
                return;
            }
            KeyCode::Enter => {
                if let Some(current) = values.get(state.selected).cloned() {
                    editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                        state.title.clone(),
                        current,
                        state.kind.add_target(Some(state.selected)),
                        EditorOverlay::StringList(state.clone()),
                    ));
                    return;
                }
            }
            _ => {}
        }

        editor.overlay = EditorOverlay::StringList(state);
    }

    fn handle_rule_list_key(&mut self, mut state: RuleListEditorState, key: KeyCode) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let rules = &mut editor.draft.rules;

        match key {
            KeyCode::Esc => {
                editor.overlay = EditorOverlay::None;
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => state.selected = state.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected = state
                    .selected
                    .saturating_add(1)
                    .min(rules.len().saturating_sub(1))
            }
            KeyCode::Char('J') => move_entry_down(rules, &mut state.selected),
            KeyCode::Char('K') => move_entry_up(rules, &mut state.selected),
            KeyCode::Backspace | KeyCode::Delete => {
                if state.selected < rules.len() {
                    rules.remove(state.selected);
                    state.selected = state.selected.min(rules.len().saturating_sub(1));
                }
            }
            KeyCode::Char(' ') => {
                if let Some(rule) = rules.get_mut(state.selected) {
                    rule.enabled = !rule.enabled;
                }
            }
            KeyCode::Char('a') => {
                rules.push(default_editable_rule());
                let index = rules.len().saturating_sub(1);
                editor.overlay = EditorOverlay::RuleEditor(RuleEditorState {
                    rule_index: index,
                    selected_field: RuleField::Select,
                });
                return;
            }
            KeyCode::Enter => {
                if state.selected < rules.len() {
                    editor.overlay = EditorOverlay::RuleEditor(RuleEditorState {
                        rule_index: state.selected,
                        selected_field: RuleField::Select,
                    });
                    return;
                }
            }
            _ => {}
        }

        editor.overlay = EditorOverlay::RuleList(state);
    }

    fn handle_rule_editor_key(&mut self, mut state: RuleEditorState, key: KeyCode) {
        let Some(editor) = self.editor.as_mut() else {
            return;
        };
        let Some(rule) = editor.draft.rules.get_mut(state.rule_index) else {
            editor.overlay = EditorOverlay::RuleList(RuleListEditorState { selected: 0 });
            return;
        };

        match key {
            KeyCode::Esc => {
                editor.overlay = EditorOverlay::RuleList(RuleListEditorState {
                    selected: state.rule_index,
                });
                return;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                state.selected_field = state.selected_field.previous()
            }
            KeyCode::Down | KeyCode::Char('j') => {
                state.selected_field = state.selected_field.next()
            }
            KeyCode::Enter => match state.selected_field {
                RuleField::Select => {
                    editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                        format!("Rule {} Select", state.rule_index + 1),
                        rule.select.clone(),
                        TextInputTarget::RuleSelect {
                            rule_index: state.rule_index,
                        },
                        EditorOverlay::RuleEditor(state.clone()),
                    ));
                    return;
                }
                RuleField::Mode => {
                    rule.mode = next_mode(&rule.mode).to_string();
                }
                RuleField::Enabled => {
                    rule.enabled = !rule.enabled;
                }
                RuleField::Note => {
                    editor.overlay = EditorOverlay::TextInput(TextInputState::new(
                        format!("Rule {} Note", state.rule_index + 1),
                        rule.note.clone().unwrap_or_default(),
                        TextInputTarget::RuleNote {
                            rule_index: state.rule_index,
                        },
                        EditorOverlay::RuleEditor(state.clone()),
                    ));
                    return;
                }
                RuleField::Destinations => {
                    editor.overlay = EditorOverlay::StringList(StringListEditorState::new(
                        format!("Rule {} Destinations", state.rule_index + 1),
                        StringListKind::RuleDestinations {
                            rule_index: state.rule_index,
                        },
                        0,
                        EditorOverlay::RuleEditor(state.clone()),
                    ));
                    return;
                }
                RuleField::Tags => {
                    editor.overlay = EditorOverlay::StringList(StringListEditorState::new(
                        format!("Rule {} Tags", state.rule_index + 1),
                        StringListKind::RuleTags {
                            rule_index: state.rule_index,
                        },
                        0,
                        EditorOverlay::RuleEditor(state.clone()),
                    ));
                    return;
                }
            },
            KeyCode::Char(' ') if state.selected_field == RuleField::Enabled => {
                rule.enabled = !rule.enabled;
            }
            _ => {}
        }

        editor.overlay = EditorOverlay::RuleEditor(state);
    }

    fn handle_confirm_key(&mut self, state: EditorConfirmState, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Esc => match state.action {
                EditorConfirmAction::ExitEditor => {
                    if let Some(editor) = self.editor.as_mut() {
                        editor.overlay = EditorOverlay::None;
                    }
                }
                EditorConfirmAction::DeleteProfile { .. } => {
                    self.editor = None;
                    self.message = "Canceled profile delete".to_string();
                }
            },
            KeyCode::Enter => match state.action {
                EditorConfirmAction::ExitEditor => {
                    if let Some(editor) = self.editor.as_mut() {
                        editor.overlay = EditorOverlay::None;
                    }
                    self.save_editor()?;
                }
                EditorConfirmAction::DeleteProfile { ref name } => {
                    self.confirm_delete_profile(name)?;
                }
            },
            KeyCode::Char('d') if matches!(state.action, EditorConfirmAction::ExitEditor) => {
                self.discard_editor();
            }
            _ => {}
        }

        Ok(())
    }

    fn autocomplete_text_input(&mut self, state: &mut TextInputState) -> Result<()> {
        let Some(base_dir) = self.autocomplete_base_dir(&state.target) else {
            return Ok(());
        };

        if let Some(completion) = state.completion.as_mut()
            && state.value == completion.matches[completion.selected]
            && !completion.matches.is_empty()
        {
            completion.selected = (completion.selected + 1) % completion.matches.len();
            state.value = completion.matches[completion.selected].clone();
            state.cursor = state.value.chars().count();
            return Ok(());
        }

        let raw = state.value.clone();
        let (dir_part, prefix) = split_path_for_completion(&raw);
        let parent_dir = if dir_part.is_empty() {
            base_dir
        } else {
            resolve_input_path(&dir_part, &base_dir)?
        };

        if !parent_dir.is_dir() {
            state.completion = None;
            return Ok(());
        }

        let prefer_hidden = prefix.starts_with('.');
        let mut matches = fs::read_dir(&parent_dir)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(&prefix) {
                    let is_dir = entry.path().is_dir();
                    let mut display = if dir_part.is_empty() {
                        name.clone()
                    } else {
                        format!("{dir_part}/{name}")
                    };
                    if is_dir {
                        display.push('/');
                    }
                    Some((name, display, is_dir))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| {
            let left_hidden = left.0.starts_with('.');
            let right_hidden = right.0.starts_with('.');
            let left_hidden_rank = if prefer_hidden {
                !left_hidden
            } else {
                left_hidden
            };
            let right_hidden_rank = if prefer_hidden {
                !right_hidden
            } else {
                right_hidden
            };

            left_hidden_rank
                .cmp(&right_hidden_rank)
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| left.0.cmp(&right.0))
        });

        if matches.is_empty() {
            state.completion = None;
            return Ok(());
        }

        let resolved_matches = matches.into_iter().map(|item| item.1).collect::<Vec<_>>();
        state.completion = Some(CompletionState {
            matches: resolved_matches,
            selected: 0,
        });
        if let Some(completion) = &state.completion {
            state.value = completion.matches[0].clone();
            state.cursor = state.value.chars().count();
        }
        Ok(())
    }

    fn autocomplete_base_dir(&self, target: &TextInputTarget) -> Option<PathBuf> {
        match target {
            TextInputTarget::ProfileSourceRoot | TextInputTarget::RuleDestination { .. } => {
                Some(self.config_doc.config_dir.clone())
            }
            _ => None,
        }
    }
}

impl TextInputState {
    fn new(
        title: impl Into<String>,
        value: String,
        target: TextInputTarget,
        return_to: EditorOverlay,
    ) -> Self {
        let cursor = value.chars().count();
        Self {
            title: title.into(),
            value,
            cursor,
            target,
            return_to: Box::new(return_to),
            completion: None,
        }
    }

    fn byte_index(line: &str, col: usize) -> usize {
        line.char_indices()
            .nth(col)
            .map(|(index, _)| index)
            .unwrap_or(line.len())
    }

    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.value.chars().count();
    }

    fn insert(&mut self, c: char) {
        let index = Self::byte_index(&self.value, self.cursor);
        self.value.insert(index, c);
        self.cursor += 1;
        self.completion = None;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = Self::byte_index(&self.value, self.cursor.saturating_sub(1));
        let end = Self::byte_index(&self.value, self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor = self.cursor.saturating_sub(1);
        self.completion = None;
    }

    fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = Self::byte_index(&self.value, self.cursor);
        let end = Self::byte_index(&self.value, self.cursor + 1);
        self.value.replace_range(start..end, "");
        self.completion = None;
    }
}

impl StringListEditorState {
    fn new(
        title: impl Into<String>,
        kind: StringListKind,
        selected: usize,
        return_to: EditorOverlay,
    ) -> Self {
        Self {
            title: title.into(),
            kind,
            selected,
            return_to: Box::new(return_to),
        }
    }
}

impl StringListKind {
    fn add_target(&self, index: Option<usize>) -> TextInputTarget {
        match self {
            StringListKind::Requires => TextInputTarget::RequiresEntry { index },
            StringListKind::RuleDestinations { rule_index } => TextInputTarget::RuleDestination {
                rule_index: *rule_index,
                index,
            },
            StringListKind::RuleTags { rule_index } => TextInputTarget::RuleTag {
                rule_index: *rule_index,
                index,
            },
        }
    }
}

fn upsert_string_entry(values: &mut Vec<String>, index: Option<usize>, value: String) {
    match index {
        Some(index) if index < values.len() => values[index] = value,
        _ => values.push(value),
    }
}

fn overlay_with_selected(overlay: EditorOverlay, selected: usize) -> EditorOverlay {
    match overlay {
        EditorOverlay::StringList(mut state) => {
            state.selected = selected;
            EditorOverlay::StringList(state)
        }
        other => other,
    }
}

fn move_entry_up<T>(values: &mut [T], selected: &mut usize) {
    if *selected > 0 && *selected < values.len() {
        values.swap(*selected, *selected - 1);
        *selected -= 1;
    }
}

fn move_entry_down<T>(values: &mut [T], selected: &mut usize) {
    if *selected + 1 < values.len() {
        values.swap(*selected, *selected + 1);
        *selected += 1;
    }
}

fn default_editable_rule() -> EditableRule {
    EditableRule {
        select: String::new(),
        to: Vec::new(),
        mode: MaterializationMode::Symlink.as_str().to_string(),
        enabled: true,
        tags: Vec::new(),
        note: None,
    }
}

fn next_mode(current: &str) -> &'static str {
    match current {
        "symlink" => "copy",
        "copy" => "hardlink",
        _ => "symlink",
    }
}

fn split_path_for_completion(raw: &str) -> (String, String) {
    let normalized = raw.replace('\\', "/");
    if normalized.ends_with('/') {
        return (normalized.trim_end_matches('/').to_string(), String::new());
    }

    match normalized.rsplit_once('/') {
        Some((dir, prefix)) => (dir.to_string(), prefix.to_string()),
        None => (String::new(), normalized),
    }
}

pub fn run_tui(config_doc: EditableConfigDocument, store: StateStore) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(config_doc, store)?;
    let result = run_event_loop(&mut terminal, &mut app);
    restore_terminal(terminal)?;
    result
}

fn run_event_loop(terminal: &mut AppTerminal, app: &mut TuiApp) -> Result<()> {
    loop {
        terminal.draw(|frame| draw_ui(frame.area(), frame.buffer_mut(), app))?;

        if !event::poll(std::time::Duration::from_millis(200))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.handle_key(key.code)? {
                break;
            }
        }
    }

    Ok(())
}

impl TuiApp {
    fn handle_key(&mut self, key: KeyCode) -> Result<bool> {
        if self.editing() {
            return self.handle_editor_key(key);
        }

        match key {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.active_view = self.active_view.next()
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.active_view = self.active_view.previous()
            }
            KeyCode::Char('r') => handle_app_action(self, |app| app.refresh())?,
            KeyCode::Char('c') => {
                handle_app_action(self, |app| app.compile_required_compositions())?
            }
            KeyCode::Char('a') => handle_app_action(self, |app| app.apply_selected())?,
            KeyCode::Char('u') => handle_app_action(self, |app| app.undo())?,
            KeyCode::Char('e') => handle_app_action(self, |app| app.begin_edit_selected())?,
            KeyCode::Char('n') => self.begin_create_profile(),
            KeyCode::Char('d') => self.prompt_delete_selected(),
            _ => {}
        }

        Ok(false)
    }
}

fn handle_app_action(
    app: &mut TuiApp,
    action: impl FnOnce(&mut TuiApp) -> Result<()>,
) -> Result<()> {
    match action(app) {
        Ok(()) => Ok(()),
        Err(error) => {
            app.message = format!("Error: {error:#}");
            Ok(())
        }
    }
}

fn draw_ui(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(area);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(outer[1]);

    let header = Paragraph::new("SSOT Manager TUI")
        .block(Block::default().borders(Borders::ALL).title("Header"));
    header.render(outer[0], buf);

    render_profile_list(body[0], buf, app);
    render_detail(body[1], buf, app);
    render_editor_overlay(body[1], buf, app);

    let footer = Paragraph::new(Text::from(vec![
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(app.message.clone(), style_for_message(&app.message)),
        ]),
        Line::from(footer_keys(app)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Status / Keys"),
    )
    .wrap(Wrap { trim: true });
    footer.render(outer[2], buf);
}

fn render_profile_list(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let items = if app.profiles.is_empty() {
        vec![ListItem::new("No profiles configured")]
    } else {
        app.profiles
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                let line = if index == app.selected_profile {
                    Line::styled(
                        format!("> {profile}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {profile}"))
                };
                ListItem::new(line)
            })
            .collect()
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Profiles"));
    list.render(area, buf);
}

fn render_detail(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    if let Some(editor) = &app.editor {
        render_editor_detail(area, buf, editor);
        return;
    }

    let detail = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(area);
    let tabs = Tabs::new(DetailView::titles())
        .select(app.active_view.index())
        .block(Block::default().borders(Borders::ALL).title("View"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    tabs.render(detail[0], buf);

    let text = match app.active_view {
        DetailView::Show => app
            .show_view()
            .map(|view| {
                view.map(render_show_text)
                    .unwrap_or_else(|| Text::from("No profile selected"))
            })
            .unwrap_or_else(|error| Text::from(format!("Error: {error:#}"))),
        DetailView::Plan => app
            .plan_view()
            .map(|view| {
                view.map(render_plan_text)
                    .unwrap_or_else(|| Text::from("No profile selected"))
            })
            .unwrap_or_else(|error| Text::from(format!("Error: {error:#}"))),
        DetailView::Doctor => app
            .doctor_view()
            .map(|view| {
                render_doctor_text(view.unwrap_or_else(|| "No profile selected".to_string()))
            })
            .unwrap_or_else(|error| Text::from(format!("Error: {error:#}"))),
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Detail"))
        .wrap(Wrap { trim: false });
    paragraph.render(detail[1], buf);
}

fn render_editor_overlay(area: Rect, buf: &mut ratatui::buffer::Buffer, app: &TuiApp) {
    let Some(editor) = &app.editor else {
        return;
    };

    match &editor.overlay {
        EditorOverlay::None => {}
        EditorOverlay::TextInput(state) => render_text_input_overlay(area, buf, state),
        EditorOverlay::StringList(state) => render_string_list_overlay(area, buf, editor, state),
        EditorOverlay::RuleList(state) => render_rule_list_overlay(area, buf, editor, state),
        EditorOverlay::RuleEditor(state) => render_rule_editor_overlay(area, buf, editor, state),
        EditorOverlay::Confirm(state) => render_confirm_overlay(area, buf, state),
    }
}

fn render_text_input_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    state: &TextInputState,
) {
    let completion_lines = state
        .completion
        .as_ref()
        .map(|completion| completion.matches.len().min(4) as u16)
        .unwrap_or(0);
    let popup = centered_rect(area, 70, 8 + completion_lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner);
    Paragraph::new(text_input_hint(state))
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    let input = Paragraph::new(state.value.clone()).block(Block::default().borders(Borders::ALL));
    input.render(chunks[1], buf);
    if let Some(completion) = &state.completion {
        let lines = completion
            .matches
            .iter()
            .take(4)
            .enumerate()
            .map(|(index, value)| {
                if index == completion.selected {
                    Line::styled(
                        format!("> {value}"),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {value}"))
                }
            })
            .collect::<Vec<_>>();
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: true })
            .render(chunks[2], buf);
    }
}

fn render_string_list_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &StringListEditorState,
) {
    let popup = centered_rect(area, 75, 14);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let values = match state.kind {
        StringListKind::Requires => &editor.draft.requires,
        StringListKind::RuleDestinations { rule_index } => editor
            .draft
            .rules
            .get(rule_index)
            .map(|rule| &rule.to)
            .unwrap_or(&editor.draft.requires),
        StringListKind::RuleTags { rule_index } => editor
            .draft
            .rules
            .get(rule_index)
            .map(|rule| &rule.tags)
            .unwrap_or(&editor.draft.requires),
    };

    let items = if values.is_empty() {
        vec![ListItem::new("No entries")]
    } else {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                ListItem::new(if index == state.selected {
                    Line::styled(
                        format!("> {value}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    )
                } else {
                    Line::from(format!("  {value}"))
                })
            })
            .collect()
    };
    Paragraph::new("a add | Enter edit | Del remove | J/K reorder | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    List::new(items).render(chunks[1], buf);
}

fn render_rule_list_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &RuleListEditorState,
) {
    let popup = centered_rect(area, 85, 16);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Profile Rules");
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let items = if editor.draft.rules.is_empty() {
        vec![ListItem::new("No rules")]
    } else {
        editor
            .draft
            .rules
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let prefix = if index == state.selected { "> " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(
                        prefix.to_string(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!(
                        "{} [{}] to={} ",
                        rule.select,
                        rule.mode,
                        rule.to.len()
                    )),
                    Span::styled(
                        if rule.enabled { "enabled" } else { "disabled" },
                        Style::default()
                            .fg(if rule.enabled {
                                Color::Green
                            } else {
                                Color::Red
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect()
    };
    Paragraph::new("Space toggle | a add | Enter edit | Del remove | J/K reorder | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    List::new(items).render(chunks[1], buf);
}

fn render_rule_editor_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
    state: &RuleEditorState,
) {
    let Some(rule) = editor.draft.rules.get(state.rule_index) else {
        return;
    };
    let popup = centered_rect(area, 85, 15);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Rule {}", state.rule_index + 1));
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let lines = vec![
        editor_line(
            state.selected_field == RuleField::Select,
            "select",
            &rule.select,
        ),
        editor_line(state.selected_field == RuleField::Mode, "mode", &rule.mode),
        editor_status_line(state.selected_field == RuleField::Enabled, rule.enabled),
        editor_line(
            state.selected_field == RuleField::Note,
            "note",
            rule.note.as_deref().unwrap_or("<none>"),
        ),
        editor_line(
            state.selected_field == RuleField::Destinations,
            "destinations",
            &format!("{} entries", rule.to.len()),
        ),
        editor_line(
            state.selected_field == RuleField::Tags,
            "tags",
            &format!("{} entries", rule.tags.len()),
        ),
    ];
    Paragraph::new("Enter edit/action | Space toggle enabled | Esc back")
        .style(Style::default().fg(Color::Yellow))
        .render(chunks[0], buf);
    Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .render(chunks[1], buf);
}

fn render_confirm_overlay(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    state: &EditorConfirmState,
) {
    let popup = centered_rect(area, 65, 7);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(state.title.clone());
    block.clone().render(popup, buf);
    let inner = block.inner(popup);
    Paragraph::new(state.message.clone())
        .wrap(Wrap { trim: true })
        .render(inner, buf);
}

fn centered_rect(area: Rect, width_percent: u16, height: u16) -> Rect {
    let width = area.width.saturating_mul(width_percent).saturating_div(100);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.max(10), height.min(area.height))
}

fn editor_line(selected: bool, label: &str, value: &str) -> Line<'static> {
    if selected {
        Line::styled(
            format!("> {label}: {value}"),
            Style::default().add_modifier(Modifier::BOLD),
        )
    } else {
        Line::from(format!("  {label}: {value}"))
    }
}

fn editor_status_line(selected: bool, enabled: bool) -> Line<'static> {
    let marker = if selected { ">" } else { " " };
    let value = if enabled { "enabled" } else { "disabled" };
    let color = if enabled { Color::Green } else { Color::Red };
    Line::from(vec![
        Span::raw(format!("{marker} enabled: ")),
        Span::styled(
            value.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn text_input_hint(state: &TextInputState) -> &'static str {
    match state.target {
        TextInputTarget::ProfileSourceRoot | TextInputTarget::RuleDestination { .. } => {
            "Enter save | Tab cycle path matches | Esc back"
        }
        _ => "Enter save | Esc back",
    }
}

fn footer_keys(app: &TuiApp) -> &'static str {
    if app.editing() {
        "Keys: j/k or Up/Down move | Enter edit/open | s save | Esc close/back | d confirm discard"
    } else {
        "Keys: q quit | j/k or Up/Down move | Tab or h/l switch view | e edit | n new | d delete | c compile deps | a apply | u undo | r refresh"
    }
}

fn render_editor_detail(
    area: Rect,
    buf: &mut ratatui::buffer::Buffer,
    editor: &ProfileEditorState,
) {
    let lines = vec![
        editor_line(
            editor.selected_field == EditorField::Name,
            "name",
            &editor.draft.name,
        ),
        editor_line(
            editor.selected_field == EditorField::SourceRoot,
            "source_root",
            editor.draft.source_root.as_deref().unwrap_or("<default>"),
        ),
        editor_line(
            editor.selected_field == EditorField::Requires,
            "requires",
            &format!("{} entries", editor.draft.requires.len()),
        ),
        editor_line(
            editor.selected_field == EditorField::Rules,
            "rules",
            &format!("{} entries", editor.draft.rules.len()),
        ),
        Line::from(""),
        Line::from("Press Enter to edit the selected field."),
        Line::from("List fields open focused collection editors."),
        Line::from("Press 's' to save or Esc to leave edit mode."),
    ];

    let paragraph = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Profile Editor"),
        )
        .wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

fn render_show_text(view: ProfileShowView) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("Profile '{}'", view.profile_name)),
        Line::from(format!("source_root={}", view.source_root)),
        Line::from(format!(
            "rules={} enabled={} disabled={}",
            view.rule_count, view.enabled_rule_count, view.disabled_rule_count
        )),
    ];

    if view.required_compositions.is_empty() {
        lines.push(Line::from("requires: none"));
    } else {
        lines.push(Line::from("requires:"));
        for requirement in view.required_compositions {
            lines.push(Line::from(vec![
                Span::raw(format!("  {} [", requirement.name)),
                Span::styled(
                    requirement.status.clone(),
                    style_for_requirement_status(&requirement.status),
                ),
                Span::raw(format!(
                    "] {} ({})",
                    requirement.output, requirement.message
                )),
            ]));
        }
    }

    for rule in view.rules {
        lines.push(Line::from(format!(
            "rule {} select={} mode={} enabled={}",
            rule.index, rule.select, rule.mode, rule.enabled
        )));
        for destination in rule.destinations {
            lines.push(Line::from(format!("  to {destination}")));
        }
        if !rule.tags.is_empty() {
            lines.push(Line::from(format!("  tags {}", rule.tags.join(","))));
        }
        if let Some(note) = rule.note {
            lines.push(Line::from(format!("  note {note}")));
        }
    }

    Text::from(lines)
}

fn render_plan_text(view: ProfileExplainView) -> Text<'static> {
    let mut lines = vec![
        Line::from(format!("Explain '{}'", view.profile_name)),
        Line::from(format!("source_root={}", view.source_root)),
    ];

    if view.required_compositions.is_empty() {
        lines.push(Line::from("required compositions: none"));
    } else {
        lines.push(Line::from("required compositions:"));
        for requirement in view.required_compositions {
            lines.push(Line::from(vec![
                Span::raw(format!("  {} [", requirement.name)),
                Span::styled(
                    requirement.status.clone(),
                    style_for_requirement_status(&requirement.status),
                ),
                Span::raw(format!(
                    "] {} ({})",
                    requirement.output, requirement.message
                )),
            ]));
        }
    }

    if view.diagnostics.is_empty() {
        lines.push(Line::from("diagnostics: none"));
    } else {
        lines.push(Line::from("diagnostics:"));
        for diagnostic in view.diagnostics {
            lines.push(Line::from(vec![
                Span::raw("  ["),
                Span::styled(diagnostic.code, Style::default().fg(Color::Yellow)),
                Span::raw(format!("] {}", diagnostic.message)),
            ]));
        }
    }

    let mut summary_spans = vec![Span::raw("plan summary: ")];
    for (index, action) in ["create", "update", "remove", "skip", "warning", "danger"]
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            summary_spans.push(Span::raw(" "));
        }
        summary_spans.push(Span::styled(
            format!(
                "{action}={}",
                view.plan_summary.get(action).copied().unwrap_or_default()
            ),
            style_for_action_name(action),
        ));
    }
    lines.push(Line::from(summary_spans));

    if view.plan_items.is_empty() {
        lines.push(Line::from("plan items: none"));
    } else {
        lines.push(Line::from("plan items:"));
        for item in view.plan_items {
            match item.desired_source {
                Some(source) => lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        action_label(&item.action, item.forceable),
                        style_for_action_name(&item.action),
                    ),
                    Span::raw(format!(" {} -> {} ({})", item.target, source, item.reason)),
                ])),
                None => lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        action_label(&item.action, item.forceable),
                        style_for_action_name(&item.action),
                    ),
                    Span::raw(format!(" {} ({})", item.target, item.reason)),
                ])),
            }
        }
    }

    Text::from(lines)
}

fn render_doctor_text(view: String) -> Text<'static> {
    let has_issue = view.contains("issues for");
    let style = if has_issue {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let lines = view
        .lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn style_for_requirement_status(status: &str) -> Style {
    match status {
        "ready" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "stale" => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        "missing" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn style_for_action_name(action: &str) -> Style {
    match action {
        "create" | "skip" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "update" | "remove" | "warning" => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        "danger" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn style_for_message(message: &str) -> Style {
    if message.starts_with("Error:") || message.contains("danger") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if message.contains("Compiled")
        || message.contains("Applied")
        || message.contains("Saved")
        || message.contains("Deleted")
        || message == "Ready"
    {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Yellow)
    }
}

fn action_label(action: &str, forceable: bool) -> String {
    if action == "danger" && forceable {
        format!("{:<7}", "danger*")
    } else {
        format!("{:<7}", action)
    }
}

fn setup_terminal() -> Result<AppTerminal> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).context("failed to initialize terminal")
}

fn restore_terminal(mut terminal: AppTerminal) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to restore cursor")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::buffer::Buffer;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn render_shows_profiles_and_detail_content() {
        let harness = Harness::new();
        let app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));

        draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &app);

        let rendered = buffer_to_string(&buffer);
        assert!(rendered.contains("Profiles"));
        assert!(rendered.contains("primary"));
        assert!(rendered.contains("Profile 'primary'"));
        assert!(rendered.contains("Status: Ready"));
        assert!(rendered.contains("a apply"));
        assert!(rendered.contains("c compile deps"));
    }

    #[test]
    fn apply_selected_updates_message_and_creates_symlink() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        app.apply_selected().unwrap();

        assert!(app.message.contains("Applied 'primary'"));
        assert!(
            fs::symlink_metadata(harness.dest_root().join("skills/alpha"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn apply_selected_preserves_danger_blocking() {
        let harness = Harness::new();
        fs::create_dir_all(harness.dest_root().join("manual")).unwrap();
        fs::write(harness.dest_root().join("manual/notes.md"), "manual").unwrap();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        handle_app_action(&mut app, |app| app.apply_selected()).unwrap();

        assert!(app.message.contains("Press 'a' again"));
        assert_eq!(app.force_apply_armed_for.as_deref(), Some("primary"));
    }

    #[test]
    fn second_apply_executes_force_with_backup_for_forceable_danger() {
        let harness = Harness::new();
        fs::create_dir_all(harness.dest_root().join("manual")).unwrap();
        fs::write(harness.dest_root().join("manual/notes.md"), "manual").unwrap();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        handle_app_action(&mut app, |app| app.apply_selected()).unwrap();
        handle_app_action(&mut app, |app| app.apply_selected()).unwrap();

        assert!(app.message.contains("Applied 'primary'"));
        assert!(app.force_apply_armed_for.is_none());
        assert_eq!(
            fs::read_to_string(harness.dest_root().join("manual/notes.md")).unwrap(),
            "notes"
        );
    }

    #[test]
    fn force_confirmation_resets_when_selection_changes() {
        let harness = Harness::new();
        fs::create_dir_all(harness.dest_root().join("manual")).unwrap();
        fs::write(harness.dest_root().join("manual/notes.md"), "manual").unwrap();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        handle_app_action(&mut app, |app| app.apply_selected()).unwrap();
        app.select_next();

        assert!(app.force_apply_armed_for.is_none());
    }

    #[test]
    fn undo_updates_message_after_apply() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.apply_selected().unwrap();

        app.undo().unwrap();

        assert!(app.message.contains("Undid 'primary'"));
        assert!(!harness.dest_root().join("skills/alpha").exists());
    }

    #[test]
    fn render_surfaces_latest_status_message() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.apply_selected().unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));

        draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &app);

        let rendered = buffer_to_string(&buffer);
        assert!(rendered.contains("Status: Applied 'primary'"));
    }

    #[test]
    fn show_view_includes_prompt_prerequisites_for_selected_profile() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.select_next();

        let rendered = text_to_plain_string(&render_show_text(app.show_view().unwrap().unwrap()));

        assert!(rendered.contains("requires:"));
        assert!(rendered.contains("agent [missing]"));
        assert!(rendered.contains("build/prompts/AGENTS.generated.md"));
    }

    #[test]
    fn compile_required_compositions_updates_message_and_materializes_output() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.select_next();

        app.compile_required_compositions().unwrap();

        assert!(
            app.message
                .contains("Compiled 1 prompt prerequisites for 'prompted'")
        );
        assert!(
            harness
                .temp
                .path()
                .join("source/build/prompts/AGENTS.generated.md")
                .exists()
        );
    }

    #[test]
    fn begin_edit_selected_opens_profile_editor() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        app.begin_edit_selected().unwrap();

        let editor = app.editor.as_ref().expect("editor should open");
        assert_eq!(editor.draft.name, "primary");
        assert_eq!(editor.selected_field, EditorField::Name);
        assert!(app.message.contains("Editing profile 'primary'"));
    }

    #[test]
    fn create_profile_and_nested_edits_can_be_saved() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

        app.begin_create_profile();
        {
            let editor = app.editor.as_mut().unwrap();
            editor.draft.name = "created".to_string();
            editor.draft.requires.push("agent".to_string());
            editor.draft.rules.push(default_editable_rule());
            let rule = editor.draft.rules.get_mut(0).unwrap();
            rule.select = "Notes/notes.md".to_string();
            rule.to.push(format!(
                "{}/created/notes.md",
                harness.dest_root().display()
            ));
            rule.tags.push("notes".to_string());
            rule.note = Some("created by tui".to_string());
        }

        app.save_editor().unwrap();

        assert_eq!(app.selected_profile_name(), Some("created"));
        assert!(app.config_doc.config.profiles.contains_key("created"));
        let saved = fs::read_to_string(harness.temp.path().join("config.yaml")).unwrap();
        assert!(saved.contains("created:"));
        assert!(saved.contains("requires:"));
        assert!(saved.contains("- agent"));
        assert!(saved.contains("tags:"));
    }

    #[test]
    fn dirty_editor_exit_opens_unsaved_confirmation() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().draft.name = "renamed".to_string();

        let result = app.attempt_exit_editor().unwrap();

        assert!(!result);
        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::Confirm(EditorConfirmState {
                action: EditorConfirmAction::ExitEditor,
                ..
            })
        ));
    }

    #[test]
    fn invalid_save_keeps_editor_open_and_surfaces_error() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().draft.rules[0].mode = "broken".to_string();

        handle_app_action(&mut app, |app| app.save_editor()).unwrap();

        assert!(app.editor.is_some());
        assert!(app.message.starts_with("Error:"));
        assert!(app.message.contains("uses invalid mode"));
    }

    #[test]
    fn esc_closes_text_input_overlay() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.open_selected_editor_field();

        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::TextInput(_)
        ));

        app.handle_editor_key(KeyCode::Esc).unwrap();

        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::None
        ));
    }

    #[test]
    fn esc_from_nested_string_list_returns_to_rule_editor() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().selected_field = EditorField::Rules;
        app.open_selected_editor_field();
        app.handle_editor_key(KeyCode::Enter).unwrap();

        match &mut app.editor.as_mut().unwrap().overlay {
            EditorOverlay::RuleEditor(state) => state.selected_field = RuleField::Destinations,
            other => panic!("expected rule editor, got {other:?}"),
        }

        app.handle_editor_key(KeyCode::Enter).unwrap();
        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::StringList(_)
        ));

        app.handle_editor_key(KeyCode::Esc).unwrap();

        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::RuleEditor(_)
        ));
    }

    #[test]
    fn enter_from_nested_text_input_returns_to_parent_list() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().selected_field = EditorField::Requires;
        app.open_selected_editor_field();
        app.handle_editor_key(KeyCode::Char('a')).unwrap();

        match &mut app.editor.as_mut().unwrap().overlay {
            EditorOverlay::TextInput(state) => {
                state.value = "agent".to_string();
                state.cursor = state.value.len();
            }
            other => panic!("expected text input, got {other:?}"),
        }

        app.handle_editor_key(KeyCode::Enter).unwrap();

        assert!(matches!(
            app.editor.as_ref().unwrap().overlay,
            EditorOverlay::StringList(_)
        ));
    }

    #[test]
    fn space_toggles_rule_enabled_in_rule_editor() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().selected_field = EditorField::Rules;
        app.open_selected_editor_field();
        app.handle_editor_key(KeyCode::Enter).unwrap();

        let initial = app
            .editor
            .as_ref()
            .and_then(|editor| match &editor.overlay {
                EditorOverlay::RuleEditor(state) => editor.draft.rules.get(state.rule_index),
                _ => None,
            })
            .map(|rule| rule.enabled)
            .unwrap();

        match &mut app.editor.as_mut().unwrap().overlay {
            EditorOverlay::RuleEditor(state) => state.selected_field = RuleField::Enabled,
            other => panic!("expected rule editor, got {other:?}"),
        }

        app.handle_editor_key(KeyCode::Char(' ')).unwrap();

        let toggled = app
            .editor
            .as_ref()
            .and_then(|editor| match &editor.overlay {
                EditorOverlay::RuleEditor(state) => editor.draft.rules.get(state.rule_index),
                _ => None,
            })
            .map(|rule| rule.enabled)
            .unwrap();

        assert_ne!(initial, toggled);
    }

    #[test]
    fn space_toggles_rule_enabled_in_rule_list() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().selected_field = EditorField::Rules;
        app.open_selected_editor_field();

        let initial = app
            .editor
            .as_ref()
            .and_then(|editor| match &editor.overlay {
                EditorOverlay::RuleList(state) => editor.draft.rules.get(state.selected),
                _ => None,
            });
        let initial = initial.map(|rule| rule.enabled).unwrap();

        app.handle_editor_key(KeyCode::Char(' ')).unwrap();

        let toggled = app
            .editor
            .as_ref()
            .and_then(|editor| match &editor.overlay {
                EditorOverlay::RuleList(state) => editor.draft.rules.get(state.selected),
                _ => None,
            });
        let toggled = toggled.map(|rule| rule.enabled).unwrap();

        assert_ne!(initial, toggled);
    }

    #[test]
    fn tab_cycles_path_completion_candidates() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        fs::create_dir_all(harness.temp.path().join("alpha-dir")).unwrap();
        fs::create_dir_all(harness.temp.path().join("alpha-file")).unwrap();

        app.begin_edit_selected().unwrap();
        app.editor.as_mut().unwrap().selected_field = EditorField::SourceRoot;
        app.open_selected_editor_field();

        match &mut app.editor.as_mut().unwrap().overlay {
            EditorOverlay::TextInput(state) => {
                state.value = "alp".to_string();
                state.cursor = state.value.len();
            }
            other => panic!("expected text input, got {other:?}"),
        }

        app.handle_editor_key(KeyCode::Tab).unwrap();
        let first = match &app.editor.as_ref().unwrap().overlay {
            EditorOverlay::TextInput(state) => state.value.clone(),
            other => panic!("expected text input, got {other:?}"),
        };

        app.handle_editor_key(KeyCode::Tab).unwrap();
        let second = match &app.editor.as_ref().unwrap().overlay {
            EditorOverlay::TextInput(state) => state.value.clone(),
            other => panic!("expected text input, got {other:?}"),
        };

        assert_ne!(first, second);
        assert!(first.starts_with("alpha"));
        assert!(second.starts_with("alpha"));
    }

    #[test]
    fn delete_profile_confirmation_removes_profile_after_confirm() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
        app.select_next();
        app.select_next();

        app.prompt_delete_selected();
        app.handle_confirm_key(
            EditorConfirmState {
                title: "Delete Profile".to_string(),
                message: String::new(),
                action: EditorConfirmAction::DeleteProfile {
                    name: "secondary".to_string(),
                },
            },
            KeyCode::Enter,
        )
        .unwrap();

        assert!(!app.config_doc.config.profiles.contains_key("secondary"));
        assert_ne!(app.selected_profile_name(), Some("secondary"));
        assert!(app.message.contains("Deleted profile 'secondary'"));
    }

    fn buffer_to_string(buffer: &Buffer) -> String {
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn text_to_plain_string(text: &Text<'_>) -> String {
        text.lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    struct Harness {
        temp: TempDir,
    }

    impl Harness {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let source_root = temp.path().join("source");
            let profile_root = temp.path().join("profile-source");
            let dest_root = temp.path().join("dest");

            fs::create_dir_all(source_root.join("Skills/alpha")).unwrap();
            fs::create_dir_all(source_root.join("Agents")).unwrap();
            fs::create_dir_all(source_root.join("Notes")).unwrap();
            fs::write(source_root.join("Skills/alpha/SKILL.md"), "# alpha").unwrap();
            fs::write(source_root.join("Agents/assistant.md"), "assistant").unwrap();
            fs::write(source_root.join("Notes/notes.md"), "notes").unwrap();
            fs::write(source_root.join("USER.md"), "user").unwrap();
            fs::create_dir_all(profile_root.join("Skills/secondary")).unwrap();
            fs::write(
                profile_root.join("Skills/secondary/SKILL.md"),
                "# secondary",
            )
            .unwrap();
            fs::create_dir_all(&dest_root).unwrap();

            let config = format!(
                "version: 1\nsource_root: {}\n\ncompositions:\n  agent:\n    output: build/prompts/AGENTS.generated.md\n    variables:\n      host: codex\n    inputs:\n      - path: Agents/assistant.md\n        wrapper:\n          before: \"<assistant path=\\\"{{path}}\\\">\\n\"\n          after: \"\\n</assistant>\\n\"\n      - path: USER.md\n        wrapper:\n          before: \"<user path=\\\"{{path}}\\\">\\n\"\n          after: \"\\n</user>\\n\"\n    renderer:\n      kind: concat\n      outer_wrapper:\n        before: \"<prompt host=\\\"{{host}}\\\">\\n\"\n        after: \"\\n</prompt>\\n\"\n\nprofiles:\n  primary:\n    rules:\n      - select: Skills/*\n        to:\n          - {}/skills/\n        mode: symlink\n      - select: Notes/notes.md\n        to:\n          - {}/manual/notes.md\n        mode: symlink\n  prompted:\n    requires:\n      - agent\n    rules:\n      - select: build/prompts/AGENTS.generated.md\n        to:\n          - {}/AGENTS.md\n        mode: symlink\n  secondary:\n    source_root: {}\n    rules:\n      - select: Skills/*\n        to:\n          - {}/secondary/\n        mode: symlink\n",
                source_root.display(),
                dest_root.display(),
                dest_root.display(),
                dest_root.display(),
                profile_root.display(),
                dest_root.display()
            );
            fs::write(temp.path().join("config.yaml"), config).unwrap();

            Self { temp }
        }

        fn config_doc(&self) -> EditableConfigDocument {
            crate::config::load_editable_config(&self.temp.path().join("config.yaml")).unwrap()
        }

        fn store(&self) -> StateStore {
            StateStore::new(Some(self.temp.path().join("state"))).unwrap()
        }

        fn dest_root(&self) -> std::path::PathBuf {
            self.temp.path().join("dest")
        }
    }
}
