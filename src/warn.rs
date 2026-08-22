use std::time::{Duration, Instant};

const WARN_DURATION: Duration = Duration::from_secs(3);

pub struct WarnPopup {
    current: Option<(String, Instant)>,
}

impl WarnPopup {
    pub fn new() -> Self {
        Self { current: None }
    }

    pub fn show(&mut self, message: impl Into<String>) {
        self.current = Some((message.into(), Instant::now()));
    }

    pub fn tick(&mut self) {
        if let Some((_, shown_at)) = &self.current {
            if shown_at.elapsed() >= WARN_DURATION {
                self.current = None;
            }
        }
    }

    pub fn render_text(&self) -> Option<String> {
        self.current
            .as_ref()
            .map(|(msg, _)| format!("!warn: {msg}"))
    }
}
