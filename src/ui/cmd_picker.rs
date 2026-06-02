use std::io::Write;

use crossterm::ExecutableCommand;
use crossterm::cursor::MoveTo;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::terminal::Clear;

use super::utils::resolve_color;

const COMMANDS: &[&str] = &[
    "/add",
    "/drop",
    "/drop-all",
    "/init",
    "/memory",
    "/model",
    "/models",
    "/models-add",
    "/provider",
    "/sessions",
    "/reasoning",
    "/thinking",
    "/mode",
    "/mcp",
    "/toggle",
    "/compress",
    "/compact",
    "/loop",
    "/prompt",
    "/theme",
    "/history",
    "/regen-prompts",
    "/regen-themes",
    "/editsys",
    "/quit",
    "/exit",
    "/clear",
    "/new",
    "/undo",
    "/retry",
    "/help",
    "/welcome",
    "/tutorial",
    "/worktree",
    "/wt-merge",
    "/wt-exit",
    "/btw",
    "/queue",
];

pub struct CommandPicker {
    pub active: bool,
    pub query: String,
    pub cursor: usize,
    pub matches: Vec<&'static str>,
    pub selected: usize,
    monochrome: bool,
}

impl CommandPicker {
    pub fn new() -> Self {
        CommandPicker {
            active: false,
            query: String::new(),
            cursor: 0,
            matches: Vec::new(),
            selected: 0,
            monochrome: false,
        }
    }

    pub fn set_monochrome(&mut self, monochrome: bool) {
        self.monochrome = monochrome;
    }

    fn color(&self, color: Color) -> Color {
        resolve_color(color, self.monochrome)
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.matches.clear();
        self.selected = 0;
        self.filter();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn char_input(&mut self, c: char) {
        let byte_pos = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert(byte_pos, c);
        self.cursor += 1;
        self.filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 && !self.query.is_empty() {
            self.cursor -= 1;
            let byte_pos = self
                .query
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.query.len());
            self.query.remove(byte_pos);
            self.filter();
        }
    }

    fn filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.matches = COMMANDS
            .iter()
            .filter(|cmd| {
                let lower = cmd.to_lowercase();
                lower.contains(&query_lower)
            })
            .take(50)
            .copied()
            .collect();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            self.selected = if self.selected == 0 {
                self.matches.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_command(&self) -> Option<&'static str> {
        self.matches.get(self.selected).copied()
    }

    /// For testing: set matches directly.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn test_set_matches(&mut self, items: Vec<&'static str>) {
        self.matches = items;
        self.selected = 0;
    }

    pub fn draw(&self) -> std::io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let (cols, rows) = crossterm::terminal::size()?;
        let mut stdout = std::io::stdout();

        let max_items = (rows.saturating_sub(4)).min(10) as usize;

        if self.matches.is_empty() {
            let r = rows.saturating_sub(3);
            stdout.execute(MoveTo(0, r))?;
            write!(
                stdout,
                "{}",
                SetForegroundColor(self.color(Color::DarkGrey))
            )?;
            write!(stdout, "no matches")?;
            write!(stdout, "{}", ResetColor)?;
            stdout.flush()?;
            return Ok(());
        }

        let list_height = max_items.min(self.matches.len());
        let start_idx = self
            .selected
            .saturating_sub(list_height / 2)
            .min(self.matches.len().saturating_sub(list_height));
        let end_idx = (start_idx + list_height).min(self.matches.len());

        let top_row = rows.saturating_sub(3).saturating_sub(list_height as u16);

        for i in start_idx..end_idx {
            let render_row = top_row + (i - start_idx) as u16;
            stdout.execute(MoveTo(0, render_row))?;
            write!(
                stdout,
                "{}",
                Clear(crossterm::terminal::ClearType::CurrentLine)
            )?;

            let truncated: String = self.matches[i]
                .chars()
                .take(cols.saturating_sub(3) as usize)
                .collect();

            if i == self.selected {
                write!(stdout, "{}", SetForegroundColor(self.color(Color::Green)))?;
                write!(stdout, "▸ {}", truncated)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    SetForegroundColor(self.color(Color::DarkGrey))
                )?;
                write!(stdout, "  {}", truncated)?;
            }
            write!(stdout, "{}", ResetColor)?;
        }
        stdout.flush()?;
        Ok(())
    }
}

pub struct PromptPicker {
    pub prefix: &'static str,
    pub active: bool,
    pub query: String,
    pub cursor: usize,
    pub matches: Vec<String>,
    pub selected: usize,
    items: Vec<String>,
    monochrome: bool,
}

