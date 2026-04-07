use std::path::PathBuf;

use anyhow::Result;
use ratatui::text::Text;

use crate::config::{
    Config, EditableConfigDocument, EditableProfile, EditableRule, MaterializationMode,
    load_editable_config, validate_and_write_editable_config, validate_editable_config,
};
use crate::inspection::{
    ProfileExplainView, ProfileShowView, explain_profile, list_profiles, show_profile,
};
use crate::prompt::build_profile_requirements;
use crate::reconcile::{
    apply_plan, apply_plan_force_with_backup, build_plan, can_force_with_backup, doctor_profile,
    undo_last_apply,
};
use crate::state::{ManagedState, StateStore};

use self::render::{render_doctor_text, render_plan_text, render_show_text};

#[path = "input.rs"]
pub(super) mod input;
#[path = "render.rs"]
pub(super) mod render;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailView {
    Show,
    Plan,
    Doctor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellFocus {
    BrowseProfiles,
    InspectDetail,
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

pub(super) struct TuiApp {
    config_doc: EditableConfigDocument,
    config: Config,
    store: StateStore,
    state: ManagedState,
    profiles: Vec<String>,
    selected_profile: usize,
    shell_focus: ShellFocus,
    active_view: DetailView,
    detail_scroll: u16,
    message: String,
    force_apply_armed_for: Option<String>,
    editor: Option<ProfileEditorState>,
}

impl TuiApp {
    pub(super) fn new(config_doc: EditableConfigDocument, store: StateStore) -> Result<Self> {
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
            shell_focus: ShellFocus::BrowseProfiles,
            active_view: DetailView::Show,
            detail_scroll: 0,
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

    fn detail_focused(&self) -> bool {
        self.shell_focus == ShellFocus::InspectDetail
    }

    fn reset_detail_scroll(&mut self) {
        self.detail_scroll = 0;
    }

    fn clamp_detail_scroll(&mut self) {
        self.detail_scroll = self
            .detail_scroll
            .min(self.detail_line_count().saturating_sub(1) as u16);
    }

    fn enter_detail_focus(&mut self) {
        if self.selected_profile_name().is_none() {
            self.message = "No profile selected".to_string();
            self.shell_focus = ShellFocus::BrowseProfiles;
            return;
        }
        self.shell_focus = ShellFocus::InspectDetail;
        self.clamp_detail_scroll();
    }

    fn exit_detail_focus(&mut self) {
        self.shell_focus = ShellFocus::BrowseProfiles;
        self.clamp_detail_scroll();
    }

    fn reload_profiles(&mut self, selected_name: Option<&str>) {
        self.profiles = list_profiles(&self.config)
            .profiles
            .into_iter()
            .map(|profile| profile.name)
            .collect();

        if self.profiles.is_empty() {
            self.selected_profile = 0;
            self.shell_focus = ShellFocus::BrowseProfiles;
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
            self.reset_detail_scroll();
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
            self.reset_detail_scroll();
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
        self.reset_detail_scroll();
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

    fn detail_text(&self) -> Result<Text<'static>> {
        match self.active_view {
            DetailView::Show => self
                .show_view()
                .map(|view| {
                    view.map(render_show_text)
                        .unwrap_or_else(|| Text::from("No profile selected"))
                })
                .map_err(Into::into),
            DetailView::Plan => self
                .plan_view()
                .map(|view| {
                    view.map(render_plan_text)
                        .unwrap_or_else(|| Text::from("No profile selected"))
                })
                .map_err(Into::into),
            DetailView::Doctor => self
                .doctor_view()
                .map(|view| {
                    render_doctor_text(view.unwrap_or_else(|| "No profile selected".to_string()))
                })
                .map_err(Into::into),
        }
    }

    fn detail_line_count(&self) -> usize {
        self.detail_text().map(|text| text.lines.len()).unwrap_or(1)
    }

    fn scroll_detail_by(&mut self, delta: i16) {
        self.clamp_detail_scroll();
        let max_scroll = self.detail_line_count().saturating_sub(1) as u16;
        let next = if delta.is_negative() {
            self.detail_scroll.saturating_sub(delta.unsigned_abs())
        } else {
            self.detail_scroll
                .saturating_add(delta as u16)
                .min(max_scroll)
        };
        self.detail_scroll = next;
    }

    fn scroll_detail_to_end(&mut self) {
        self.detail_scroll = self.detail_line_count().saturating_sub(1) as u16;
        self.clamp_detail_scroll();
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
