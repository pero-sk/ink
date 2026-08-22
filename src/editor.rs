use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::document::Document;

pub struct Editor {
    pub documents: Vec<Document>,
    pub active: usize,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        Self {
            documents: vec![document],
            active: 0,
        }
    }

    pub fn doc_len(&self) -> usize {
        self.documents.len()
    }

    pub fn doc(&self) -> &Document {
        &self.documents[self.active]
    }

    pub fn has_dirty(&self) -> bool {
        for (_i, d) in self.documents.iter().enumerate() {
            if d.dirty {
                return true;
            }
        }

        false
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active]
    }

    pub fn open(&mut self, document: Document) {
        self.documents.push(document);
        self.active = self.documents.len() - 1;
    }

    pub fn next(&mut self) {
        if self.documents.len() > 1 {
            self.active = (self.active + 1) % self.documents.len();
        }
    }

    pub fn previous(&mut self) {
        if self.documents.len() > 1 {
            self.active = self
                .active
                .checked_sub(1)
                .unwrap_or(self.documents.len() - 1);
        }
    }

    pub fn close(&mut self) {
        if self.documents.is_empty() {
            return;
        }

        self.documents.remove(self.active);

        if !self.documents.is_empty() && self.active >= self.documents.len() {
            self.active = self.documents.len() - 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Tab => {
                self.doc_mut().indent();
            }

            KeyCode::BackTab => {
                self.doc_mut().dedent();
            }

            KeyCode::Delete if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.doc_mut().delete_word_forward();
            }

            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.doc_mut().delete_word_forward();
            }

            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.doc_mut().delete_word_backward();
            }

            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().insert_char(c);
                }
            }

            KeyCode::Enter => {
                self.doc_mut().insert_newline();
            }

            KeyCode::Backspace => {
                self.doc_mut().backspace();
            }

            KeyCode::Delete => {
                self.doc_mut().delete();
            }

            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().move_word_left();
                } else {
                    self.doc_mut().move_left();
                }
            }

            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().move_word_right();
                } else {
                    self.doc_mut().move_right();
                }
            }

            KeyCode::Up => {
                self.doc_mut().move_up(1);
            }

            KeyCode::Down => {
                self.doc_mut().move_down(1);
            }

            KeyCode::Esc => {
                return true;
            }

            _ => {}
        }

        false
    }
}
