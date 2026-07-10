use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFrame {
    pub text: String,
    pub rows: u16,
    pub columns: u16,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub alternate_screen: bool,
}

impl ScreenFrame {
    pub fn from_parser(parser: &vt100::Parser) -> Self {
        let screen = parser.screen();
        let (rows, columns) = screen.size();
        Self {
            text: normalize_contents(&screen.contents()),
            rows,
            columns,
            cursor: screen.cursor_position(),
            cursor_visible: !screen.hide_cursor(),
            alternate_screen: screen.alternate_screen(),
        }
    }

    pub fn contains(&self, text: &str) -> bool {
        self.text.contains(text)
    }

    pub fn diagnostic(&self) -> String {
        format!(
            "screen={}x{} cursor={:?} visible={} alternate={}\n{}",
            self.columns,
            self.rows,
            self.cursor,
            self.cursor_visible,
            self.alternate_screen,
            self.text
        )
    }

    /// Stable, reviewable representation of the user-visible character grid.
    ///
    /// The only scrubbed value is the test sandbox's absolute path. Opaque
    /// session IDs are deliberately not broadly regex-redacted: if one becomes
    /// visible, the snapshot should fail and expose that product regression.
    pub fn snapshot_text(&self, sandbox_root: &Path) -> String {
        let root = sandbox_root.to_string_lossy();
        let text = self.text.replace(root.as_ref(), "<SANDBOX>");
        format!(
            "viewport: {}x{}\ncursor: {},{} visible={}\nalternate-screen: {}\n---\n{}",
            self.columns,
            self.rows,
            self.cursor.0,
            self.cursor.1,
            self.cursor_visible,
            self.alternate_screen,
            text
        )
    }
}

fn normalize_contents(contents: &str) -> String {
    let mut rows = contents.lines().map(str::trim_end).collect::<Vec<_>>();
    while rows.last().is_some_and(|row| row.is_empty()) {
        rows.pop();
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_only_trailing_screen_space() {
        let mut parser = vt100::Parser::new(3, 20, 0);
        parser.process(b"  meaningful  \r\n\r\n");
        let frame = ScreenFrame::from_parser(&parser);
        assert_eq!(frame.text, "  meaningful");
    }

    #[test]
    fn snapshot_normalizes_only_the_explicit_sandbox_root() {
        let frame = ScreenFrame {
            text: "/tmp/jtv-random/project jtv-session-should-remain".into(),
            rows: 40,
            columns: 120,
            cursor: (3, 7),
            cursor_visible: true,
            alternate_screen: true,
        };
        let snapshot = frame.snapshot_text(Path::new("/tmp/jtv-random"));
        assert!(snapshot.contains("<SANDBOX>/project"));
        assert!(snapshot.contains("jtv-session-should-remain"));
    }
}
