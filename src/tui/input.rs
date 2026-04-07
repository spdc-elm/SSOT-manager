use std::fs;

use anyhow::Result;
use crossterm::event::KeyCode;

use crate::paths::resolve_input_path;

use super::*;

impl TuiApp {
    pub(super) fn handle_editor_key(&mut self, key: KeyCode) -> Result<bool> {
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

    pub(super) fn handle_confirm_key(
        &mut self,
        state: EditorConfirmState,
        key: KeyCode,
    ) -> Result<()> {
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
}

impl TuiApp {
    pub(crate) fn handle_key(&mut self, key: KeyCode) -> Result<bool> {
        if self.editing() {
            return self.handle_editor_key(key);
        }

        match key {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Enter => {
                self.enter_detail_focus();
                if let Some(profile_name) = self.selected_profile_name() {
                    self.message = format!("Inspecting '{}' detail", profile_name);
                }
            }
            KeyCode::Esc => {
                if self.detail_focused() {
                    self.exit_detail_focus();
                    self.message = "Browsing profiles".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail_focused() {
                    self.scroll_detail_by(1);
                } else {
                    self.select_next();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail_focused() {
                    self.scroll_detail_by(-1);
                } else {
                    self.select_previous();
                }
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                self.active_view = self.active_view.next();
                self.reset_detail_scroll();
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.active_view = self.active_view.previous();
                self.reset_detail_scroll();
            }
            KeyCode::Char('h') => {
                if self.detail_focused() && self.active_view == DetailView::Show {
                    self.exit_detail_focus();
                    self.message = "Browsing profiles".to_string();
                } else {
                    self.active_view = self.active_view.previous();
                    self.reset_detail_scroll();
                }
            }
            KeyCode::PageDown => self.scroll_detail_by(8),
            KeyCode::PageUp => self.scroll_detail_by(-8),
            KeyCode::Home => self.reset_detail_scroll(),
            KeyCode::End => self.scroll_detail_to_end(),
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

pub(super) fn handle_app_action(
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
