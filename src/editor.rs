use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{EditResult, document::Document};

#[derive(Clone)]
pub struct Editor {
    pub documents: Vec<Document>,
    pub active: usize,

    pub command_history: Vec<String>,
    pub history_idx: Option<usize>,

    next_document_id: u64,
}

impl Editor {
    pub fn new(mut document: Document) -> Self {
        let id = 1;
        document.id = id;

        Self {
            documents: vec![document],
            active: 0,
            command_history: Vec::new(),
            history_idx: None,
            next_document_id: id + 1,
        }
    }

    pub fn add_command_history(&mut self, command: String) {
        if command.is_empty() {
            return;
        }

        if self.command_history.last() == Some(&command) {
            self.history_idx = None;
            return;
        }

        self.command_history.push(command);
        self.history_idx = None;
    }

    pub fn previous_command(&mut self) -> Option<&str> {
        if self.command_history.is_empty() {
            return None;
        }

        let index = match self.history_idx {
            Some(index) => index.saturating_sub(1),
            None => self.command_history.len() - 1,
        };

        self.history_idx = Some(index);
        Some(&self.command_history[index])
    }

    pub fn next_command(&mut self) -> Option<&str> {
        let index = match self.history_idx {
            Some(index) if index + 1 < self.command_history.len() => index + 1,
            _ => {
                self.history_idx = None;
                return None;
            }
        };

        self.history_idx = Some(index);
        Some(&self.command_history[index])
    }

    pub fn find_doc_index(&self, id: u64) -> Option<usize> {
        self.documents.iter().position(|doc| doc.id == id)
    }

    pub fn get_docs_len(&self) -> usize {
        self.documents.len()
    }

    pub fn get_doc_by_id(&self, id: u64) -> Option<&Document> {
        self.find_doc_index(id).map(|index| &self.documents[index])
    }

    pub fn get_doc_by_id_mut(&mut self, id: u64) -> Option<&mut Document> {
        self.find_doc_index(id)
            .map(|index| &mut self.documents[index])
    }

    pub fn set_doc_by_id(&mut self, id: u64, document: Document) -> bool {
        let Some(index) = self.find_doc_index(id) else {
            return false;
        };

        let mut document = document;
        document.id = id;

        self.documents[index] = document;

        true
    }

    pub fn close_doc(&mut self, id: u64) -> bool {
        let Some(index) = self.find_doc_index(id) else {
            return false;
        };

        self.documents.remove(index);

        if self.documents.is_empty() {
            self.active = 0;
        } else if self.active > index {
            self.active -= 1;
        } else if self.active >= self.documents.len() {
            self.active = self.documents.len() - 1;
        }

        true
    }

    pub fn doc(&self) -> &Document {
        &self.documents[self.active]
    }

    pub fn doc_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active]
    }

    pub fn active_id(&self) -> u64 {
        self.documents[self.active].id
    }

    pub fn has_dirty(&self) -> bool {
        self.documents.iter().any(|d| d.dirty)
    }

    pub fn open(&mut self, mut document: Document) -> u64 {
        let id = self.next_document_id;
        self.next_document_id += 1;

        document.id = id;

        self.documents.push(document);
        self.active = self.documents.len() - 1;

        id
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

    pub fn close(&mut self) -> bool {
        if self.documents.is_empty() {
            return false;
        }

        let id = self.active_id();
        self.close_doc(id)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditResult {
        let id = self.active_id();

        match key.code {
            KeyCode::Tab => {
                self.doc_mut().indent();
                EditResult::Changed(id)
            }

            KeyCode::BackTab => {
                self.doc_mut().dedent();
                EditResult::Changed(id)
            }

            KeyCode::Delete if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.doc_mut().delete_word_forward();
                EditResult::Changed(id)
            }

            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.doc_mut().delete_word_forward();
                EditResult::Changed(id)
            }

            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.doc_mut().delete_word_backward();
                EditResult::Changed(id)
            }

            KeyCode::Char(c) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().insert_char(c);
                    EditResult::Changed(id)
                } else {
                    EditResult::Nothing
                }
            }

            KeyCode::Enter => {
                self.doc_mut().insert_newline();
                EditResult::Changed(id)
            }

            KeyCode::Backspace => {
                self.doc_mut().backspace();
                EditResult::Changed(id)
            }

            KeyCode::Delete => {
                self.doc_mut().delete();
                EditResult::Changed(id)
            }

            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().move_word_left();
                } else {
                    self.doc_mut().move_left();
                }

                EditResult::Nothing
            }

            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.doc_mut().move_word_right();
                } else {
                    self.doc_mut().move_right();
                }

                EditResult::Nothing
            }

            KeyCode::Up => {
                self.doc_mut().move_up(1);
                EditResult::Nothing
            }

            KeyCode::Down => {
                self.doc_mut().move_down(1);
                EditResult::Nothing
            }

            KeyCode::Esc => EditResult::CommandBar,

            _ => EditResult::Nothing,
        }
    }
}
