use crate::tika_document::TikaDocument;
use crate::util::event::{Event, Events};
use crate::xapian_utils;
use color_eyre::Report;
use std::io::{stdout, Write};
use termion::{event::Key, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

// Needed to provide `width()` method on String:
// no method named `width` found for struct `std::string::String` in the current scope
use unicode_width::UnicodeWidthStr;

/// TerminalApp holds the state of the application
pub(crate) struct TerminalApp {
    /// Current value of the input box
    pub(crate) input: String,
    /// Preview window
    pub(crate) output: String,
    /// Query Matches
    pub(crate) matches: Vec<TikaDocument>,
    /// Keep track of which matches are selected
    pub(crate) state: ListState,
    /// Report query parsing errors back to the user
    pub(crate) errout: String,
    /// Display the parsed query for debugging purposes
    pub(crate) query: String,
}

impl TerminalApp {
    pub fn get_selected(&mut self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        if let Some(i) = self.state.selected() {
            if let Some(s) = self.matches[i].full_path.to_str() {
                ret.push(s.into());
            }
        };
        ret
    }

    pub fn get_selected_contents(&mut self) -> String {
        if let Some(i) = self.state.selected() {
            return self.matches[i].body.clone();
        };
        String::from("")
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.matches.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

impl Default for TerminalApp {
    fn default() -> TerminalApp {
        TerminalApp {
            input: String::new(),
            output: String::new(),
            matches: Vec::new(),
            state: ListState::default(),
            errout: String::new(),
            query: String::new(),
        }
    }
}

pub fn setup_panic() {
    std::panic::set_hook(Box::new(move |x| {
        stdout()
            .into_raw_mode()
            .unwrap()
            .suspend_raw_mode()
            .unwrap();
        write!(
            stdout().into_raw_mode().unwrap(),
            "{}",
            termion::screen::ToMainScreen
        )
        .unwrap();
        write!(stdout(), "{:?}", x).unwrap();
    }));
}

/// Interactive query interface
pub fn interactive_query() -> Result<Vec<String>, Report> {
    // TODO create DB in main and pass it through to query_db
    let mut tui = tui::Terminal::new(TermionBackend::new(AlternateScreen::from(
        stdout().into_raw_mode().unwrap(),
    )))
    .unwrap();

    // Setup event handlers
    let events = Events::new();

    // Create default app state
    let mut app = TerminalApp::default();

    loop {
        // Draw UI
        tui.draw(|f| {
            let panes = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Min(1),
                        Constraint::Length(2),
                        Constraint::Length(2),
                        Constraint::Length(2),
                    ]
                    .as_ref(),
                )
                .split(f.size());
            let selected_style = Style::default().add_modifier(Modifier::REVERSED);

            let content = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(panes[0]);

            // Output area where match titles are displayed
            let matches: Vec<ListItem> = app
                .matches
                .iter()
                .map(|m| {
                    let content = vec![Spans::from(Span::raw(format!("{}", m.title)))];
                    ListItem::new(content)
                })
                .collect();
            let matches = List::new(matches)
                .block(Block::default().borders(Borders::LEFT))
                .highlight_style(selected_style)
                .highlight_symbol("> ");
            f.render_stateful_widget(matches, content[0], &mut app.state);

            // Preview area where content is displayed
            let paragraph = Paragraph::new(app.output.as_ref())
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(paragraph, content[1]);

            // Input area where queries are entered
            let input = Paragraph::new(app.input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(input, panes[1]);

            // Make the cursor visible and ask tui-rs to put it at the specified
            // coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                panes[1].x + app.input.width() as u16,
                panes[1].y,
            );

            // Area to display the parsed Xapian::Query.get_description()
            let query = Paragraph::new(app.query.as_ref())
                .style(Style::default().fg(Color::Green))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(query, panes[2]);

            // Area where errors are displayed, query parsing errors, etc
            let errout = Paragraph::new(app.errout.as_ref())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().borders(Borders::NONE));
            f.render_widget(errout, panes[3]);
        })?;

        // Handle input
        if let Event::Input(input) = events.next()? {
            match input {
                Key::Char('\n') => {
                    // Select choice
                    break;
                }
                Key::Ctrl('c') => {
                    break;
                }
                Key::Char(c) => {
                    app.input.push(c);
                }
                Key::Backspace => {
                    app.input.pop();
                }
                Key::Down | Key::Ctrl('n') => {
                    app.next();
                    app.output = app.get_selected_contents();
                }
                Key::Up | Key::Ctrl('p') => {
                    app.previous();
                    app.output = app.get_selected_contents();
                }
                _ => {}
            }

            let mut inp: String = app.input.to_owned();
            // Add a trailing ` ;` to the query to hint to Nom that it has a "full" string
            inp.push_str(&" ;");

            match xapian_utils::parse_user_query(&inp) {
                Ok(mut query) => {
                    app.query = query.get_description();
                    app.matches = xapian_utils::query_db(query)?;
                }
                Err(e) => {
                    app.errout = e.to_string();
                }
            };
        }
    }

    tui.clear().unwrap();

    Ok(app.get_selected())
}
