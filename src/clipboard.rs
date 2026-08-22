pub struct Clipboard {
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            inner: arboard::Clipboard::new().ok(),
        }
    }

    pub fn copy(&mut self, text: &str) -> Result<(), String> {
        match &mut self.inner {
            Some(cb) => cb
                .set_text(text.to_owned())
                .map_err(|e| format!("clipboard copy failed: {e}")),
            None => Err("terminal has no clipboard. c/p is nop".to_string()),
        }
    }

    pub fn paste(&mut self) -> Result<String, String> {
        match &mut self.inner {
            Some(cb) => cb
                .get_text()
                .map_err(|e| format!("clipboard paste failed: {e}")),
            None => Err("terminal has no clipboard. c/p is nop".to_string()),
        }
    }
}