impl PromptPicker {
    pub fn new() -> Self {
        PromptPicker {
            prefix: "/prompt ",
            active: false,
            query: String::new(),
            cursor: 0,
            matches: Vec::new(),
            selected: 0,
            items: Vec::new(),
            monochrome: false,
        }
    }

    pub fn with_prefix(prefix: &'static str) -> Self {
        PromptPicker {
            prefix,
            active: false,
            query: String::new(),
            cursor: 0,
            matches: Vec::new(),
            selected: 0,
            items: Vec::new(),
            monochrome: false,
        }
    }

    pub fn set_monochrome(&mut self, monochrome: bool) {
        self.monochrome = monochrome;
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
    }

    fn color(&self, color: Color) -> Color {
        resolve_color(color, self.monochrome)
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.matches.clear();
        self.selected = 0;
        self.filter();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn char_input(&mut self, c: char) {
        let byte_pos = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert(byte_pos, c);
        self.cursor += 1;
        self.filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 && !self.query.is_empty() {
            self.cursor -= 1;
            let byte_pos = self
                .query
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.query.len());
            self.query.remove(byte_pos);
            self.filter();
        }
    }

    fn filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.matches = self
            .items
            .iter()
            .filter(|name| name.to_lowercase().contains(&query_lower))
            .take(50)
            .cloned()
            .collect();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            self.selected = if self.selected == 0 {
                self.matches.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.matches.get(self.selected).map(|s| s.as_str())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn test_set_items(&mut self, items: Vec<String>) {
        self.items = items;
    }

    pub fn draw(&self) -> std::io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let (cols, rows) = crossterm::terminal::size()?;
        let mut stdout = std::io::stdout();

        let max_items = (rows.saturating_sub(4)).min(10) as usize;

        if self.matches.is_empty() {
            let r = rows.saturating_sub(3);
            stdout.execute(MoveTo(0, r))?;
            write!(
                stdout,
                "{}",
                SetForegroundColor(self.color(Color::DarkGrey))
            )?;
            // The provider list is not exhaustive: built-ins plus configured custom
            // gateways are shown, but a name typed here is still submitted as-is, so
            // remind the user they can free-type a registered gateway name.
            if self.prefix == "/provider " {
                write!(stdout, "no matches  (type a registered custom gateway name)")?;
            } else {
                write!(stdout, "no matches")?;
            }
            write!(stdout, "{}", ResetColor)?;
            stdout.flush()?;
            return Ok(());
        }

        let list_height = max_items.min(self.matches.len());
        let start_idx = self
            .selected
            .saturating_sub(list_height / 2)
            .min(self.matches.len().saturating_sub(list_height));
        let end_idx = (start_idx + list_height).min(self.matches.len());

        let top_row = rows.saturating_sub(3).saturating_sub(list_height as u16);

        for i in start_idx..end_idx {
            let render_row = top_row + (i - start_idx) as u16;
            stdout.execute(MoveTo(0, render_row))?;
            write!(
                stdout,
                "{}",
                Clear(crossterm::terminal::ClearType::CurrentLine)
            )?;

            let truncated: String = self.matches[i]
                .chars()
                .take(cols.saturating_sub(3) as usize)
                .collect();

            if i == self.selected {
                write!(stdout, "{}", SetForegroundColor(self.color(Color::Green)))?;
                write!(stdout, "▸ {}", truncated)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    SetForegroundColor(self.color(Color::DarkGrey))
                )?;
                write!(stdout, "  {}", truncated)?;
            }
            write!(stdout, "{}", ResetColor)?;
        }
        stdout.flush()?;
        Ok(())
    }
}

fn fuzzy_score(item: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let item_l = item.to_lowercase();
    let query_l = query.to_lowercase();
    let is_boundary = |bytes: &[u8], pos: usize| -> bool {
        pos == 0 || matches!(bytes.get(pos - 1), Some(b'-' | b'.' | b'/' | b'_' | b' ' | b':'))
    };

    // Tier 1: contiguous substring wins decisively over any scattered subsequence
    // (so "gemini" ranks google/gemini-* above openai/gpt-…-mini).
    if let Some(pos) = item_l.find(&query_l) {
        let mut score = 1000;
        if is_boundary(item_l.as_bytes(), pos) {
            score += 200;
        }
        if pos == 0 {
            score += 100;
        }
        score -= pos as i32; // earlier match better
        score -= (item_l.chars().count() / 4) as i32; // shorter id better
        return Some(score);
    }

    // Tier 2: scattered subsequence fallback (low band, never beats a substring)
    let chars: Vec<char> = item_l.chars().collect();
    let mut score = 0i32;
    let mut idx = 0usize;
    let mut last: Option<usize> = None;
    for qc in query_l.chars() {
        let mut pos = None;
        while idx < chars.len() {
            if chars[idx] == qc {
                pos = Some(idx);
                break;
            }
            idx += 1;
        }
        let pos = pos?;
        if last == Some(pos.wrapping_sub(1)) {
            score += 5;
        }
        if pos == 0 || matches!(chars.get(pos - 1), Some('-' | '.' | '/' | '_' | ' ' | ':')) {
            score += 3;
        }
        last = Some(pos);
        idx = pos + 1;
    }
    score -= (chars.len() / 20) as i32;
    Some(score)
}

