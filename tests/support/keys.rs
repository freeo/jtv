#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Tab,
    BackTab,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Ctrl(char),
}

impl Key {
    pub fn bytes(self) -> Vec<u8> {
        match self {
            Self::Enter => b"\r".to_vec(),
            Self::Tab => b"\t".to_vec(),
            Self::BackTab => b"\x1b[Z".to_vec(),
            Self::Escape => b"\x1b".to_vec(),
            Self::Backspace => b"\x7f".to_vec(),
            Self::Delete => b"\x1b[3~".to_vec(),
            Self::Up => b"\x1b[A".to_vec(),
            Self::Down => b"\x1b[B".to_vec(),
            Self::Right => b"\x1b[C".to_vec(),
            Self::Left => b"\x1b[D".to_vec(),
            Self::Home => b"\x1b[H".to_vec(),
            Self::End => b"\x1b[F".to_vec(),
            Self::PageUp => b"\x1b[5~".to_vec(),
            Self::PageDown => b"\x1b[6~".to_vec(),
            Self::Ctrl(character) if character.is_ascii_alphabetic() => {
                vec![character.to_ascii_uppercase() as u8 - b'@']
            }
            Self::Ctrl(character) => panic!("Ctrl key must be an ASCII letter, got {character:?}"),
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Ctrl(character) => format!("Ctrl-{}", character.to_ascii_uppercase()),
            other => format!("{other:?}"),
        }
    }
}
