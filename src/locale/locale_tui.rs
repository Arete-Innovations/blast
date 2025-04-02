use crate::configs::Config;
use crate::locale::locale_helpers;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, read_to_string};
use std::io::{stdout, Write};
use std::path::Path;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{clear, color, cursor, style};

// Different views in the locale manager
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    LanguageList,
    SectionList,
    KeyDetail,
    EditValue,
    ConfirmSave,
}

// Node in the JSON tree display
#[derive(Debug, Clone)]
pub struct JsonTreeNode {
    pub key: String,       // Key name
    pub path: String,      // Full path to this node
    pub value: String,     // Value if this is a leaf node
    pub is_leaf: bool,     // If true, this is a value node
    pub is_expanded: bool, // For non-leaf nodes, if children are visible
    pub depth: usize,      // Indentation level
}

// Main application structure
pub struct LocaleManagerApp {
    languages: Vec<String>,                                  // Available language codes
    selected_language: usize,                                // Index of currently selected language
    current_view: View,                                      // Current view being displayed
    json_tree: HashMap<String, Vec<JsonTreeNode>>,           // Tree structure for language details view
    selected_node: Option<(String, usize)>,                  // Currently selected node (section name, index)
    message: Option<(String, bool)>,                         // Optional message to display (text, is_error)
    run: Option<termion::raw::RawTerminal<std::io::Stdout>>, // Terminal in raw mode
}

impl LocaleManagerApp {
    // Create a new LocaleManagerApp
    pub fn new() -> Self {
        // Get list of language files
        let languages = locale_helpers::get_language_files();

        // Default view and selections
        let view = if languages.is_empty() { View::LanguageList } else { View::LanguageList };

        // Initialize the JSON tree (will be populated when a language is selected)
        let json_tree = HashMap::new();

        Self {
            languages,
            selected_language: 0,
            current_view: view,
            json_tree,
            selected_node: None,
            message: None,
            run: None,
        }
    }

    // Run the application
    pub fn run(&mut self) {
        // Initialize terminal
        let stdout = stdout();
        let mut stdout = stdout.into_raw_mode().unwrap();
        write!(stdout, "{}{}", clear::All, cursor::Hide).unwrap();
        stdout.flush().unwrap();

        self.run = Some(stdout);

        // If no languages are found, show a message
        if self.languages.is_empty() {
            self.message = Some(("No language files found. Press 'n' to create a new language.".to_string(), true));
        }

        // Main event loop
        self.event_loop();

        // Cleanup terminal on exit
        if let Some(mut stdout) = self.run.take() {
            write!(stdout, "{}{}", clear::All, cursor::Show).unwrap();
            stdout.flush().unwrap();
        }
    }

    // Handle user input events
    fn event_loop(&mut self) {
        let stdin = std::io::stdin();
        let events = stdin.keys();

        // Draw initial screen
        self.render();

        for evt in events {
            match evt {
                Ok(key) => {
                    // Handle key press
                    match key {
                        Key::Char('q') | Key::Esc => {
                            // Exit application
                            break;
                        }
                        _ => {
                            // Handle key based on current view
                            self.handle_key(key);
                        }
                    }

                    // Redraw screen after key press
                    self.render();
                }
                Err(_) => {}
            }
        }
    }

    // Handle key press based on current view
    fn handle_key(&mut self, key: Key) {
        match self.current_view {
            View::LanguageList => self.handle_language_list_key(key),
            View::SectionList => self.handle_section_list_key(key),
            View::KeyDetail => self.handle_key_detail_key(key),
            View::EditValue => self.handle_edit_value_key(key),
            View::ConfirmSave => self.handle_confirm_save_key(key),
        }
    }

    // Handle keys in language list view
    fn handle_language_list_key(&mut self, key: Key) {
        match key {
            Key::Up | Key::Char('k') => {
                // Move selection up
                if !self.languages.is_empty() {
                    self.selected_language = self.selected_language.saturating_sub(1);
                }
            }
            Key::Down | Key::Char('j') => {
                // Move selection down
                if !self.languages.is_empty() {
                    self.selected_language = (self.selected_language + 1).min(self.languages.len() - 1);
                }
            }
            Key::Char('\n') | Key::Char(' ') => {
                // Select language and load its content
                if !self.languages.is_empty() {
                    self.load_language_content(self.selected_language);
                    self.current_view = View::SectionList;
                }
            }
            Key::Char('n') => {
                // Create new language
                self.add_new_language();
            }
            _ => {}
        }
    }

