mod clipboard;
mod command;
mod config;
mod document;
mod editor;
mod terminal;
mod warn;

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use clipboard::Clipboard;
use command::ExecContext;
use document::Document;
use editor::Editor;
use terminal::Screen;
use warn::WarnPopup;

#[derive(Parser, Debug)]
#[command(version, about = "A terminal text editor")]
struct Cli {
    /// Open the file in read-only mode
    #[arg(short = 'r', long = "readonly")]
    readonly: bool,

    /// File to open
    path: Option<PathBuf>,
}

enum Mode {
    Editing,
    CommandBar(String),
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let mut doc = match cli.path {
        Some(path) => {
            Document::open(path).unwrap_or_else(|_| Document::new_empty())
        }
        None => Document::new_empty(),
    };

    if cli.readonly {
        doc.read_only = true;
    }

    let mut editor = Editor::new(doc);

    let mut clipboard = Clipboard::new();
    let mut warn = WarnPopup::new();

    let (theme, theme_warning) = config::load_theme();
    let mut screen = Screen::init(theme)?;

    if let Some(msg) = theme_warning {
        warn.show(msg);
    }

    let mut mode = Mode::Editing;
    let mut should_quit = false;

    loop {
        warn.tick();

        let command_bar_text = match &mode {
            Mode::CommandBar(s) => Some(s.as_str()),
            Mode::Editing => None,
        };

        screen.draw(&editor, command_bar_text, &warn)?;

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        match &mut mode {
            Mode::Editing => {
                if editor.handle_key(key) {
                    mode = Mode::CommandBar(String::new());
                }
            }

            Mode::CommandBar(buf) => match key.code {
                KeyCode::Esc => {
                    mode = Mode::Editing;
                }

                KeyCode::Char(c) => {
                    buf.push(c);
                }

                KeyCode::Backspace => {
                    buf.pop();
                }

                KeyCode::Up => {
                    if let Some(command) = editor.previous_command() {
                        *buf = command.to_string();
                    }
                }

                KeyCode::Down => match editor.next_command() {
                    Some(command) => *buf = command.to_string(),
                    None => buf.clear(),
                },

                KeyCode::Enter => {
                    let command = buf.clone();

                    editor.add_command_history(command.clone());

                    let nodes = command::parse(&command);
                    mode = Mode::Editing;

                    match nodes {
                        Ok(nodes) => {
                            let mut ctx = ExecContext {
                                editor: &mut editor,
                                clipboard: &mut clipboard,
                                warn: &mut warn,
                                should_quit: &mut should_quit,
                            };

                            command::run(&nodes, &mut ctx);
                        }

                        Err(e) => {
                            warn.show(e.to_string());
                        }
                    }
                }

                _ => {}
            },
        }

        if should_quit {
            break;
        }

        screen.refresh_size()?;
    }

    Ok(())
}