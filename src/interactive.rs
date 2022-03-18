mod xapian_utils;
use crate::document;

use color_eyre::Report;
use eyre::bail;
use std::io::{stdout, Write};
use std::process::Command;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as hStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use tempfile::Builder;
use termion::{event::Key, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use xapian_rusty::Database;

/// TerminalApp holds the state of the application
pub(crate) struct TerminalApp {
    /// Current value of the query_input box
    pub(crate) query_input: String,
    /// Current value of the filter_input box
    pub(crate) filter_input: String,
    /// Preview window
    pub(crate) preview: String,
    /// Query Matches
    pub(crate) matches: Vec<document::Document>,
    /// Keep track of which matches are selected
    pub(crate) selected_state: ListState,
    /// Display error messages
    pub(crate) error: String,
    /// Display the serialized payload to send to the server
    pub(crate) debug: String,
    // TODO Add fields for sort expression
    inp_idx: usize,
    // Length here should stay in sync with the number of editable areas
    inp_widths: [i32; 2],
}

impl TerminalApp {
    // TODO make this work for multiple selections
    pub fn get_selected(&mut self) -> Vec<String> {
        let ret: Vec<String> = Vec::new();
        if let Some(i) = self.selected_state.selected() {
            vec![self.matches[i].id.to_owned()]
        } else {
            ret
        }
    }

    pub fn get_selected_contents(&mut self) -> String {
        match self.selected_state.selected() {
            Some(i) => self.matches[i].to_string(),
            None => String::from(""),
        }
    }

    pub fn next(&mut self) {
        let i = match self.selected_state.selected() {
            Some(i) => {
                if i >= self.matches.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.selected_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.selected_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.matches.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.selected_state.select(Some(i));
    }

    fn new() -> TerminalApp {
        TerminalApp {
            query_input: String::new(),
            filter_input: String::new(),
            preview: String::new(),
            matches: Vec::new(),
            selected_state: ListState::default(),
            error: String::new(),
            debug: String::new(),
            inp_idx: 0,
            inp_widths: [0, 0],
        }
    }
}

pub fn setup_panic() {
    std::panic::set_hook(Box::new(move |_x| {
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
        print!("");
    }));
}

/// Interactive query interface
pub fn query(
    mut db: Database,
    verbosity: u8,
    pager: String,
    editor: String,
) -> Result<Vec<String>, Report> {
    let mut tui = tui::Terminal::new(TermionBackend::new(AlternateScreen::from(
        stdout().into_raw_mode().unwrap(),
    )))
    .unwrap();

    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();

    let syntax = ps.find_syntax_by_extension("md").unwrap();
    // TODO make themes configurable
    let mut highlighter = HighlightLines::new(syntax, &ts.themes["Solarized (dark)"]);

    // Setup event handlers
    let mut events = event::Events::new();

    // Create default app state
    let mut app = TerminalApp::new();

    loop {
        // Draw UI
        if let Err(e) = tui.draw(|f| {
            let main = if verbosity > 0 {
                // Enable debug and error areas
                Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints(
                        [
                            // Content Preview Area
                            Constraint::Percentage(80),
                            // Debug Message Area
                            Constraint::Percentage(10),
                            // Error Message Area
                            Constraint::Percentage(10),
                        ]
                        .as_ref(),
                    )
                    .split(f.size())
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(f.size())
            };

            let screen = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints(
                    [
                        // Match results area
                        Constraint::Percentage(50),
                        // Document Preview area
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(main[0]);

            // Preview area where content is displayed
            let mut preview_text = String::from("");
            for line in LinesWithEndings::from(app.preview.as_ref()) {
                let ranges: Vec<(hStyle, &str)> = highlighter.highlight(line, &ps);
                let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
                preview_text.push_str(&escaped);
            }
            //let preview_text = Paragraph::new(ansi_to_text(preview_text.bytes()).unwrap())
            let preview_text = Paragraph::new(app.preview.as_ref())
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });
            f.render_widget(preview_text, screen[1]);

            // Output area where match titles are displayed
            // TODO panes specifically for tag, weight, date, author, id, parentid
            let interactive = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        // Match titles display area
                        Constraint::Min(20),
                        // Query input box
                        Constraint::Length(3),
                        // Filter input box
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(screen[0]);

            let selected_style = Style::default().add_modifier(Modifier::REVERSED);
            let matches: Vec<ListItem> = app
                .matches
                .iter()
                .map(|m| ListItem::new(vec![Spans::from(Span::raw(m.title.to_string()))]))
                .collect();
            let matches = List::new(matches)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(selected_style)
                .highlight_symbol("> ");
            f.render_stateful_widget(matches, interactive[0], &mut app.selected_state);

            // Input area where queries are entered
            let query_input = Paragraph::new(app.query_input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .title("Query input")
                        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
                );
            f.render_widget(query_input, interactive[1]);

            // Input area where filters are entered
            let filter_input = Paragraph::new(app.filter_input.as_ref())
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .title("Filter input (e.g. 'vim | !bash')")
                        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT),
                );
            f.render_widget(filter_input, interactive[2]);

            // Make the cursor visible and ask tui-rs to put it at the specified
            // coordinates after rendering
            f.set_cursor(
                // Put cursor past the end of the input text
                // TODO refactor input area switching
                interactive[app.inp_idx + 1].x + 1 + app.inp_widths[app.inp_idx] as u16,
                interactive[app.inp_idx + 1].y + 1,
            );

            if verbosity > 0 {
                // Area to display debug messages
                let debug = Paragraph::new(app.debug.as_ref())
                    .style(Style::default().fg(Color::Green).bg(Color::Black))
                    .block(
                        Block::default()
                            .title("Debug messages")
                            .borders(Borders::TOP | Borders::LEFT),
                    )
                    .wrap(Wrap { trim: true });
                f.render_widget(debug, main[1]);

                // Area to display Error messages
                let error = Paragraph::new(app.error.as_ref())
                    .style(Style::default().fg(Color::Red).bg(Color::Black))
                    .block(
                        Block::default()
                            .title("Error messages")
                            .borders(Borders::TOP | Borders::LEFT),
                    )
                    .wrap(Wrap { trim: true });
                f.render_widget(error, main[2]);
            }
        }) {
            tui.clear().unwrap();
            drop(tui);
            bail!("Failed to draw TUI App {}", e.to_string());
        }

        // Handle input
        match events.next() {
            Err(e) => {
                tui.clear().unwrap();
                drop(tui);
                bail!("Failed to handle input {}", e.to_string());
            }
            Ok(ev) => {
                if let event::Event::Input(input) = ev {
                    // TODO add support for:
                    //  - ctrl-e to open selected in $EDITOR, then submit on file close
                    //  - pageup/pagedn/home/end for navigating displayed selection
                    //  - ctrl-jkdu for navigating displayed selection
                    //  - ctrl-hl for navigating between links
                    //  - Limit query and filter input box length
                    //  - +/- (and return) to modify weight
                    //  - ctrl-m to toggle displaying frontmatter metadata (off by default)
                    match input {
                        Key::Char('\n') => {
                            // Select choice
                            // TODO increment weight for selected doc
                            break;
                        }
                        Key::Ctrl('c') => {
                            break;
                        }
                        Key::Left | Key::Right | Key::Char('\t') => {
                            app.inp_idx = match app.inp_idx {
                                1 => 0,
                                _ => 1,
                            };
                        }
                        Key::Char(c) => {
                            if app.inp_idx == 0 {
                                app.query_input.push(c);
                            } else {
                                app.filter_input.push(c);
                            }
                            app.inp_widths[app.inp_idx] += 1;
                        }
                        Key::Backspace => {
                            // TODO prevent this from going to far back
                            if app.inp_idx == 0 {
                                app.query_input.pop();
                            } else {
                                app.filter_input.pop();
                            }
                            app.inp_widths[app.inp_idx] -= 1;
                        }
                        Key::Ctrl('e') => {
                            // Temporarily drop the TUI app and event handling while
                            // we shell out to EDITOR, restore these on return
                            //events.tx.send("q");
                            drop(events);
                            tui.clear().unwrap();
                            drop(tui);
                            // TODO get rid of the random bytes here and use the doc id as part of
                            // the prefix
                            let mut tf = Builder::new()
                                .prefix("meilizet-")
                                .suffix(".md")
                                .rand_bytes(5)
                                .tempfile()?;
                            tf.write_all(app.get_selected_contents().as_bytes())?;
                            let editor = editor.clone();
                            let mut editor = editor.split_whitespace();
                            let mut cmd = Command::new(editor.next().unwrap());
                            for arg in editor {
                                cmd.arg(arg);
                            }
                            cmd.arg(tf.path())
                                .status()
                                .expect("failed to execute process");
                            events = event::Events::new();
                            tui = tui::Terminal::new(TermionBackend::new(AlternateScreen::from(
                                stdout().into_raw_mode().unwrap(),
                            )))
                            .unwrap();
                        }
                        Key::Ctrl('v') => {
                            // Temporarily drop the TUI app and event handling while
                            // we shell out to less, restore these on return
                            //events.tx.send("q");
                            drop(events);
                            tui.clear().unwrap();
                            drop(tui);
                            // TODO get rid of the random bytes here and use the doc id as part of
                            // the prefix
                            let mut tf = Builder::new()
                                .prefix("meilizet-")
                                .suffix(".md")
                                .rand_bytes(5)
                                .tempfile()?;
                            tf.write_all(app.get_selected_contents().as_bytes())?;
                            let viewer = pager.clone();
                            // Support setting PAGER="bat --paging always"
                            let mut viewer = viewer.split_whitespace();
                            let mut cmd = Command::new(viewer.next().unwrap());
                            for arg in viewer {
                                cmd.arg(arg);
                            }
                            cmd.arg(tf.path())
                                .status()
                                .expect("failed to execute process");
                            events = event::Events::new();
                            tui = tui::Terminal::new(TermionBackend::new(AlternateScreen::from(
                                stdout().into_raw_mode().unwrap(),
                            )))
                            .unwrap();
                        }
                        Key::Down | Key::Ctrl('n') => {
                            app.next();
                            app.preview = app.get_selected_contents();
                        }
                        Key::Up | Key::Ctrl('p') => {
                            app.previous();
                            app.preview = app.get_selected_contents();
                        }
                        _ => {}
                    }

                    let mut inp: String = app.query_input.to_owned();
                    // Add a trailing ` ;` to the query to hint to Nom that it has a "full" string
                    inp.push_str(" ;");

                    let enq = db.new_enquire()?;
                    match xapian_utils::parse_user_query(&inp) {
                        Ok(query) => {
                            //app.query = query.get_description();
                            app.matches = xapian_utils::query_db(enq, query)?;
                        }
                        Err(e) => {
                            app.error = e.to_string();
                        }
                    };
                    //let mut q = api::ApiQuery::new();
                    //q.query = Some(app.query_input.to_owned());

                    //q.process_filter(app.filter_input.to_owned());

                    //app.debug = serde_json::to_string(&q).unwrap();

                    //// Split up the JSON decoding into two steps.
                    //// 1.) Get the text of the body.
                    //let response_body = match client
                    //    .post(uri.as_ref())
                    //    .body::<String>(serde_json::to_string(&q).unwrap())
                    //    .header(CONTENT_TYPE, "application/json")
                    //    .send()
                    //{
                    //    Ok(resp) => {
                    //        if !resp.status().is_success() {
                    //            app.error = format!("Request failed: {:?}", resp);
                    //            continue;
                    //        }
                    //        match resp.text() {
                    //            Ok(text) => text,
                    //            Err(e) => {
                    //                app.error = format!("resp.text() failed: {:?}", e);
                    //                continue;
                    //            }
                    //        }
                    //    }
                    //    Err(e) => {
                    //        app.error = format!("Send failed: {:?}", e);
                    //        continue;
                    //    }
                    //};

                    //// 2.) Parse the results as JSON.
                    //match serde_json::from_str::<api::ApiResponse>(&response_body) {
                    //    Ok(mut resp) => {
                    //        app.matches = resp
                    //            .hits
                    //            .iter_mut()
                    //            .map(|mut m| {
                    //                m.serialization_type = document::SerializationType::Human;
                    //                m.to_owned()
                    //            })
                    //            .collect::<Vec<_>>();
                    //        app.error = String::from("");
                    //    }
                    //    Err(e) => {
                    //        app.error = format!(
                    //            "Could not deserialize body from: {}; error: {:?}",
                    //            response_body, e
                    //        )
                    //    }
                    //};
                }
            }
        }
    }

    tui.clear().unwrap();

    Ok(app.get_selected())
}

pub mod event {

    use std::io;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use termion::event::Key;
    use termion::input::TermRead;

    pub enum Event<I> {
        Input(I),
        Tick,
    }

    /// A small event handler that wrap termion input and tick events. Each event
    /// type is handled in its own thread and returned to a common `Receiver`
    pub struct Events {
        rx: mpsc::Receiver<Event<Key>>,
        #[allow(dead_code)]
        input_handle: thread::JoinHandle<()>,
        #[allow(dead_code)]
        tick_handle: thread::JoinHandle<()>,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Config {
        pub tick_rate: Duration,
    }

    impl Default for Config {
        fn default() -> Config {
            Config {
                tick_rate: Duration::from_millis(250),
            }
        }
    }

    impl Default for Events {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Events {
        pub fn new() -> Events {
            Events::with_config(Config::default())
        }

        pub fn with_config(config: Config) -> Events {
            let (tx, rx) = mpsc::channel();
            let input_handle = {
                let tx = tx.clone();
                thread::spawn(move || {
                    let stdin = io::stdin();
                    for evt in stdin.keys().flatten() {
                        if let Err(err) = tx.send(Event::Input(evt)) {
                            dbg!(err);
                            return;
                        }
                    }
                })
            };
            let tick_handle = {
                thread::spawn(move || loop {
                    if let Err(err) = tx.send(Event::Tick) {
                        dbg!(err);
                        break;
                    }
                    thread::sleep(config.tick_rate);
                })
            };
            Events {
                rx,
                input_handle,
                tick_handle,
            }
        }

        pub fn next(&self) -> Result<Event<Key>, mpsc::RecvError> {
            self.rx.recv()
        }
    }
}
