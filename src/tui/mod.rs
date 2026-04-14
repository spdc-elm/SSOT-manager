use std::io::{self, Stdout};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::config::EditableConfigDocument;
use crate::state::StateStore;

mod state;

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run_tui(config_doc: EditableConfigDocument, store: StateStore) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = state::TuiApp::new(config_doc, store)?;
    let result = run_event_loop(&mut terminal, &mut app);
    restore_terminal(terminal)?;
    result
}

fn run_event_loop(terminal: &mut AppTerminal, app: &mut state::TuiApp) -> Result<()> {
    terminal.draw(|frame| state::render::draw_ui(frame.area(), frame.buffer_mut(), app))?;

    loop {
        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if app.handle_key(key.code)? {
                    break;
                }
                terminal
                    .draw(|frame| state::render::draw_ui(frame.area(), frame.buffer_mut(), app))?;
            }
            Event::Resize(_, _) => {
                terminal
                    .draw(|frame| state::render::draw_ui(frame.area(), frame.buffer_mut(), app))?;
            }
            _ => {}
        }
    }

    Ok(())
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