pub struct ModelsPicker {
    pub active: bool,
    pub query: String,
    pub cursor: usize,
    pub matches: Vec<String>,
    pub selected: usize,
    quick: Vec<String>,
    provider: Vec<String>,
    group: usize,
    monochrome: bool,
}

impl ModelsPicker {
    pub fn new() -> Self {
        ModelsPicker {
            active: false,
            query: String::new(),
            cursor: 0,
            matches: Vec::new(),
            selected: 0,
            quick: Vec::new(),
            provider: Vec::new(),
            group: 0,
            monochrome: false,
        }
    }

    pub fn set_monochrome(&mut self, monochrome: bool) {
        self.monochrome = monochrome;
    }

    pub fn set_groups(&mut self, quick: Vec<String>, provider: Vec<String>) {
        self.quick = quick;
        self.provider = provider;
    }

    fn color(&self, color: Color) -> Color {
        resolve_color(color, self.monochrome)
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.matches.clear();
        self.selected = 0;
        // start on Provider if there are no quick presets to show
        self.group = if self.quick.is_empty() && !self.provider.is_empty() {
            1
        } else {
            0
        };
        self.filter();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn toggle_group(&mut self) {
        self.group = 1 - self.group;
        self.selected = 0;
        self.filter();
    }

    pub fn char_input(&mut self, c: char) {
        let byte_pos = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert(byte_pos, c);
        self.cursor += 1;
        self.filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 && !self.query.is_empty() {
            self.cursor -= 1;
            let byte_pos = self
                .query
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.query.len());
            self.query.remove(byte_pos);
            self.filter();
        }
    }

    fn filter(&mut self) {
        let src = if self.group == 0 {
            &self.quick
        } else {
            &self.provider
        };
        let mut scored: Vec<(i32, &String)> = src
            .iter()
            .filter_map(|n| fuzzy_score(n, &self.query).map(|s| (s, n)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        self.matches = scored.into_iter().take(50).map(|(_, n)| n.clone()).collect();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            self.selected = if self.selected == 0 {
                self.matches.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.matches.get(self.selected).map(|s| s.as_str())
    }

    pub fn draw(&self) -> std::io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let (cols, rows) = crossterm::terminal::size()?;
        let mut stdout = std::io::stdout();

        let max_items = (rows.saturating_sub(5)).min(10) as usize;
        let list_height = max_items.min(self.matches.len().max(1));
        let top_row = rows.saturating_sub(3).saturating_sub(list_height as u16);

        // tab header showing the two groups (skip on very short terminals)
        if rows >= 8 {
            let header_row = top_row.saturating_sub(1);
            stdout.execute(MoveTo(0, header_row))?;
            write!(
                stdout,
                "{}",
                Clear(crossterm::terminal::ClearType::CurrentLine)
            )?;
            let tab = |label: &str, count: usize, active: bool| {
                if active {
                    format!("[{} {}]", label, count)
                } else {
                    format!(" {} {} ", label, count)
                }
            };
            write!(stdout, "{}", SetForegroundColor(self.color(Color::DarkGrey)))?;
            write!(
                stdout,
                "{}  {}   (Tab to switch)",
                tab("Quick", self.quick.len(), self.group == 0),
                tab("Provider", self.provider.len(), self.group == 1)
            )?;
            write!(stdout, "{}", ResetColor)?;
        }

        if self.matches.is_empty() {
            let r = rows.saturating_sub(3);
            stdout.execute(MoveTo(0, r))?;
            write!(
                stdout,
                "{}",
                SetForegroundColor(self.color(Color::DarkGrey))
            )?;
            write!(stdout, "no matches")?;
            write!(stdout, "{}", ResetColor)?;
            stdout.flush()?;
            return Ok(());
        }

        let start_idx = self
            .selected
            .saturating_sub(list_height / 2)
            .min(self.matches.len().saturating_sub(list_height));
        let end_idx = (start_idx + list_height).min(self.matches.len());

        for i in start_idx..end_idx {
            let render_row = top_row + (i - start_idx) as u16;
            stdout.execute(MoveTo(0, render_row))?;
            write!(
                stdout,
                "{}",
                Clear(crossterm::terminal::ClearType::CurrentLine)
            )?;

            let truncated: String = self.matches[i]
                .chars()
                .take(cols.saturating_sub(3) as usize)
                .collect();

            if i == self.selected {
                write!(stdout, "{}", SetForegroundColor(self.color(Color::Green)))?;
                write!(stdout, "▸ {}", truncated)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    SetForegroundColor(self.color(Color::DarkGrey))
                )?;
                write!(stdout, "  {}", truncated)?;
            }
            write!(stdout, "{}", ResetColor)?;
        }
        stdout.flush()?;
        Ok(())
    }
}

pub struct ThemePicker {
    pub active: bool,
    pub query: String,
    pub cursor: usize,
    pub matches: Vec<String>,
    pub selected: usize,
    items: Vec<String>,
    monochrome: bool,
}

impl ThemePicker {
    pub fn new() -> Self {
        ThemePicker {
            active: false,
            query: String::new(),
            cursor: 0,
            matches: Vec::new(),
            selected: 0,
            items: Vec::new(),
            monochrome: false,
        }
    }

