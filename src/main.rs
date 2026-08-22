mod clipboard;
mod command;
mod document;
mod terminal;
mod warn;

use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

use clipboard::Clipboard;
use command::{ExecContext};
use document::Document;
use terminal::Screen;
use warn::WarnPopup;

enum Mode {
    Editing,
    CommandBar(String),
}

fn main() -> std::io::Result<()> {
    let path_arg = std::env::args().nth(1);
    let mut doc = match path_arg {
        Some(p) => Document::open(PathBuf::from(p), false).unwrap_or_else(|_| Document::new_empty()),
        None => Document::new_empty(),
    };

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
        screen.draw(&doc, command_bar_text, &warn)?;

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
            Mode::Editing => match key.code {
                KeyCode::Esc => mode = Mode::CommandBar(String::new()),
                KeyCode::Char(c) => {
                    if !key.modifiers.contains(KeyModifiers::CONTROL) {
                        doc.insert_char(c);
                    }
                }
                KeyCode::Enter => doc.insert_newline(),
                KeyCode::Backspace => doc.backspace(),
                KeyCode::Delete => doc.delete(),
                KeyCode::Left => doc.move_left(),
                KeyCode::Right => doc.move_right(),
                KeyCode::Up => doc.move_up(1),
                KeyCode::Down => doc.move_down(1),
                _ => {}
            },
            Mode::CommandBar(buf) => match key.code {
                KeyCode::Esc => mode = Mode::Editing,
                KeyCode::Char(c) => buf.push(c),
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Enter => {
                    let nodes = command::parse(buf);
                    mode = Mode::Editing;
                    match nodes {
                        Ok(nodes) => {
                            let mut ctx = ExecContext {
                                doc: &mut doc,
                                clipboard: &mut clipboard,
                                warn: &mut warn,
                                should_quit: &mut should_quit,
                            };
                            command::run(&nodes, &mut ctx);
                        }
                        Err(e) => warn.show(e.to_string()),
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
