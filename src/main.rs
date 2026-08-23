mod clipboard;
mod command;
mod config;
mod document;
mod editor;
mod plugin;
mod terminal;
mod warn;

use std::rc::Rc;
use std::time::Duration;
use std::{cell::RefCell, path::PathBuf};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use clipboard::Clipboard;
use command::ExecContext;
use document::Document;
use editor::Editor;
use terminal::Screen;
use warn::WarnPopup;

use crate::plugin::PluginRuntime;

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
        Some(path) => Document::open(path).unwrap_or_else(|_| Document::new_empty()),
        None => Document::new_empty(),
    };

    if cli.readonly {
        doc.read_only = true;
    }

    let editor = Rc::new(RefCell::new(Editor::new(doc)));
    let warn = Rc::new(RefCell::new(WarnPopup::new()));

    let mut clipboard = Clipboard::new();
    let mut plugins = PluginRuntime::load(editor.clone(), warn.clone());

    let (theme, theme_warning) = config::load_theme();
    let mut screen = Screen::init(theme)?;

    if let Some(msg) = theme_warning {
        warn.borrow_mut().show(msg);
    }

    let mut mode = Mode::Editing;
    let mut should_quit = false;

    loop {
        warn.borrow_mut().tick();

        let command_bar_text = match &mode {
            Mode::CommandBar(s) => Some(s.as_str()),
            Mode::Editing => None,
        };

        screen.draw(&editor.borrow(), command_bar_text, &warn.borrow())?;

        plugins.tick_timers();

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
                if plugins.in_plugin_mode() {
                    if key.code == KeyCode::Esc {
                        plugins.exit_plugin_mode();
                    } else if let Some(name) = plugin::key_name(&key) {
                        if !plugins.dispatch_key(&name) {
                            let mut ed = editor.borrow_mut();
                            match key.code {
                                KeyCode::Up => ed.doc_mut().move_up(1),
                                KeyCode::Down => ed.doc_mut().move_down(1),
                                _ => {}
                            }
                        }
                    }
                } else if editor.borrow_mut().handle_key(key) {
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
                    let mut ed = editor.borrow_mut();
                    if let Some(command) = ed.previous_command() {
                        *buf = command.to_string();
                    }
                }

                KeyCode::Down => {
                    let mut ed = editor.borrow_mut();
                    match ed.next_command() {
                        Some(command) => *buf = command.to_string(),
                        None => buf.clear(),
                    }
                }

                KeyCode::Enter => {
                    let command = buf.clone();

                    editor.borrow_mut().add_command_history(command.clone());

                    let nodes = command::parse(&command);
                    mode = Mode::Editing;

                    match nodes {
                        Ok(nodes) => {
                            let mut ctx = ExecContext {
                                editor: editor.clone(),
                                clipboard: &mut clipboard,
                                warn: warn.clone(),
                                should_quit: &mut should_quit,
                                plugins: &mut plugins,
                            };

                            command::run(&nodes, &mut ctx);
                        }

                        Err(e) => {
                            warn.borrow_mut().show(e.to_string());
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
