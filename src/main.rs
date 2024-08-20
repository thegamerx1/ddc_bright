use std::{
    error::Error,
    io,
    process::exit,
    sync::{Arc, Mutex},
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use display::{Controller, DisplayManager, MyDisplay};
use ratatui::{prelude::*, widgets::*};

mod display;

enum InputMode {
    Select,
    Help,
    Selected(Arc<MyDisplay>),
}

/// App holds the state of the application
struct App {
    input_mode: InputMode,
    manager: DisplayManager,
    step_size: i16,
    loading: bool,
    show_help: bool,

    control_index: usize,
    control_selected: Option<Arc<Mutex<Controller>>>,
    control_widget_state: ListState,

    display_index: usize,
    display_selected: Option<Arc<MyDisplay>>,
    display_widget_state: ListState,
}

impl Default for App {
    fn default() -> App {
        let manager = DisplayManager::new();
        App {
            input_mode: InputMode::Select,
            step_size: 1,
            manager,
            control_index: 0,
            display_index: 0,
            control_selected: None,
            display_selected: None,
            loading: false,
            show_help: false,
            display_widget_state: ListState::default().with_selected(None).with_offset(0),
            control_widget_state: ListState::default().with_selected(None).with_offset(0),
        }
    }
}

impl App {
    fn select_display(&mut self) {
        if let Some(display) = self.manager.displays.get(self.display_index) {
            self.display_selected = Some(Arc::clone(display));
            self.select_control(0);
            self.input_mode =
                InputMode::Selected(Arc::clone(self.display_selected.as_ref().unwrap()));
        }
    }
    fn set_display(&mut self, desired: usize) {
        let desired = if desired >= self.manager.displays.len() {
            0
        } else {
            desired
        };
        self.display_index = desired;
        self.display_widget_state.select(Some(desired));
    }

    fn select_control(&mut self, mut desired: usize) {
        let display = self.display_selected.as_mut().unwrap();
        let length = display.controls.len();
        if desired >= length {
            desired = 0;
        }
        let control = Arc::clone(display.controls.get(desired).unwrap());
        self.control_index = desired;
        self.control_selected = Some(control);
        self.control_widget_state.select(Some(desired));
    }

    fn next_control(&mut self) {
        self.select_control(self.control_index.saturating_add(1));
    }

    fn prev_control(&mut self) {
        self.select_control(self.control_index.saturating_sub(1));
    }

    fn add_to_control(&mut self, value: i16) {
        if let Some(control_mutex) = &mut self.control_selected {
            let mut control = control_mutex.lock().unwrap();
            let value = std::cmp::max(std::cmp::min(control.value as i16 + value, 100), 0) as u16;
            control.set(value).unwrap()
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::default();
    println!("Loading monitors..");
    app.manager.refresh()?;

    if app.manager.displays.len() == 0 {
        println!("No displays!");
        exit(1);
    }

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;

    // create app and run it
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char(char) = key.code {
                if let Some(char) = char.to_digit(10) {
                    app.set_display((char as usize).saturating_sub(1));
                    app.select_display();
                }
            };
            match app.input_mode {
                InputMode::Help => match key.code {
                    KeyCode::Char(_) | KeyCode::Esc => {
                        app.show_help = false;
                        app.input_mode = InputMode::Select;
                    }
                    _ => (),
                },
                InputMode::Select if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('r') => {
                        app.manager.refresh().unwrap();
                    }
                    KeyCode::Char('?') => {
                        app.input_mode = InputMode::Help;
                        app.show_help = true;
                    }
                    KeyCode::Char('w') => app.set_display(app.display_index.saturating_sub(1)),
                    KeyCode::Char('s') => app.set_display(app.display_index.saturating_add(1)),
                    KeyCode::Enter | KeyCode::Char(' ') => app.select_display(),
                    _ => {}
                },
                InputMode::Selected(_) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Up | KeyCode::Char('w') => app.prev_control(),
                    KeyCode::Down | KeyCode::Char('s') => app.next_control(),
                    KeyCode::Left | KeyCode::Char('a') => app.add_to_control(-app.step_size),
                    KeyCode::Right | KeyCode::Char('d') => app.add_to_control(app.step_size),
                    KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                        app.input_mode = InputMode::Select;
                        app.display_selected = None;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    // let offset = app.vertical_display_state.offset_mut();
    // *offset = app.scroll_display;

    let display_widget: Vec<ListItem> = app
        .manager
        .displays
        .iter()
        .enumerate()
        .map(|(i, display)| {
            let content = Line::from(Span::raw(format!("{0}: {1}", i + 1, display.name)));
            ListItem::new(content)
        })
        .collect();

    let mut display_block = Block::default().borders(Borders::ALL).title("Displays");
    if app.display_selected.is_none() {
        display_block = display_block.border_style(Style::default().fg(Color::Blue))
    }
    let display_widget = List::new(display_widget)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("> ")
        .block(display_block);

    f.render_stateful_widget(display_widget, chunks[0], &mut app.display_widget_state);

    if let Some(display) = &app.display_selected {
        let control_widget: Vec<ListItem> = display
            .controls
            .iter()
            .map(|controller_mutex| {
                let controller = controller_mutex.lock().unwrap();
                let content = Line::from(Span::raw(format!(
                    "{0}: {1}",
                    controller.control.name, controller.value
                )));
                ListItem::new(content)
            })
            .collect();
        let control_widget = List::new(control_widget)
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol("> ")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue))
                    .title(format!("Controls - {}", display.name)),
            );
        f.render_stateful_widget(control_widget, chunks[1], &mut app.control_widget_state);
    }

    if app.show_help {
        let msg = vec![
            Line::from(vec!["q".bold(), " exit".into()]),
            Line::from(vec!["r".bold(), " reload".into()]),
            Line::from(vec!["1-9".bold(), " select monitor".into()]),
        ];
        let text = Text::from(msg);

        let area = centered_rect(100, 100, f.size());
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        let help_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)].as_ref())
            .margin(1)
            .split(area);

        let block = Block::default().title("Help").borders(Borders::ALL);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        f.render_widget(paragraph, help_chunks[0]);
    }
}
