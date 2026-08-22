mod clipboard;
mod command;
mod document;
mod editor;
mod terminal;
mod warn;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use clipboard::Clipboard;
use command::ExecContext;
use document::Document;
use editor::Editor;
use terminal::Screen;
use warn::WarnPopup;

enum Mode {
    Editing,
    CommandBar(String),
}

fn main() -> std::io::Result<()> {
    let path_arg = std::env::args().nth(1);

    let doc = match path_arg {
        Some(p) => {
            Document::open(PathBuf::from(p), false).unwrap_or_else(|_| Document::new_empty())
        }
        None => Document::new_empty(),
    };

    let mut editor = Editor::new(doc);

    let mut clipboard = Clipboard::new();
    let mut warn = WarnPopup::new();
    let mut screen = Screen::init()?;
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

                KeyCode::Enter => {
                    let nodes = command::parse(buf);
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