    // Handle keys in section list view
    fn handle_section_list_key(&mut self, key: Key) {
        let lang = self.get_current_language();

        if !self.json_tree.contains_key(&lang) {
            return;
        }

        let tree = &self.json_tree[&lang];
        let selected_idx = if let Some((_, idx)) = self.selected_node { idx } else { 0 };

        match key {
            Key::Up | Key::Char('k') => {
                // Move selection up
                if !tree.is_empty() {
                    let new_idx = selected_idx.saturating_sub(1);
                    self.selected_node = Some((lang.clone(), new_idx));
                }
            }
            Key::Down | Key::Char('j') => {
                // Move selection down
                if !tree.is_empty() {
                    let new_idx = (selected_idx + 1).min(tree.len() - 1);
                    self.selected_node = Some((lang.clone(), new_idx));
                }
            }
            Key::Char('\n') | Key::Char(' ') => {
                // Select item
                if !tree.is_empty() {
                    let node = &tree[selected_idx];

                    if node.is_leaf {
                        // If it's a leaf node, go to edit view
                        self.current_view = View::KeyDetail;
                    } else {
                        // If it's a branch node, toggle expansion
                        self.toggle_node_expansion(lang.clone(), selected_idx);
                    }
                }
            }
            Key::Char('e') => {
                // Edit value
                if !tree.is_empty() {
                    let node = &tree[selected_idx];

                    if node.is_leaf {
                        self.current_view = View::EditValue;
                    }
                }
            }
            Key::Char('n') => {
                // Add new key
                self.add_new_key();
            }
            Key::Char('b') | Key::Backspace | Key::Ctrl('h') => {
                // Go back to language list
                self.current_view = View::LanguageList;
            }
            _ => {}
        }
    }

    // Handle keys in key detail view
    fn handle_key_detail_key(&mut self, key: Key) {
        match key {
            Key::Char('e') => {
                // Edit value
                self.current_view = View::EditValue;
            }
            Key::Char('b') | Key::Backspace | Key::Ctrl('h') | Key::Esc => {
                // Go back to section list
                self.current_view = View::SectionList;
            }
            _ => {}
        }
    }

    // Handle keys in edit value view
    fn handle_edit_value_key(&mut self, _key: Key) {
        // This would be implemented for real editing
        // For now, just go back to key detail view
        self.current_view = View::KeyDetail;
    }

    // Handle keys in confirm save view
    fn handle_confirm_save_key(&mut self, key: Key) {
        match key {
            Key::Char('y') => {
                // Confirm save
                self.save_current_language();
                self.current_view = View::SectionList;
            }
            Key::Char('n') | Key::Esc => {
                // Cancel save
                self.current_view = View::SectionList;
            }
            _ => {}
        }
    }

    // Get the current language code
    fn get_current_language(&self) -> String {
        if self.languages.is_empty() {
            String::new()
        } else {
            self.languages[self.selected_language].clone()
        }
    }

