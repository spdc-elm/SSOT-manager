use std::io::{self, Stdout};

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
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap};

use crate::config::Config;
use crate::inspection::{
    ProfileExplainView, ProfileShowView, explain_profile, list_profiles, show_profile,
};
use crate::reconcile::{apply_plan, build_plan, doctor_profile, undo_last_apply};
use crate::state::{ManagedState, StateStore};

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetailView {
    Show,
    Plan,
    Doctor,
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
    config: Config,
    store: StateStore,
    state: ManagedState,
    profiles: Vec<String>,
    selected_profile: usize,
    active_view: DetailView,
    message: String,
}

impl TuiApp {
    pub fn new(config: Config, store: StateStore) -> Result<Self> {
        let state = store.load()?;
        let profiles = list_profiles(&config)
            .profiles
            .into_iter()
            .map(|profile| profile.name)
            .collect();

        Ok(Self {
            config,
            store,
            state,
            profiles,
            selected_profile: 0,
            active_view: DetailView::Show,
            message: "Ready".to_string(),
        })
    }

    fn selected_profile_name(&self) -> Option<&str> {
        self.profiles.get(self.selected_profile).map(String::as_str)
    }

    fn select_next(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_profile = (self.selected_profile + 1) % self.profiles.len();
        }
    }

    fn select_previous(&mut self) {
        if !self.profiles.is_empty() {
            self.selected_profile = if self.selected_profile == 0 {
                self.profiles.len() - 1
            } else {
                self.selected_profile - 1
            };
        }
    }

    fn refresh(&mut self) -> Result<()> {
        self.state = self.store.load()?;
        self.message = "Refreshed state".to_string();
        Ok(())
    }

    fn apply_selected(&mut self) -> Result<()> {
        let Some(profile_name) = self.selected_profile_name().map(str::to_string) else {
            self.message = "No profile selected".to_string();
            return Ok(());
        };
        self.state = self.store.load()?;
        let plan = build_plan(&self.config, &profile_name, &self.state)?;
        let result = apply_plan(plan, &self.state, &self.store)?;
        self.state = self.store.load()?;
        self.message = format!(
            "Applied '{}' with {} journal entries",
            profile_name,
            result.journal.entries.len()
        );
        Ok(())
    }

    fn undo(&mut self) -> Result<()> {
        let result = undo_last_apply(&self.store)?;
        self.state = self.store.load()?;
        self.message = format!(
            "Undid '{}' with {} reverted targets",
            result.profile_name,
            result.reverted_targets.len()
        );
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
}

pub fn run_tui(config: Config, store: StateStore) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = TuiApp::new(config, store)?;
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

            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Down | KeyCode::Char('j') => app.select_next(),
                KeyCode::Up | KeyCode::Char('k') => app.select_previous(),
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                    app.active_view = app.active_view.next()
                }
                KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                    app.active_view = app.active_view.previous()
                }
                KeyCode::Char('r') => handle_app_action(app, |app| app.refresh())?,
                KeyCode::Char('a') => handle_app_action(app, |app| app.apply_selected())?,
                KeyCode::Char('u') => handle_app_action(app, |app| app.undo())?,
                _ => {}
            }
        }
    }

    Ok(())
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

    let footer = Paragraph::new(Text::from(vec![
        Line::from(format!("Status: {}", app.message)),
        Line::from(
            "Keys: q quit | j/k or Up/Down move | Tab or h/l switch view | a apply | u undo | r refresh",
        ),
    ]))
        .block(Block::default().borders(Borders::ALL).title("Status / Keys"))
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
                    .unwrap_or_else(|| "No profile selected".to_string())
            })
            .unwrap_or_else(|error| format!("Error: {error:#}")),
        DetailView::Plan => app
            .plan_view()
            .map(|view| {
                view.map(render_plan_text)
                    .unwrap_or_else(|| "No profile selected".to_string())
            })
            .unwrap_or_else(|error| format!("Error: {error:#}")),
        DetailView::Doctor => app
            .doctor_view()
            .map(|view| view.unwrap_or_else(|| "No profile selected".to_string()))
            .unwrap_or_else(|error| format!("Error: {error:#}")),
    };

    let paragraph = Paragraph::new(Text::from(text))
        .block(Block::default().borders(Borders::ALL).title("Detail"))
        .wrap(Wrap { trim: false });
    paragraph.render(detail[1], buf);
}