    pub fn set_monochrome(&mut self, monochrome: bool) {
        self.monochrome = monochrome;
    }

    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
    }

    fn color(&self, color: Color) -> Color {
        resolve_color(color, self.monochrome)
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.query.clear();
        self.cursor = 0;
        self.matches.clear();
        self.selected = 0;
        self.filter();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn char_input(&mut self, c: char) {
        let byte_pos = self
            .query
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());
        self.query.insert(byte_pos, c);
        self.cursor += 1;
        self.filter();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 && !self.query.is_empty() {
            self.cursor -= 1;
            let byte_pos = self
                .query
                .char_indices()
                .nth(self.cursor)
                .map(|(i, _)| i)
                .unwrap_or(self.query.len());
            self.query.remove(byte_pos);
            self.filter();
        }
    }

    fn filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.matches = self
            .items
            .iter()
            .filter(|name| name.to_lowercase().contains(&query_lower))
            .take(50)
            .cloned()
            .collect();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected = (self.selected + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            self.selected = if self.selected == 0 {
                self.matches.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.matches.get(self.selected).map(|s| s.as_str())
    }

    pub fn draw(&self) -> std::io::Result<()> {
        if !self.active {
            return Ok(());
        }
        let (cols, rows) = crossterm::terminal::size()?;
        let mut stdout = std::io::stdout();

        let max_items = (rows.saturating_sub(4)).min(10) as usize;

        if self.matches.is_empty() {
            let r = rows.saturating_sub(3);
            stdout.execute(MoveTo(0, r))?;
            write!(
                stdout,
                "{}",
                SetForegroundColor(self.color(Color::DarkGrey))
            )?;
            write!(stdout, "no matches")?;
            write!(stdout, "{}", ResetColor)?;
            stdout.flush()?;
            return Ok(());
        }

        let list_height = max_items.min(self.matches.len());
        let start_idx = self
            .selected
            .saturating_sub(list_height / 2)
            .min(self.matches.len().saturating_sub(list_height));
        let end_idx = (start_idx + list_height).min(self.matches.len());

        let top_row = rows.saturating_sub(3).saturating_sub(list_height as u16);

        for i in start_idx..end_idx {
            let render_row = top_row + (i - start_idx) as u16;
            stdout.execute(MoveTo(0, render_row))?;
            write!(
                stdout,
                "{}",
                Clear(crossterm::terminal::ClearType::CurrentLine)
            )?;

            let truncated: String = self.matches[i]
                .chars()
                .take(cols.saturating_sub(3) as usize)
                .collect();

            if i == self.selected {
                write!(stdout, "{}", SetForegroundColor(self.color(Color::Green)))?;
                write!(stdout, "▸ {}", truncated)?;
            } else {
                write!(
                    stdout,
                    "{}",
                    SetForegroundColor(self.color(Color::DarkGrey))
                )?;
                write!(stdout, "  {}", truncated)?;
            }
            write!(stdout, "{}", ResetColor)?;
        }
        stdout.flush()?;
        Ok(())
    }
}
