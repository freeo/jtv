//! Pure state transitions for the ordinary-string line editor.

use console::Key;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineBuffer {
    chars: Vec<char>,
    cursor: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditAction {
    Continue,
    Submit(String),
    Browse { buffer: String, cursor: usize },
    Cancel,
}

impl LineBuffer {
    pub fn new(value: &str) -> Self {
        let chars: Vec<_> = value.chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    pub fn value(&self) -> String {
        self.chars.iter().collect()
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn replace(&mut self, value: &str) {
        self.chars = value.chars().collect();
        self.cursor = self.chars.len();
    }

    pub fn apply(&mut self, key: Key) -> EditAction {
        match key {
            Key::Char(character) if !character.is_control() => {
                self.chars.insert(self.cursor, character);
                self.cursor += 1;
                EditAction::Continue
            }
            Key::ArrowLeft => {
                self.cursor = self.cursor.saturating_sub(1);
                EditAction::Continue
            }
            Key::ArrowRight => {
                self.cursor = (self.cursor + 1).min(self.chars.len());
                EditAction::Continue
            }
            Key::Home => {
                self.cursor = 0;
                EditAction::Continue
            }
            Key::End => {
                self.cursor = self.chars.len();
                EditAction::Continue
            }
            Key::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
                EditAction::Continue
            }
            Key::Del if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
                EditAction::Continue
            }
            Key::Enter => EditAction::Submit(self.value()),
            Key::Tab => EditAction::Browse {
                buffer: self.value(),
                cursor: self.cursor,
            },
            Key::Escape | Key::CtrlC => EditAction::Cancel,
            _ => EditAction::Continue,
        }
    }

    pub fn suffix(&self) -> String {
        self.chars[self.cursor..].iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_by_character_and_reports_browse_state() {
        let mut line = LineBuffer::new("a界");
        line.apply(Key::ArrowLeft);
        line.apply(Key::Char('λ'));
        assert_eq!(line.value(), "aλ界");
        assert_eq!(line.cursor(), 2);
        assert_eq!(line.suffix(), "界");
        assert_eq!(
            line.apply(Key::Tab),
            EditAction::Browse {
                buffer: "aλ界".into(),
                cursor: 2
            }
        );
    }

    #[test]
    fn supports_navigation_deletion_submission_and_cancellation() {
        let mut line = LineBuffer::new("abc");
        line.apply(Key::Home);
        line.apply(Key::Del);
        line.apply(Key::End);
        line.apply(Key::Backspace);
        assert_eq!(line.apply(Key::Enter), EditAction::Submit("b".into()));
        assert_eq!(line.apply(Key::Escape), EditAction::Cancel);
    }

    #[test]
    fn selected_completion_replaces_a_midline_buffer_and_moves_to_the_end() {
        let mut line = LineBuffer::new("docs-old-suffix");
        for _ in 0..11 {
            line.apply(Key::ArrowLeft);
        }
        assert_eq!(line.cursor(), 4);
        line.replace("docs/guide λ.md");
        assert_eq!(line.value(), "docs/guide λ.md");
        assert_eq!(line.cursor(), line.value().chars().count());
        assert_eq!(line.suffix(), "");
    }
}