fn render_show_text(view: ProfileShowView) -> String {
    let mut lines = vec![
        format!("Profile '{}'", view.profile_name),
        format!("source_root={}", view.source_root),
        format!(
            "rules={} enabled={} disabled={}",
            view.rule_count, view.enabled_rule_count, view.disabled_rule_count
        ),
    ];

    for rule in view.rules {
        lines.push(format!(
            "rule {} select={} mode={} enabled={}",
            rule.index, rule.select, rule.mode, rule.enabled
        ));
        for destination in rule.destinations {
            lines.push(format!("  to {destination}"));
        }
        if !rule.tags.is_empty() {
            lines.push(format!("  tags {}", rule.tags.join(",")));
        }
        if let Some(note) = rule.note {
            lines.push(format!("  note {note}"));
        }
    }

    lines.join("\n")
}

fn render_plan_text(view: ProfileExplainView) -> String {
    let mut lines = vec![
        format!("Explain '{}'", view.profile_name),
        format!("source_root={}", view.source_root),
    ];

    if view.diagnostics.is_empty() {
        lines.push("diagnostics: none".to_string());
    } else {
        lines.push("diagnostics:".to_string());
        for diagnostic in view.diagnostics {
            lines.push(format!("  [{}] {}", diagnostic.code, diagnostic.message));
        }
    }

    let summary = ["create", "update", "remove", "skip", "warning", "danger"]
        .into_iter()
        .map(|action| {
            format!(
                "{action}={}",
                view.plan_summary.get(action).copied().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(format!("plan summary: {summary}"));

    if view.plan_items.is_empty() {
        lines.push("plan items: none".to_string());
    } else {
        lines.push("plan items:".to_string());
        for item in view.plan_items {
            match item.desired_source {
                Some(source) => lines.push(format!(
                    "  {} {} -> {} ({})",
                    item.action, item.target, source, item.reason
                )),
                None => lines.push(format!(
                    "  {} {} ({})",
                    item.action, item.target, item.reason
                )),
            }
        }
    }

    lines.join("\n")
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
        let app = TuiApp::new(harness.config(), harness.store()).unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));

        draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &app);

        let rendered = buffer_to_string(&buffer);
        assert!(rendered.contains("Profiles"));
        assert!(rendered.contains("primary"));
        assert!(rendered.contains("Profile 'primary'"));
        assert!(rendered.contains("Status: Ready"));
        assert!(rendered.contains("a apply"));
    }

    #[test]
    fn apply_selected_updates_message_and_creates_symlink() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config(), harness.store()).unwrap();

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
        let mut app = TuiApp::new(harness.config(), harness.store()).unwrap();

        handle_app_action(&mut app, |app| app.apply_selected()).unwrap();

        assert!(app.message.contains("danger actions"));
    }

    #[test]
    fn undo_updates_message_after_apply() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config(), harness.store()).unwrap();
        app.apply_selected().unwrap();

        app.undo().unwrap();

        assert!(app.message.contains("Undid 'primary'"));
        assert!(!harness.dest_root().join("skills/alpha").exists());
    }

    #[test]
    fn render_surfaces_latest_status_message() {
        let harness = Harness::new();
        let mut app = TuiApp::new(harness.config(), harness.store()).unwrap();
        app.apply_selected().unwrap();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 40));

        draw_ui(Rect::new(0, 0, 120, 40), &mut buffer, &app);

        let rendered = buffer_to_string(&buffer);
        assert!(rendered.contains("Status: Applied 'primary'"));
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
            fs::create_dir_all(source_root.join("Notes")).unwrap();
            fs::write(source_root.join("Skills/alpha/SKILL.md"), "# alpha").unwrap();
            fs::write(source_root.join("Notes/notes.md"), "notes").unwrap();
            fs::create_dir_all(profile_root.join("Skills/secondary")).unwrap();
            fs::write(
                profile_root.join("Skills/secondary/SKILL.md"),
                "# secondary",
            )
            .unwrap();
            fs::create_dir_all(&dest_root).unwrap();

            let config = format!(
                "version: 1\nsource_root: {}\n\nprofiles:\n  primary:\n    rules:\n      - select: Skills/*\n        to:\n          - {}/skills/\n        mode: symlink\n      - select: Notes/notes.md\n        to:\n          - {}/manual/notes.md\n        mode: symlink\n  secondary:\n    source_root: {}\n    rules:\n      - select: Skills/*\n        to:\n          - {}/secondary/\n        mode: symlink\n",
                source_root.display(),
                dest_root.display(),
                dest_root.display(),
                profile_root.display(),
                dest_root.display()
            );
            fs::write(temp.path().join("config.yaml"), config).unwrap();

            Self { temp }
        }

        fn config(&self) -> Config {
            crate::config::load_config(&self.temp.path().join("config.yaml")).unwrap()
        }

        fn store(&self) -> StateStore {
            StateStore::new(Some(self.temp.path().join("state"))).unwrap()
        }

        fn dest_root(&self) -> std::path::PathBuf {
            self.temp.path().join("dest")
        }
    }
}
