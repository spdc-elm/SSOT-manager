use std::fs;

use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect, text::Text};
use tempfile::TempDir;

use super::input::handle_app_action;
use super::render::{
    draw_ui, render_detail, render_plan_text, render_profile_list, render_rule_list_overlay,
    render_show_text, render_string_list_overlay,
};
use super::*;

use crate::config::{EditableConfigDocument, EditableRule, MaterializationMode};
use crate::state::StateStore;

#[test]
fn render_shows_profiles_and_detail_content() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));

    draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &mut app);

    let rendered = buffer_to_string(&buffer);
    assert!(rendered.contains("Profiles"));
    assert!(rendered.contains("primary"));
    assert!(rendered.contains("Profile 'primary'"));
    assert!(rendered.contains("Status: Ready"));
    assert!(rendered.contains("a apply"));
    assert!(rendered.contains("c compile deps"));
}

#[test]
fn render_profile_list_keeps_selected_profile_visible_in_small_viewport() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.profiles = (1..=8).map(|index| format!("profile-{index}")).collect();
    app.selected_profile = 6;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 6));

    render_profile_list(Rect::new(0, 0, 24, 6), &mut buffer, &app);

    let rendered = buffer_to_string(&buffer);
    assert!(rendered.contains("> profile-7"));
    assert!(!rendered.contains("profile-1"));
}

#[test]
fn render_detail_scrolls_to_later_rules() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let profile = app.config.profiles.get_mut("primary").unwrap();
    for index in 0..12 {
        profile.rules.push(crate::config::Rule {
            select: format!("Extra/{index}.md"),
            to: vec![format!(
                "{}/extra-{index}.md",
                harness.dest_root().display()
            )],
            mode: MaterializationMode::Symlink,
            ignore: Vec::new(),
            enabled: true,
            tags: Vec::new(),
            note: None,
        });
    }
    app.scroll_detail_to_end();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 18));

    render_detail(Rect::new(0, 0, 80, 18), &mut buffer, &mut app);

    let rendered = buffer_to_string(&buffer);
    assert!(rendered.contains("rule 11 [symlink] enabled"), "{rendered}");
    assert!(!rendered.contains("Skills/*"), "{rendered}");
}

#[test]
fn enter_and_escape_toggle_detail_focus_without_losing_context() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.active_view = DetailView::Plan;
    let selected = app.selected_profile_name().unwrap().to_string();

    app.handle_key(KeyCode::Enter).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::InspectDetail);
    assert_eq!(app.selected_profile_name(), Some(selected.as_str()));
    assert_eq!(app.active_view, DetailView::Plan);

    app.handle_key(KeyCode::Esc).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::BrowseProfiles);
    assert_eq!(app.selected_profile_name(), Some(selected.as_str()));
    assert_eq!(app.active_view, DetailView::Plan);
}

#[test]
fn h_returns_to_browse_mode_from_detail_focus() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.active_view = DetailView::Show;

    app.handle_key(KeyCode::Enter).unwrap();
    app.handle_key(KeyCode::Char('h')).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::BrowseProfiles);
    assert_eq!(app.active_view, DetailView::Show);
}

#[test]
fn h_moves_to_previous_view_inside_detail_focus_before_exiting() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.active_view = DetailView::Doctor;

    app.handle_key(KeyCode::Enter).unwrap();
    app.handle_key(KeyCode::Char('h')).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::InspectDetail);
    assert_eq!(app.active_view, DetailView::Plan);

    app.handle_key(KeyCode::Char('h')).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::InspectDetail);
    assert_eq!(app.active_view, DetailView::Show);
}

#[test]
fn vertical_navigation_keys_switch_between_profile_movement_and_detail_scrolling() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let profile = app.config.profiles.get_mut("primary").unwrap();
    for index in 0..12 {
        profile.rules.push(crate::config::Rule {
            select: format!("Extra/{index}.md"),
            to: vec![format!(
                "{}/extra-{index}.md",
                harness.dest_root().display()
            )],
            mode: MaterializationMode::Symlink,
            ignore: Vec::new(),
            enabled: true,
            tags: Vec::new(),
            note: None,
        });
    }

    app.handle_key(KeyCode::Down).unwrap();
    assert_eq!(app.selected_profile_name(), Some("prompted"));

    app.selected_profile = 0;
    app.handle_key(KeyCode::Enter).unwrap();
    app.handle_key(KeyCode::Down).unwrap();

    assert_eq!(app.selected_profile_name(), Some("primary"));
    assert!(app.detail_scroll > 0);
}

#[test]
fn page_navigation_keys_still_scroll_detail_in_browse_mode() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let profile = app.config.profiles.get_mut("primary").unwrap();
    for index in 0..12 {
        profile.rules.push(crate::config::Rule {
            select: format!("Extra/{index}.md"),
            to: vec![format!(
                "{}/extra-{index}.md",
                harness.dest_root().display()
            )],
            mode: MaterializationMode::Symlink,
            ignore: Vec::new(),
            enabled: true,
            tags: Vec::new(),
            note: None,
        });
    }

    app.handle_key(KeyCode::PageDown).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::BrowseProfiles);
    assert!(app.detail_scroll > 0);
}

