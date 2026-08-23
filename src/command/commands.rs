use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use super::ast::Arg;
use crate::clipboard::Clipboard;
use crate::command::commands::CommandKind::Replace;
use crate::document::Document;
use crate::editor::Editor;
use crate::plugin::PluginRuntime;
use crate::warn::WarnPopup;

pub struct ExecContext<'a> {
    pub editor: Rc<RefCell<Editor>>,
    pub clipboard: &'a mut Clipboard,
    pub warn: Rc<RefCell<WarnPopup>>,
    pub should_quit: &'a mut bool,
    pub plugins: &'a mut PluginRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Save,
    Quit,
    Exit,
    Edit,
    Find,
    Replace,
    Goto,
    Undo,
    Redo,
    Execute,
    Copy,
    Paste,
    Delete,
    Match,
    PreviousFile,
    NextFile,
    Name,
    Yank,
}

impl CommandKind {
    pub fn from_char(c: char) -> Option<Self> {
        use CommandKind::*;
        Some(match c {
            's' => Save,
            'q' => Quit,
            'Q' => Exit,
            'e' => Edit,
            'f' => Find,
            'R' => Replace,
            'g' => Goto,
            'u' => Undo,
            'r' => Redo,
            'x' => Execute,
            'c' => Copy,
            'p' => Paste,
            'd' => Delete,
            'm' => Match,
            'A' => PreviousFile,
            'D' => NextFile,
            'n' => Name,
            'y' => Yank,
            _ => return None,
        })
    }

    pub fn trigger_change(&self) -> bool {
        match self {
            Self::Replace | Self::Undo | Self::Redo | Self::Paste | Self::Delete | Self::Yank => {
                true
            }

            _ => false,
        }
    }