    // Load and parse the content of a language file
    fn load_language_content(&mut self, language_idx: usize) {
        if language_idx >= self.languages.len() {
            return;
        }

        let lang = &self.languages[language_idx];
        let file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), lang);

        // Try to read and parse the file
        let content = match read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                self.message = Some((format!("Error reading {}.json: {}", lang, e), true));
                return;
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                self.message = Some((format!("Error parsing {}.json: {}", lang, e), true));
                return;
            }
        };

        // Convert JSON to tree structure
        let tree = self.json_to_tree(&json);
        self.json_tree.insert(lang.clone(), tree);

        // Set initial selection
        if let Some(tree) = self.json_tree.get(lang) {
            if !tree.is_empty() {
                self.selected_node = Some((lang.clone(), 0));
            }
        }
    }

    // Toggle expansion of a tree node
    fn toggle_node_expansion(&mut self, lang: String, idx: usize) {
        if !self.json_tree.contains_key(&lang) {
            return;
        }

        let tree = self.json_tree.get_mut(&lang).unwrap();
        if idx >= tree.len() {
            return;
        }

        if !tree[idx].is_leaf {
            tree[idx].is_expanded = !tree[idx].is_expanded;
        }
    }

    // Convert a JSON value to a tree structure for display
    fn json_to_tree(&self, json: &Value) -> Vec<JsonTreeNode> {
        let mut tree = Vec::new();
        self.build_tree(json, "", 0, &mut tree);
        tree
    }

    // Recursively build tree structure from JSON
    fn build_tree(&self, value: &Value, path: &str, depth: usize, tree: &mut Vec<JsonTreeNode>) {
        match value {
            Value::Object(obj) => {
                for (key, val) in obj {
                    let new_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };

                    let is_leaf = !matches!(val, Value::Object(_));
                    let value_str = if is_leaf {
                        match val {
                            Value::String(s) => s.clone(),
                            _ => val.to_string(),
                        }
                    } else {
                        String::new()
                    };

                    tree.push(JsonTreeNode {
                        key: key.clone(),
                        path: new_path.clone(),
                        value: value_str,
                        is_leaf,
                        is_expanded: false,
                        depth,
                    });

                    if !is_leaf {
                        self.build_tree(val, &new_path, depth + 1, tree);
                    }
                }
            }
            _ => {}
        }
    }

    // Create a new language file
    fn add_new_language(&mut self) {
        // This is a stub implementation
        self.message = Some(("New language creation not implemented in TUI mode yet. Use the CLI command.".to_string(), false));
    }

    // Add a new key to the current language
    fn add_new_key(&mut self) {
        // This is a stub implementation
        self.message = Some(("New key creation not implemented in TUI mode yet. Use the CLI command.".to_string(), false));
    }

    // Save changes to the current language file
    fn save_current_language(&mut self) {
        // This is a stub implementation
        self.message = Some(("Saving not implemented in TUI mode yet.".to_string(), false));
    }

    // Render the user interface
    fn render(&mut self) {
        if let Some(mut stdout) = self.run.take() {
            // Clear the screen
            write!(stdout, "{}{}", clear::All, cursor::Goto(1, 1)).unwrap();

            // Draw the header
            self.draw_header(&mut stdout);

            // Draw the body based on current view
            match self.current_view {
                View::LanguageList => self.draw_language_list(&mut stdout),
                View::SectionList => self.draw_section_list(&mut stdout),
                View::KeyDetail => self.draw_key_detail(&mut stdout),
                View::EditValue => self.draw_edit_value(&mut stdout),
                View::ConfirmSave => self.draw_confirm_save(&mut stdout),
            }

            // Draw the footer (help text)
            self.draw_footer(&mut stdout);

            // Draw any messages
            if let Some((msg, is_error)) = &self.message {
                if *is_error {
                    write!(stdout, "\n\r{}{}{}", color::Fg(color::Red), msg, color::Fg(color::Reset)).unwrap();
                } else {
                    write!(stdout, "\n\r{}{}{}", color::Fg(color::Green), msg, color::Fg(color::Reset)).unwrap();
                }
            }

            stdout.flush().unwrap();
            self.run = Some(stdout);
        }
    }

    // Draw the header section
    fn draw_header(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        // Title
        write!(stdout, "{}{} Locale Manager {}\n\r", style::Bold, color::Fg(color::Blue), style::Reset).unwrap();

        // Current view
        let view_name = match self.current_view {
            View::LanguageList => "Language List",
            View::SectionList => "Section List",
            View::KeyDetail => "Key Detail",
            View::EditValue => "Edit Value",
            View::ConfirmSave => "Confirm Save",
        };
        write!(stdout, "View: {}{}{}\n\r", color::Fg(color::Green), view_name, color::Fg(color::Reset)).unwrap();

        // Current language
        if !self.languages.is_empty() {
            let current_lang = &self.languages[self.selected_language];
            write!(stdout, "Current Language: {}{}{}\n\r", color::Fg(color::Yellow), current_lang, color::Fg(color::Reset)).unwrap();
        }

        // Separator
        write!(stdout, "{}\n\r", "-".repeat(50)).unwrap();
    }

    // Draw the language list view
    fn draw_language_list(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        write!(stdout, "Available Languages:\n\r").unwrap();

        if self.languages.is_empty() {
            write!(stdout, "  No language files found.\n\r").unwrap();
            write!(stdout, "  Press 'n' to create a new language.\n\r").unwrap();
        } else {
            for (i, lang) in self.languages.iter().enumerate() {
                let prefix = if i == self.selected_language { "> " } else { "  " };
                write!(stdout, "{}{}{}\n\r", prefix, lang, style::Reset).unwrap();
            }
        }
    }

    // Draw the section list view
    fn draw_section_list(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        if self.languages.is_empty() {
            write!(stdout, "No languages available.\n\r").unwrap();
            return;
        }

        let lang = self.get_current_language();

        if !self.json_tree.contains_key(&lang) {
            write!(stdout, "Loading content for {}...\n\r", lang).unwrap();
            return;
        }

        let tree = &self.json_tree[&lang];
        let selected_idx = if let Some((_, idx)) = self.selected_node { idx } else { 0 };

        write!(stdout, "Keys for {}:\n\r", lang).unwrap();

        if tree.is_empty() {
            write!(stdout, "  No keys found.\n\r").unwrap();
        } else {
            for (i, node) in tree.iter().enumerate() {
                // Only show nodes at depth 0 or expanded parents
                if node.depth == 0 || true {
                    let prefix = if i == selected_idx { "> " } else { "  " };
                    let indent = "  ".repeat(node.depth);

                    let node_icon = if node.is_leaf {
                        "📄"
                    } else if node.is_expanded {
                        "📂"
                    } else {
                        "📁"
                    };

                    write!(stdout, "{}{}{} {}", prefix, indent, node_icon, node.key).unwrap();

                    if node.is_leaf {
                        write!(stdout, ": {}", node.value).unwrap();
                    }

                    write!(stdout, "\n\r").unwrap();
                }
            }
        }
    }

    // Draw the key detail view
    fn draw_key_detail(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        let lang = self.get_current_language();

        if !self.json_tree.contains_key(&lang) {
            return;
        }

        let tree = &self.json_tree[&lang];
        let selected_idx = if let Some((_, idx)) = self.selected_node { idx } else { 0 };

        if selected_idx >= tree.len() {
            return;
        }

        let node = &tree[selected_idx];

        write!(stdout, "Key: {}{}{}\n\r", color::Fg(color::Green), node.path, color::Fg(color::Reset)).unwrap();
        write!(stdout, "Value: {}{}{}\n\r", color::Fg(color::Yellow), node.value, color::Fg(color::Reset)).unwrap();

        write!(stdout, "\n\rPress 'e' to edit, 'b' to go back\n\r").unwrap();
    }

    // Draw the edit value view
    fn draw_edit_value(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        let lang = self.get_current_language();

        if !self.json_tree.contains_key(&lang) {
            return;
        }

        let tree = &self.json_tree[&lang];
        let selected_idx = if let Some((_, idx)) = self.selected_node { idx } else { 0 };

        if selected_idx >= tree.len() {
            return;
        }

        let node = &tree[selected_idx];

        write!(stdout, "Editing: {}{}{}\n\r", color::Fg(color::Green), node.path, color::Fg(color::Reset)).unwrap();
        write!(stdout, "Current value: {}{}{}\n\r", color::Fg(color::Yellow), node.value, color::Fg(color::Reset)).unwrap();

        write!(stdout, "\n\rEdit mode not implemented in the TUI yet.\n\r").unwrap();
        write!(stdout, "Press any key to go back.\n\r").unwrap();
    }

    // Draw the confirm save view
    fn draw_confirm_save(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        write!(stdout, "{}Save changes to {}?{}\n\r", color::Fg(color::Yellow), self.get_current_language(), color::Fg(color::Reset)).unwrap();
        write!(stdout, "\n\r").unwrap();
        write!(stdout, "Press 'y' to save, 'n' to cancel\n\r").unwrap();
    }

    // Draw the footer section
    fn draw_footer(&self, stdout: &mut termion::raw::RawTerminal<std::io::Stdout>) {
        // Separator
        write!(stdout, "\n\r{}\n\r", "-".repeat(50)).unwrap();

        // Help text based on current view
        match self.current_view {
            View::LanguageList => {
                write!(stdout, "j/k: Move selection, Enter: Select language, n: New language, q: Quit\n\r").unwrap();
            }
            View::SectionList => {
                write!(stdout, "j/k: Move, Enter: Select/Expand, e: Edit, n: New key, b: Back, q: Quit\n\r").unwrap();
            }
            View::KeyDetail => {
                write!(stdout, "e: Edit, b: Back, q: Quit\n\r").unwrap();
            }
            View::EditValue => {
                write!(stdout, "Edit mode (ESC: Cancel, Enter: Save)\n\r").unwrap();
            }
            View::ConfirmSave => {
                write!(stdout, "y: Yes, n: No\n\r").unwrap();
            }
        }
    }
}
