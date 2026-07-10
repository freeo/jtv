use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenColor {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellStyle {
    pub foreground: ScreenColor,
    pub background: ScreenColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            foreground: ScreenColor::Default,
            background: ScreenColor::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledCell {
    pub text: String,
    pub style: CellStyle,
    pub wide: bool,
    pub continuation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRegion {
    pub row: u16,
    pub start_column: u16,
    pub end_column: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleRun {
    pub row: u16,
    pub start_column: u16,
    pub end_column: u16,
    pub text: String,
    pub style: CellStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFrame {
    pub text: String,
    pub rows: u16,
    pub columns: u16,
    pub cursor: (u16, u16),
    pub cursor_visible: bool,
    pub alternate_screen: bool,
    cells: Vec<StyledCell>,
}

impl ScreenFrame {
    pub fn from_parser(parser: &vt100::Parser) -> Self {
        let screen = parser.screen();
        let (rows, columns) = screen.size();
        let cells = (0..rows)
            .flat_map(|row| {
                (0..columns).map(move |column| {
                    let cell = screen.cell(row, column).expect("cell inside screen bounds");
                    StyledCell {
                        text: cell.contents().to_owned(),
                        style: CellStyle {
                            foreground: color(cell.fgcolor()),
                            background: color(cell.bgcolor()),
                            bold: cell.bold(),
                            dim: cell.dim(),
                            italic: cell.italic(),
                            underline: cell.underline(),
                            inverse: cell.inverse(),
                        },
                        wide: cell.is_wide(),
                        continuation: cell.is_wide_continuation(),
                    }
                })
            })
            .collect();
        Self {
            text: normalize_contents(&screen.contents()),
            rows,
            columns,
            cursor: screen.cursor_position(),
            cursor_visible: !screen.hide_cursor(),
            alternate_screen: screen.alternate_screen(),
            cells,
        }
    }

    pub fn contains(&self, text: &str) -> bool {
        self.text.contains(text)
    }

    pub fn cell(&self, row: u16, column: u16) -> Option<&StyledCell> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.cells
            .get(usize::from(row) * usize::from(self.columns) + usize::from(column))
    }

    /// Locates visible text and returns its terminal-cell region.
    pub fn find_text(&self, needle: &str) -> Option<TextRegion> {
        self.find_text_in_region(needle, 0, self.rows, 0, self.columns)
    }

    pub fn find_text_in_region(
        &self,
        needle: &str,
        row_start: u16,
        row_end: u16,
        column_start: u16,
        column_end: u16,
    ) -> Option<TextRegion> {
        if needle.is_empty() {
            return None;
        }
        for row in row_start..row_end.min(self.rows) {
            let mut visible = String::new();
            let mut byte_columns = Vec::new();
            for column in column_start..column_end.min(self.columns) {
                let cell = self.cell(row, column)?;
                if cell.continuation {
                    continue;
                }
                let contents = if cell.text.is_empty() {
                    " "
                } else {
                    &cell.text
                };
                byte_columns.extend(std::iter::repeat_n(column, contents.len()));
                visible.push_str(contents);
            }
            if let Some(start) = visible.find(needle) {
                let end_byte = start + needle.len() - 1;
                let start_column = byte_columns[start];
                let last_column = byte_columns[end_byte];
                let last_cell = self.cell(row, last_column)?;
                return Some(TextRegion {
                    row,
                    start_column,
                    end_column: last_column + u16::from(last_cell.wide) + 1,
                });
            }
        }
        None
    }

    pub fn style_at_text(&self, needle: &str) -> Option<CellStyle> {
        let region = self.find_text(needle)?;
        Some(self.cell(region.row, region.start_column)?.style)
    }

    pub fn region_has_style(&self, region: TextRegion, style: CellStyle) -> bool {
        (region.start_column..region.end_column).all(|column| {
            self.cell(region.row, column)
                .is_some_and(|cell| cell.continuation || cell.style == style)
        })
    }

    /// Stable runs for non-empty cells. Default-styled text is included so
    /// reset/bleed regressions remain visible to snapshots.
    pub fn style_runs(&self) -> Vec<StyleRun> {
        let mut runs = Vec::new();
        for row in 0..self.rows {
            let mut current: Option<StyleRun> = None;
            for column in 0..self.columns {
                let cell = self.cell(row, column).expect("cell inside frame bounds");
                if cell.continuation || (cell.text.is_empty() && cell.style == CellStyle::default())
                {
                    continue;
                }
                let text = if cell.text.is_empty() {
                    " ".to_owned()
                } else {
                    cell.text.clone()
                };
                let width = 1 + u16::from(cell.wide);
                match &mut current {
                    Some(run) if run.style == cell.style && run.end_column == column => {
                        run.end_column += width;
                        run.text.push_str(&text);
                    }
                    Some(run) => {
                        runs.push(run.clone());
                        *run = StyleRun {
                            row,
                            start_column: column,
                            end_column: column + width,
                            text,
                            style: cell.style,
                        };
                    }
                    None => {
                        current = Some(StyleRun {
                            row,
                            start_column: column,
                            end_column: column + width,
                            text,
                            style: cell.style,
                        });
                    }
                }
            }
            if let Some(run) = current {
                runs.push(run);
            }
        }
        runs
    }

    pub fn style_manifest(&self) -> String {
        self.style_runs()
            .into_iter()
            .map(|run| {
                format!(
                    "{}:{}-{} \"{}\" {:?}",
                    run.row,
                    run.start_column,
                    run.end_column,
                    run.text.escape_debug(),
                    run.style
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Stable style evidence for authored/non-default cells only. The visible
    /// grid already records default text, so omitting it here keeps reviewed
    /// snapshots compact while still exposing color or modifier bleed.
    #[allow(dead_code)]
    pub fn non_default_style_manifest(&self) -> String {
        self.style_runs()
            .into_iter()
            .filter(|run| run.style != CellStyle::default())
            .map(|run| {
                format!(
                    "{}:{}-{} \"{}\" {:?}",
                    run.row,
                    run.start_column,
                    run.end_column,
                    run.text.escape_debug(),
                    run.style
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
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

    #[allow(dead_code)]
    pub fn styled_snapshot_text(&self, sandbox_root: &Path) -> String {
        format!(
            "{}\n--- styles (non-default) ---\n{}",
            self.snapshot_text(sandbox_root),
            self.non_default_style_manifest()
        )
    }
}

fn color(value: vt100::Color) -> ScreenColor {
    match value {
        vt100::Color::Default => ScreenColor::Default,
        vt100::Color::Idx(index) => ScreenColor::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => ScreenColor::Rgb(red, green, blue),
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
            cells: vec![
                StyledCell {
                    text: String::new(),
                    style: CellStyle::default(),
                    wide: false,
                    continuation: false,
                };
                40 * 120
            ],
        };
        let snapshot = frame.snapshot_text(Path::new("/tmp/jtv-random"));
        assert!(snapshot.contains("<SANDBOX>/project"));
        assert!(snapshot.contains("jtv-session-should-remain"));
    }
}
