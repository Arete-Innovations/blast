use crate::configs::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
pub enum TuiMessage {
    NewLogLine(String),
    Quit,
}

pub struct TuiLogViewer {
    logs: Arc<Mutex<VecDeque<String>>>,
    filtered_logs: Vec<String>,
    search_input: String,
    search_mode: bool,
    scroll_offset: usize,
    list_state: ListState,
    expanded_entries: HashSet<usize>, // Track which entries are expanded
    matcher: SkimMatcherV2,
    log_path: PathBuf,
    receiver: Option<Receiver<TuiMessage>>,
}

impl TuiLogViewer {
    pub fn new(log_path: PathBuf) -> Self {
        Self {
            logs: Arc::new(Mutex::new(VecDeque::new())),
            filtered_logs: Vec::new(),
            search_input: String::new(),
            search_mode: false,
            scroll_offset: 0,
            list_state: ListState::default(),
            expanded_entries: HashSet::new(),
            matcher: SkimMatcherV2::default(),
            log_path,
            receiver: None,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Setup file watcher and log reader
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.setup_log_watcher(tx.clone())?;
        self.load_existing_logs()?;

        // Main event loop
        let result = self.run_event_loop(&mut terminal);

        // Cleanup
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
        terminal.show_cursor()?;

        result
    }

    fn setup_log_watcher(&self, tx: Sender<TuiMessage>) -> io::Result<()> {
        let log_path = self.log_path.clone();
        let logs_clone = Arc::clone(&self.logs);

        thread::spawn(move || {
            let mut last_size = 0u64;

            // Initial file size
            if let Ok(metadata) = fs::metadata(&log_path) {
                last_size = metadata.len();
            }

            loop {
                if let Ok(metadata) = fs::metadata(&log_path) {
                    let current_size = metadata.len();

                    if current_size > last_size {
                        // Read new content
                        if let Ok(file) = fs::File::open(&log_path) {
                            let mut reader = BufReader::new(file);
                            if reader.seek(SeekFrom::Start(last_size)).is_ok() {
                                for line in reader.lines() {
                                    if let Ok(line) = line {
                                        if !line.trim().is_empty() && !line.starts_with("---") {
                                            // Add to logs buffer
                                            if let Ok(mut logs) = logs_clone.lock() {
                                                logs.push_back(line.clone());
                                                // Keep only last 1000 lines to prevent memory issues
                                                if logs.len() > 1000 {
                                                    logs.pop_front();
                                                }
                                            }

                                            // Send to UI
                                            if tx.send(TuiMessage::NewLogLine(line)).is_err() {
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        last_size = current_size;
                    }
                }

                thread::sleep(Duration::from_millis(100));
            }
        });

        Ok(())
    }

    fn load_existing_logs(&mut self) -> io::Result<()> {
        if let Ok(content) = fs::read_to_string(&self.log_path) {
            let mut logs = self.logs.lock().unwrap();
            for line in content.lines() {
                if !line.trim().is_empty() && !line.starts_with("---") {
                    logs.push_back(line.to_string());
                }
            }
            // Keep only last 1000 lines
            while logs.len() > 1000 {
                logs.pop_front();
            }
        }
        self.update_filtered_logs();
        // Initialize scroll position at bottom for tail behavior
        self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
        self.list_state.select(Some(self.scroll_offset));
        Ok(())
    }

    fn run_event_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            // Check for new log messages
            let mut should_update = false;
            if let Some(ref receiver) = self.receiver {
                while let Ok(msg) = receiver.try_recv() {
                    match msg {
                        TuiMessage::NewLogLine(_) => {
                            should_update = true;
                        }
                        TuiMessage::Quit => return Ok(()),
                    }
                }
            }

            if should_update {
                let old_len = self.filtered_logs.len();
                self.update_filtered_logs();

                // Only auto-scroll to bottom if user was already at the bottom (or close to it)
                // and not searching
                if self.search_input.is_empty() {
                    let was_at_bottom = self.scroll_offset >= old_len.saturating_sub(2);
                    if was_at_bottom {
                        self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
                        self.list_state.select(Some(self.scroll_offset));
                    }
                }
            }

            terminal.draw(|f| self.ui(f))?;

            // Handle input events
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('/') if !self.search_mode => {
                                self.search_mode = true;
                                self.search_input.clear();
                            }
                            KeyCode::Esc => {
                                if self.search_mode {
                                    // Exit search mode
                                    self.search_mode = false;
                                    self.search_input.clear();
                                    self.expanded_entries.clear();
                                    self.update_filtered_logs();
                                    // Force terminal clear to fix trailing character issue
                                    let _ = terminal.clear();
                                } else if !self.search_input.is_empty() {
                                    // Clear active search filter
                                    self.search_input.clear();
                                    self.expanded_entries.clear();
                                    self.update_filtered_logs();
                                    // Force terminal clear to fix trailing character issue
                                    let _ = terminal.clear();
                                } else if !self.expanded_entries.is_empty() {
                                    // First Esc: collapse any expanded entries
                                    self.expanded_entries.clear();
                                    // Force terminal clear to fix trailing character issue
                                    let _ = terminal.clear();
                                } else {
                                    // Second Esc: move to last entry and resume auto-following
                                    self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
                                    self.list_state.select(Some(self.scroll_offset));
                                }
                            }
                            KeyCode::Enter if self.search_mode => {
                                self.search_mode = false;
                                // When confirming search, expand all entries to show traces
                                if !self.search_input.is_empty() {
                                    for i in 0..self.filtered_logs.len() {
                                        self.expanded_entries.insert(i);
                                    }
                                }
                                self.update_filtered_logs();
                            }
                            KeyCode::Enter if !self.search_mode => {
                                // Toggle expansion of current entry (only if it has expandable content)
                                if let Some(selected) = self.list_state.selected() {
                                    if selected < self.filtered_logs.len() {
                                        let line = &self.filtered_logs[selected];
                                        if self.has_expandable_content(line) {
                                            if self.expanded_entries.contains(&selected) {
                                                self.expanded_entries.remove(&selected);
                                            } else {
                                                self.expanded_entries.insert(selected);
                                            }
                                            // Force terminal clear to fix trailing character issue
                                            let _ = terminal.clear();
                                        }
                                    }
                                }
                            }
                            KeyCode::Char(c) if self.search_mode => {
                                self.search_input.push(c);
                                self.update_filtered_logs();
                                // Auto-tail to bottom when filtering (search mode always goes to bottom)
                                self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::Backspace if self.search_mode => {
                                self.search_input.pop();
                                // Clear expanded entries if search becomes empty
                                if self.search_input.is_empty() {
                                    self.expanded_entries.clear();
                                }
                                self.update_filtered_logs();
                                // Auto-tail to bottom when filtering (search mode always goes to bottom)
                                self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::Up => {
                                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::Down => {
                                if self.scroll_offset < self.filtered_logs.len().saturating_sub(1) {
                                    self.scroll_offset += 1;
                                }
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::PageUp => {
                                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::PageDown => {
                                self.scroll_offset = (self.scroll_offset + 10).min(self.filtered_logs.len().saturating_sub(1));
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::Home => {
                                self.scroll_offset = 0;
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            KeyCode::End => {
                                self.scroll_offset = self.filtered_logs.len().saturating_sub(1);
                                self.list_state.select(Some(self.scroll_offset));
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn update_filtered_logs(&mut self) {
        let logs = self.logs.lock().unwrap();

        if self.search_input.is_empty() {
            self.filtered_logs = logs.iter().cloned().collect();
        } else {
            self.filtered_logs = logs.iter().filter(|line| self.matcher.fuzzy_match(line, &self.search_input).is_some()).cloned().collect();
        }
    }

    fn ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Min(0), Constraint::Length(2)]).split(f.area());

        // Main log display
        let logs_to_display: Vec<ListItem> = self
            .filtered_logs
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let should_expand = self.expanded_entries.contains(&i) || !self.search_input.is_empty();
                let is_selected = self.list_state.selected() == Some(i);
                let formatted_line = if should_expand {
                    self.format_log_line_expanded(line, is_selected)
                } else {
                    self.format_log_line_single(line, is_selected)
                };
                // Same color for all entries - selection will be handled by our custom indicator
                ListItem::new(formatted_line)
            })
            .collect();

        let logs_list = List::new(logs_to_display);

        f.render_stateful_widget(logs_list, chunks[0], &mut self.list_state);

        // Bottom row - split horizontally between search input and status info
        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[1]);

        // Left side - Search input
        let search_text = if self.search_mode {
            format!("/{}", self.search_input)
        } else {
            "'/' to search, Enter to toggle trace, Esc to collapse all, ↑↓ to scroll".to_string()
        };

        let input = Paragraph::new(search_text)
            .style(if self.search_mode { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) })
            .block(Block::default().borders(Borders::TOP));

        f.render_widget(input, bottom_chunks[0]);

        // Right side - Status info
        let selected_entry = self.list_state.selected().unwrap_or(0) + 1;
        let total_entries = self.filtered_logs.len();
        let status_text = if !self.search_input.is_empty() {
            format!(
                "Logs: {}/{} entries (filtered by '{}') - Selected: {}/{}",
                total_entries,
                self.logs.lock().unwrap().len(),
                self.search_input,
                selected_entry,
                total_entries
            )
        } else {
            format!("Logs: {} entries - Selected: {}/{}", total_entries, selected_entry, total_entries)
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Right)
            .block(Block::default().borders(Borders::TOP));

        f.render_widget(status, bottom_chunks[1]);

        // Set cursor position when in search mode
        if self.search_mode {
            f.set_cursor_position((bottom_chunks[0].x + self.search_input.len() as u16 + 1, bottom_chunks[0].y + 1));
        }
    }

    fn has_expandable_content(&self, line: &str) -> bool {
        // Check if the line has trace data (→ appears more than once)
        line.matches(" → ").count() >= 3
    }

    fn extract_log_level(&self, line: &str) -> Option<String> {
        // Extract log level from format: "timestamp [LEVEL] [file:loc]"
        if let Some(first_bracket) = line.find(" [") {
            if let Some(first_bracket_end) = line[first_bracket + 2..].find(']') {
                return Some(line[first_bracket + 2..first_bracket + 2 + first_bracket_end].to_string());
            }
        }
        None
    }

    fn get_level_icon_and_color(&self, level: &str) -> (&'static str, Color, Color) {
        match level.to_uppercase().as_str() {
            "INFO" => ("ℹ", Color::White, Color::Rgb(230, 230, 230)), // White, light gray
            "WARNING" => ("⚠", Color::Rgb(255, 165, 0), Color::Rgb(255, 200, 100)), // Orange, light orange
            "ERROR" => ("🔥", Color::Red, Color::Rgb(255, 100, 100)), // Red, light red
            "DEBUG" => ("🔍", Color::Blue, Color::Rgb(100, 150, 255)), // Blue, light blue
            "TRACE" => ("🔬", Color::Magenta, Color::Rgb(255, 150, 255)), // Magenta, light magenta
            _ => ("📍", Color::Blue, Color::Rgb(100, 150, 255)), // Default fallback
        }
    }

    fn extract_timestamp(&self, line: &str) -> Option<String> {
        // Extract timestamp from format: "1748751467-2025-06-01 07:17:47 [WARNING] [auth.rs:129::auth]"
        if let Some(first_bracket) = line.find(" [") {
            let timestamp_part = &line[..first_bracket];
            // Split by space to get the human-readable part
            if let Some(space_pos) = timestamp_part.find(' ') {
                return Some(timestamp_part[space_pos + 1..].to_string());
            }
        }
        None
    }

    fn format_log_line_single(&self, line: &str, is_selected: bool) -> Vec<Line<'static>> {
        // Single line format - extract main message only
        if let Some(second_bracket_start) = line.find("] [") {
            if let Some(second_bracket_end) = line[second_bracket_start + 3..].find(']') {
                let file_location = &line[second_bracket_start + 3..second_bracket_start + 3 + second_bracket_end];
                let rest = &line[second_bracket_start + 3 + second_bracket_end + 1..].trim();

                let parts: Vec<&str> = rest.split(" → ").collect();
                let message = parts[0];

                let has_expandable = self.has_expandable_content(line);
                let selection_indicator = if is_selected {
                    if has_expandable {
                        "► "
                    } else {
                        "• "
                    }
                } else {
                    if has_expandable {
                        "▷ "
                    } else {
                        "  "
                    }
                };
                let timestamp = self.extract_timestamp(line).unwrap_or_else(|| "".to_string());
                let level = self.extract_log_level(line).unwrap_or_else(|| "INFO".to_string());
                let (level_icon, level_color, file_color) = self.get_level_icon_and_color(&level);

                let selection_bg = if is_selected { Color::DarkGray } else { Color::Reset };

                let mut spans = vec![Span::styled(selection_indicator.to_string(), Style::default().fg(Color::Yellow).bg(selection_bg).add_modifier(Modifier::BOLD))];

                if !timestamp.is_empty() {
                    let timestamp_color = if is_selected { Color::White } else { Color::DarkGray };
                    spans.push(Span::styled(format!("{} ", timestamp), Style::default().fg(timestamp_color).bg(selection_bg)));
                }

                spans.extend(vec![
                    Span::styled(format!("{}[", level_icon), Style::default().fg(level_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                    Span::styled(file_location.to_string(), Style::default().fg(file_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                    Span::styled("] ".to_string(), Style::default().fg(level_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                    Span::styled(message.to_string(), Style::default().fg(Color::White).bg(selection_bg)),
                ]);

                return vec![Line::from(spans)];
            }
        }

        // Fallback - format as much as we can for single line
        let has_expandable = self.has_expandable_content(line);
        let selection_indicator = if is_selected {
            if has_expandable {
                "► "
            } else {
                "• "
            }
        } else {
            if has_expandable {
                "▷ "
            } else {
                "  "
            }
        };
        let timestamp = self.extract_timestamp(line).unwrap_or_else(|| "".to_string());
        let level = self.extract_log_level(line).unwrap_or_else(|| "INFO".to_string());
        let (level_icon, _level_color, _file_color) = self.get_level_icon_and_color(&level);

        let selection_bg = if is_selected { Color::DarkGray } else { Color::Reset };

        let mut spans = vec![Span::styled(selection_indicator.to_string(), Style::default().fg(Color::Yellow).bg(selection_bg).add_modifier(Modifier::BOLD))];

        if !timestamp.is_empty() {
            let timestamp_color = if is_selected { Color::White } else { Color::DarkGray };
            spans.push(Span::styled(format!("{} ", timestamp), Style::default().fg(timestamp_color).bg(selection_bg)));
        }

        spans.push(Span::styled(format!("{} {}", level_icon, line), Style::default().fg(Color::White).bg(selection_bg)));
        vec![Line::from(spans)]
    }

    fn format_log_line_expanded(&self, line: &str, is_selected: bool) -> Vec<Line<'static>> {
        // Parse the log line format and create formatted spans
        if let Some(second_bracket_start) = line.find("] [") {
            if let Some(second_bracket_end) = line[second_bracket_start + 3..].find(']') {
                let file_location = &line[second_bracket_start + 3..second_bracket_start + 3 + second_bracket_end];
                let rest = &line[second_bracket_start + 3 + second_bracket_end + 1..].trim();

                let parts: Vec<&str> = rest.split(" → ").collect();

                if parts.len() >= 3 {
                    let message = parts[0];
                    let context_timing = parts[1];
                    let trace_items = &parts[2..];

                    let mut lines = Vec::new();

                    // Main message line with selection indicator and timestamp
                    let has_expandable = self.has_expandable_content(line);
                    let selection_indicator = if is_selected {
                        if has_expandable {
                            "► "
                        } else {
                            "• "
                        }
                    } else {
                        if has_expandable {
                            "▷ "
                        } else {
                            "  "
                        }
                    };
                    let timestamp = self.extract_timestamp(line).unwrap_or_else(|| "".to_string());
                    let level = self.extract_log_level(line).unwrap_or_else(|| "INFO".to_string());
                    let (level_icon, level_color, file_color) = self.get_level_icon_and_color(&level);

                    let selection_bg = if is_selected { Color::DarkGray } else { Color::Reset };

                    let mut main_spans = vec![Span::styled(selection_indicator.to_string(), Style::default().fg(Color::Yellow).bg(selection_bg).add_modifier(Modifier::BOLD))];

                    if !timestamp.is_empty() {
                        let timestamp_color = if is_selected { Color::White } else { Color::DarkGray };
                        main_spans.push(Span::styled(format!("{} ", timestamp), Style::default().fg(timestamp_color).bg(selection_bg)));
                    }

                    main_spans.extend(vec![
                        Span::styled(format!("{}[", level_icon), Style::default().fg(level_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                        Span::styled(file_location.to_string(), Style::default().fg(file_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                        Span::styled("] ".to_string(), Style::default().fg(level_color).bg(selection_bg).add_modifier(Modifier::BOLD)),
                        Span::styled(message.to_string(), Style::default().fg(Color::White).bg(selection_bg)),
                    ]);

                    lines.push(Line::from(main_spans));

                    // Context line - use brighter color for line drawing when selected
                    let line_color = if is_selected { Color::Gray } else { Color::DarkGray };
                    lines.push(Line::from(vec![
                        Span::styled("  ┗┳╾ ".to_string(), Style::default().fg(line_color).bg(selection_bg)),
                        Span::styled(context_timing.to_string(), Style::default().fg(Color::Cyan).bg(selection_bg)),
                    ]));

                    // Trace items - use brighter color for line drawing when selected
                    for (i, trace_item) in trace_items.iter().enumerate() {
                        let indent = " ".repeat(i + 1);
                        let connector = if i == trace_items.len() - 1 { "┗━╾" } else { "┗┳╾" };
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {}{} ", indent, connector), Style::default().fg(line_color).bg(selection_bg)),
                            Span::styled(trace_item.to_string(), Style::default().fg(Color::Yellow).bg(selection_bg)),
                        ]));
                    }

                    return lines;
                }
            }
        }

        // Fallback - format as much as we can for expanded line (same as single since no trace data)
        let has_expandable = self.has_expandable_content(line);
        let selection_indicator = if is_selected {
            if has_expandable {
                "► "
            } else {
                "• "
            }
        } else {
            if has_expandable {
                "▷ "
            } else {
                "  "
            }
        };
        let timestamp = self.extract_timestamp(line).unwrap_or_else(|| "".to_string());
        let level = self.extract_log_level(line).unwrap_or_else(|| "INFO".to_string());
        let (level_icon, _level_color, _file_color) = self.get_level_icon_and_color(&level);

        let selection_bg = if is_selected { Color::DarkGray } else { Color::Reset };

        let mut spans = vec![Span::styled(selection_indicator.to_string(), Style::default().fg(Color::Yellow).bg(selection_bg).add_modifier(Modifier::BOLD))];

        if !timestamp.is_empty() {
            let timestamp_color = if is_selected { Color::White } else { Color::DarkGray };
            spans.push(Span::styled(format!("{} ", timestamp), Style::default().fg(timestamp_color).bg(selection_bg)));
        }

        spans.push(Span::styled(format!("{} {}", level_icon, line), Style::default().fg(Color::White).bg(selection_bg)));
        vec![Line::from(spans)]
    }
}

pub fn run_tui_log_viewer(level: &str, config: &Config) -> io::Result<()> {
    let logs_dir = config.project_dir.join("storage").join("logs");
    let log_file = format!("{}.log", level.to_lowercase());
    let log_path = logs_dir.join(&log_file);

    if !log_path.exists() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("Log file not found: {}", log_file)));
    }

    let mut viewer = TuiLogViewer::new(log_path);
    viewer.run()
}