#[test]
fn view_switch_resets_detail_scroll_but_keeps_detail_focus() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let profile = app.config.profiles.get_mut("primary").unwrap();
    for index in 0..12 {
        profile.rules.push(crate::config::Rule {
            select: format!("Extra/{index}.md"),
            to: vec![format!(
                "{}/extra-{index}.md",
                harness.dest_root().display()
            )],
            mode: MaterializationMode::Symlink,
            ignore: Vec::new(),
            enabled: true,
            tags: Vec::new(),
            note: None,
        });
    }

    app.handle_key(KeyCode::Enter).unwrap();
    app.handle_key(KeyCode::PageDown).unwrap();
    assert!(app.detail_scroll > 0);

    app.handle_key(KeyCode::Tab).unwrap();

    assert_eq!(app.shell_focus, ShellFocus::InspectDetail);
    assert_eq!(app.detail_scroll, 0);
}

#[test]
fn render_detail_shows_overflow_indicator_only_when_needed() {
    let harness = Harness::new();
    let mut long_app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let profile = long_app.config.profiles.get_mut("primary").unwrap();
    for index in 0..12 {
        profile.rules.push(crate::config::Rule {
            select: format!("Extra/{index}.md"),
            to: vec![format!(
                "{}/extra-{index}.md",
                harness.dest_root().display()
            )],
            mode: MaterializationMode::Symlink,
            ignore: Vec::new(),
            enabled: true,
            tags: Vec::new(),
            note: None,
        });
    }
    long_app.handle_key(KeyCode::Enter).unwrap();
    let mut long_buffer = Buffer::empty(Rect::new(0, 0, 80, 18));

    render_detail(Rect::new(0, 0, 80, 18), &mut long_buffer, &mut long_app);

    let long_rendered = buffer_to_string(&long_buffer);
    assert!(long_rendered.contains("Detail [Inspect] 1-13/"));
    assert!(long_rendered.contains("#"));
    assert!(long_rendered.contains(":"));

    let mut short_app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    let mut short_buffer = Buffer::empty(Rect::new(0, 0, 80, 18));

    render_detail(Rect::new(0, 0, 80, 18), &mut short_buffer, &mut short_app);

    let short_rendered = buffer_to_string(&short_buffer);
    assert!(short_rendered.contains("Detail [Preview]"));
    assert!(!short_rendered.contains("Detail [Preview] 1-"));
    assert!(!short_rendered.contains("#"));
}

#[test]
fn detail_cache_is_reused_until_state_changes() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

    let first = text_to_plain_string(&app.detail_text().unwrap());
    assert!(app.detail_cache.is_some());
    let cached_revision = app.detail_revision;

    let second = text_to_plain_string(&app.detail_text().unwrap());
    assert_eq!(first, second);
    assert_eq!(app.detail_revision, cached_revision);

    app.apply_selected().unwrap();
    assert!(app.detail_cache.is_none());
    assert!(app.detail_revision > cached_revision);
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

    draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &mut app);

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
fn plan_view_shows_source_paths_relative_to_source_root() {
    let harness = Harness::new();
    let app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();

    let rendered = text_to_plain_string(&render_plan_text(app.plan_view().unwrap().unwrap()));
    let source_root = harness.temp.path().join("source").display().to_string();

    assert!(rendered.contains(&format!("source_root: {source_root}")));
    assert!(rendered.contains("./Skills/alpha"));
    assert_eq!(rendered.matches(&source_root).count(), 1);
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
fn render_rule_list_keeps_selected_rule_visible_in_small_viewport() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.begin_edit_selected().unwrap();
    {
        let editor = app.editor.as_mut().unwrap();
        editor.draft.rules = (1..=10)
            .map(|index| EditableRule {
                select: format!("rule-{index}"),
                to: vec![format!("/tmp/rule-{index}")],
                mode: MaterializationMode::Symlink.as_str().to_string(),
                ignore: Vec::new(),
                enabled: true,
                tags: Vec::new(),
                note: None,
            })
            .collect();
        editor.overlay = EditorOverlay::RuleList(RuleListEditorState { selected: 8 });
    }
    let editor = app.editor.as_ref().unwrap();
    let state = match &editor.overlay {
        EditorOverlay::RuleList(state) => state,
        other => panic!("expected rule list, got {other:?}"),
    };
    let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 10));

    render_rule_list_overlay(Rect::new(0, 0, 50, 10), &mut buffer, editor, state);

    let rendered = buffer_to_string(&buffer);
    assert!(rendered.contains("> rule-9"));
    assert!(!rendered.contains("rule-1"));
    assert!(rendered.contains("#"));
    assert!(rendered.contains(":"));
}

#[test]
fn render_string_list_overlay_shows_scrollbar_when_entries_overflow() {
    let harness = Harness::new();
    let mut app = TuiApp::new(harness.config_doc(), harness.store()).unwrap();
    app.begin_edit_selected().unwrap();
    {
        let editor = app.editor.as_mut().unwrap();
        editor.draft.requires = (1..=16).map(|index| format!("req-{index}")).collect();
        editor.overlay = EditorOverlay::StringList(StringListEditorState::new(
            "Profile Requires",
            StringListKind::Requires,
            14,
            EditorOverlay::None,
        ));
    }
    let editor = app.editor.as_ref().unwrap();
    let state = match &editor.overlay {
        EditorOverlay::StringList(state) => state,
        other => panic!("expected string list, got {other:?}"),
    };
    let mut buffer = Buffer::empty(Rect::new(0, 0, 50, 14));

    render_string_list_overlay(Rect::new(0, 0, 50, 14), &mut buffer, editor, state);

    let rendered = buffer_to_string(&buffer);
    assert!(rendered.contains("> req-15"));
    assert!(rendered.contains("#"));
    assert!(rendered.contains(":"));
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
