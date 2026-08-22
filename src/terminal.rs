use std::io::{self, Write};

use crossterm::{
    cursor, execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use unicode_width::UnicodeWidthStr;

use crate::document::Document;
use crate::warn::WarnPopup;

pub struct Screen {
    stdout: io::Stdout,
    pub width: u16,
    pub height: u16,
}

impl Screen {
    pub fn init() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        let (width, height) = terminal::size()?;
        Ok(Self { stdout, width, height })
    }

    pub fn refresh_size(&mut self) -> io::Result<()> {
        let (w, h) = terminal::size()?;
        self.width = w;
        self.height = h;
        Ok(())
    }

    fn text_rows(&self) -> u16 {
        self.height.saturating_sub(3)
    }

    fn text_start_row(&self) -> u16 {
        1
    }

    pub fn draw(
        &mut self,
        doc: &Document,
        command_bar: Option<&str>,
        warn: &WarnPopup,
    ) -> io::Result<()> {
        queue!(self.stdout, cursor::Hide, cursor::MoveTo(0, 0))?;
        queue!(self.stdout, Clear(ClearType::All))?;

        if let Some(w) = warn.render_text() {
            queue!(self.stdout, cursor::MoveTo(0, 0))?;
            queue!(self.stdout, Print(truncate_to_width(&w, self.width as usize)))?;
        }

        let start = self.text_start_row();
        let rows = self.text_rows();
        for i in 0..rows {
            let row = start + i;
            queue!(self.stdout, cursor::MoveTo(0, row))?;
            if let Some(line) = doc.lines.get(i as usize) {
                let truncated = truncate_to_width(line, self.width as usize);
                queue!(self.stdout, Print(truncated))?;
            } else {
                queue!(self.stdout, Print("~"))?;
            }
        }

        let status_row = self.height.saturating_sub(2);
        queue!(self.stdout, cursor::MoveTo(0, status_row))?;
        let name = doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[no name]");
        let dirty = if doc.dirty { "*" } else { "" };
        let ro = if doc.read_only { " [ro]" } else { "" };
        let status = format!(
            "{name}{dirty}{ro} -- {}:{}",
            doc.cursor_line + 1,
            doc.cursor_col + 1
        );
        queue!(self.stdout, Print(truncate_to_width(&status, self.width as usize)))?;

        let bottom_row = self.height.saturating_sub(1);
        queue!(self.stdout, cursor::MoveTo(0, bottom_row))?;
        if let Some(cmd) = command_bar {
            queue!(self.stdout, Print(format!(":{cmd}")))?;
        }

        if let Some(cmd) = command_bar {
            let col = 1 + UnicodeWidthStr::width(cmd);
            queue!(self.stdout, cursor::MoveTo(col as u16, bottom_row), cursor::Show)?;
        } else {
            queue!(
                self.stdout,
                cursor::MoveTo(doc.cursor_col as u16, start + doc.cursor_line as u16),
                cursor::Show
            )?;
        }

        self.stdout.flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = UnicodeWidthStr::width(c.to_string().as_str());
        if w + cw > max_width {
            break;
        }
        out.push(c);
        w += cw;
    }
    out
}