    pub fn help(&self) -> &'static str {
        use CommandKind::*;
        match self {
            Save => {
                "[s] save the current document (s! to override read-only, might need root privilages)"
            }
            Quit => "[q] close file (q! to force close with unsaved changes)",
            Exit => "[Q] quit ink",
            Edit => "[e]{path} open/edit a file",
            Find => "[f]{find} find next occurrence of find, wrapping to the start if needed",
            Replace => "[R]{find}{replace} replace the next occurrence of find with replace",
            Goto => {
                "[g]{N/arg} go to line N (or: sl-startOfLine, el-endOfLine, sf-startOfFile, ef-endOfFile)"
            }
            Undo => "[u] undo",
            Redo => "[r] redo",
            Execute => "[x]{command} execute a shell command",
            Copy => "[c] copy current line to the system clipboard",
            Paste => "[p] paste from the system clipboard",
            Delete => "[d] delete to end of line from the cursor",
            Match => "[m] match to the relevant delimiter",
            PreviousFile => "[A] go to the previous file in the editor",
            NextFile => "[D] go to the next file in the editor",
            Name => "[n] set the name of the currently selected file.",
            Yank => "[y] duplicate the current line",
        }
    }

    pub fn run(&self, args: &[Arg], forced: bool, ctx: &mut ExecContext) {
        use CommandKind::*;
        match self {
            Save => {
                let result = {
                    let mut editor = ctx.editor.borrow_mut();
                    if forced {
                        editor.doc_mut().save_forced()
                    } else {
                        editor.doc_mut().save()
                    }
                };
                if let Err(e) = result {
                    ctx.warn.borrow_mut().show(e.to_string());
                }
            }
            Quit => {
                let dirty = ctx.editor.borrow().doc().dirty;
                if dirty && !forced {
                    ctx.warn
                        .borrow_mut()
                        .show("unsaved changes. use q! to force quit");
                } else {
                    let mut editor = ctx.editor.borrow_mut();
                    editor.close();

                    if editor.get_docs_len() == 0 {
                        *ctx.should_quit = true;
                    }
                }
            }
            Exit => {
                let has_dirty = ctx.editor.borrow().has_dirty();
                if has_dirty && !forced {
                    ctx.warn
                        .borrow_mut()
                        .show("unsaved changes in a file. use Q! to force quit");
                } else {
                    *ctx.should_quit = true;
                }
            }
            Edit => match args.first() {
                Some(arg) => match Document::open(PathBuf::from(arg.as_str())) {
                    Ok(new_doc) => {
                        ctx.editor.borrow_mut().open(new_doc);
                    }
                    Err(e) => ctx.warn.borrow_mut().show(format!("open failed: {e}")),
                },

                None => {
                    ctx.editor.borrow_mut().open(Document::new_empty());
                }
            },
            Find => match args.first() {
                Some(needle) => {
                    let found = ctx.editor.borrow_mut().doc_mut().find_next(needle.as_str());
                    if !found {
                        ctx.warn
                            .borrow_mut()
                            .show(format!("not found: {}", needle.as_str()));
                    }
                }
                None => ctx
                    .warn
                    .borrow_mut()
                    .show("f requires a search term, e.g. f;needle;"),
            },
            Replace => match (args.first(), args.get(1)) {
                (Some(needle), Some(replacement)) => {
                    let _ = forced;
                    let found = ctx
                        .editor
                        .borrow_mut()
                        .doc_mut()
                        .replace_next(needle.as_str(), replacement.as_str());
                    if !found {
                        ctx.warn
                            .borrow_mut()
                            .show(format!("not found: {}", needle.as_str()));
                    }
                }
                _ => ctx
                    .warn
                    .borrow_mut()
                    .show("R requires two arguments, e.g. R;needle:replacement;"),
            },
            Goto => match args.first().map(|a| a.as_str()) {
                Some("ef") => {
                    let mut editor = ctx.editor.borrow_mut();
                    let doc = editor.doc_mut();

                    if let Some(line) = doc.lines.last() {
                        let line_len = line.len();
                        doc.cursor_line = doc.lines.len().saturating_sub(1);
                        doc.cursor_col = line_len;
                    }
                }

                Some("sf") => {
                    let mut editor = ctx.editor.borrow_mut();
                    editor.doc_mut().goto_line(0);
                    editor.doc_mut().cursor_col = 0;
                }

                Some("el") => {
                    let mut editor = ctx.editor.borrow_mut();
                    let line_len = editor
                        .doc()
                        .lines
                        .get(editor.doc().cursor_line)
                        .map(|l| l.len());
                    if let Some(len) = line_len {
                        editor.doc_mut().cursor_col = len;
                    }
                }

                Some("sl") => {
                    ctx.editor.borrow_mut().doc_mut().cursor_col = 0;
                }

                Some(arg) => match arg.parse::<usize>() {
                    Ok(line) => ctx.editor.borrow_mut().doc_mut().goto_line(line),
                    Err(_) => ctx
                        .warn
                        .borrow_mut()
                        .show("g requires a line number or position, g;N; / g;ef/sf; g;el/sl;"),
                },

                None => ctx
                    .warn
                    .borrow_mut()
                    .show("g requires a line number or position, g;N; / g;ef/sf; g;el/sl;"),
            },
            Undo => ctx.editor.borrow_mut().doc_mut().undo(),
            Redo => ctx.editor.borrow_mut().doc_mut().redo(),
            Execute => match args.first() {
                Some(cmdline) => {
                    match std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmdline.as_str())
                        .output()
                    {
                        Ok(output) => {
                            let mut text = String::new();

                            if !output.stdout.is_empty() {
                                text.push_str(&String::from_utf8_lossy(&output.stdout));
                            }

                            if !output.stderr.is_empty() {
                                if !text.is_empty() && !text.ends_with('\n') {
                                    text.push('\n');
                                }

                                text.push_str(&String::from_utf8_lossy(&output.stderr));
                            }

                            if !text.is_empty() {
                                let mut doc = Document::from_text("out", text);
                                doc.read_only = true;
                                ctx.editor.borrow_mut().open(doc);
                            }
                        }

                        Err(e) => {
                            ctx.warn
                                .borrow_mut()
                                .show(format!("failed to run command: {e}"));
                        }
                    }
                }

                None => ctx
                    .warn
                    .borrow_mut()
                    .show("x requires a command, e.g. x;cargo check;"),
            },
            Copy => {
                let text = {
                    let editor = ctx.editor.borrow();
                    editor
                        .doc()
                        .lines
                        .get(editor.doc().cursor_line)
                        .cloned()
                        .unwrap_or_default()
                };
                if let Err(e) = ctx.clipboard.copy(&text) {
                    ctx.warn.borrow_mut().show(e);
                }
            }
            Paste => match ctx.clipboard.paste() {
                Ok(text) => ctx.editor.borrow_mut().doc_mut().insert_text(&text),
                Err(e) => ctx.warn.borrow_mut().show(e),
            },
            Delete => ctx.editor.borrow_mut().doc_mut().delete_to_end_of_line(),
            Match => {
                ctx.editor.borrow_mut().doc_mut().jump_to_match();
            }

            PreviousFile => ctx.editor.borrow_mut().previous(),
            NextFile => ctx.editor.borrow_mut().next(),

            Name => match args.first() {
                Some(arg) => {
                    ctx.editor.borrow_mut().doc_mut().path = Some(PathBuf::from(arg.as_str()));
                }

                None => ctx
                    .warn
                    .borrow_mut()
                    .show("n requires a filename, e.g. n;file.txt;"),
            },

            Yank => ctx.editor.borrow_mut().doc_mut().duplicate_line(),
        }
    }
}
