use crate::configs::Config;
use crate::progress::ProgressManager;
use dialoguer::{theme::ColorfulTheme, FuzzySelect, Input, Select};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{self, create_dir_all, read_to_string, OpenOptions};
use std::io::{stdout, Write};
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{clear, color, cursor, style};

// Enum for different views in the TUI
#[derive(PartialEq, Copy, Clone)]
enum View {
    Overview,        // Main language selection view
    LanguageDetails, // Viewing specific language content
    EditKey,         // Editing a specific key
    AddKey,          // Adding a new key
    AddPage,         // Adding a new page
    AddLanguage,     // Adding a new language
}

// Struct to represent a node in the JSON tree view
#[derive(Clone)]
struct JsonTreeNode {
    path: String,       // Full dotted path to this node
    key: String,        // Key name for this node
    value: String,      // Value as string (or special representation for objects/arrays)
    has_children: bool, // Whether this node has child nodes
    expanded: bool,     // Whether this node is expanded in the tree view
    level: usize,       // Indentation level in the tree
}

const DEFAULT_LOCALE_DIR: &str = "src/assets/locale";
const TBI_PLACEHOLDER: &str = "TBI (To Be Implemented)";

// Main entry point for basic interactive locale management
pub fn edit_key() {
    let options = vec!["View Language Files", "Edit Existing Key", "Add New Key", "Add New Page", "Add New Language", "🔙 Back"];

    let theme = ColorfulTheme::default();
    let selection = Select::with_theme(&theme).with_prompt("Locale Management").items(&options).default(0).interact().unwrap_or(5); // Default to Back if error

    match selection {
        0 => view_language_files(),
        1 => edit_existing_key(),
        2 => add_new_key(),
        3 => add_new_page(),
        4 => add_new_language(),
        _ => return, // Back to main menu
    }
}

// Advanced locale management interface for the dashboard
pub fn launch_manager() {
    // Create and run the full-screen TUI locale manager
    let mut app = LocaleManagerApp::new();
    app.run();
}

// Displays the content of all language files
fn view_language_files_interactive(languages: &[String]) {
    // Clear screen
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== LANGUAGE FILES ===\x1b[0m\n");

    // Select a language
    let theme = ColorfulTheme::default();
    let selection = Select::with_theme(&theme).with_prompt("Select a language to view").default(0).items(languages).interact().unwrap_or(0);

    let lang_code = &languages[selection];
    let file_path = format!("{}/{}.json", get_locale_dir(&None), lang_code);

    // Read and parse JSON
    let content = match read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading language file: {}", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing JSON: {}", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Clear screen again to show the content
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== {} LANGUAGE FILE ===\x1b[0m\n", lang_code.to_uppercase());

    // Print the JSON content in a readable format
    print_json_structure(&json, 0);

    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Edit a key in a language file
fn edit_language_key_interactive(languages: &[String]) {
    // Clear screen
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== EDIT LANGUAGE KEY ===\x1b[0m\n");

    // Select a language
    let theme = ColorfulTheme::default();
    let lang_selection = Select::with_theme(&theme).with_prompt("Select a language to edit").default(0).items(languages).interact().unwrap_or(0);

    let lang_code = &languages[lang_selection];
    let file_path = format!("{}/{}.json", get_locale_dir(&None), lang_code);

    // Read and parse JSON
    let content = match read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading language file: {}", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing JSON: {}", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Extract all paths for selection
    let paths = extract_json_paths("", &json, Vec::new());
    if paths.is_empty() {
        println!("No keys found in the language file.");
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return;
    }

    // Select a key to edit
    let key_selection = FuzzySelect::with_theme(&theme).with_prompt("\nSelect a key to edit").default(0).items(&paths).interact().unwrap_or(0);

    let key_path = &paths[key_selection];
    let current_value = get_value_from_path(&json, key_path).unwrap_or(&Value::Null);

    // Input new value
    let value_result = Input::<String>::with_theme(&theme)
        .with_prompt(&format!("Edit value for '{}'", key_path))
        .with_initial_text(current_value.as_str().unwrap_or(""))
        .interact();

    match value_result {
        Ok(value) => {
            // Update the file
            update_nested_key(&file_path, key_path, &value);
            println!("\n\x1b[32mUpdated key '{}' in {}\x1b[0m", key_path, lang_code);
        }
        Err(_) => {
            println!("\n\x1b[31mEdit cancelled\x1b[0m");
        }
    }

    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Add a new key to all language files
fn add_language_key_interactive(languages: &[String]) {
    if languages.is_empty() {
        println!("No language files found.");
        return;
    }

    // Clear screen
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== ADD NEW LANGUAGE KEY ===\x1b[0m\n");

    // Get default language file for structure
    let default_lang = languages.iter().find(|l| *l == "en").unwrap_or(&languages[0]);
    let default_file_path = format!("{}/{}.json", get_locale_dir(&None), default_lang);

    let default_json = match fs::read_to_string(&default_file_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(json) => json,
            Err(e) => {
                println!("Error parsing JSON: {}", e);
                println!("\nPress Enter to continue...");
                let _: String = Input::new().interact_text().unwrap_or_default();
                return;
            }
        },
        Err(e) => {
            println!("Error reading default language file: {}", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Extract section groups from the default language
    let sections: Vec<String> = if let Some(obj) = default_json.as_object() {
        obj.keys().cloned().collect()
    } else {
        vec!["pages".to_string(), "buttons".to_string(), "errors".to_string(), "placeholders".to_string()]
    };

    // Select a section
    let theme = ColorfulTheme::default();
    let section_selection = Select::with_theme(&theme).with_prompt("Select a section for the new key").default(0).items(&sections).interact();

    let section = match section_selection {
        Ok(idx) if idx < sections.len() => sections[idx].clone(),
        _ => {
            println!("\n\x1b[31mSection selection cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Input the key name
    let key_name_result = Input::<String>::with_theme(&theme).with_prompt("Enter new key name").interact();

    let key_name = match key_name_result {
        Ok(name) => name,
        Err(_) => {
            println!("\n\x1b[31mKey name input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    if key_name.is_empty() {
        println!("\n\x1b[31mKey name cannot be empty. Operation cancelled.\x1b[0m");
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return;
    }

    // Input the key value
    let key_value_result = Input::<String>::with_theme(&theme).with_prompt("Enter value for this key").interact();

    let key_value = match key_value_result {
        Ok(value) => value,
        Err(_) => {
            println!("\n\x1b[31mKey value input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    let full_key_path = format!("{}.{}", section, key_name);

    // Update all language files with this new key
    for language in languages {
        let file_path = format!("{}/{}.json", get_locale_dir(&None), language);
        let value = if language == default_lang { key_value.clone() } else { TBI_PLACEHOLDER.to_string() };

        update_nested_key(&file_path, &full_key_path, &value);
    }

    println!("\n\x1b[32mAdded new key '{}' to all language files\x1b[0m", full_key_path);
    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Add a new page with title and subtitle
fn add_page_interactive(languages: &[String]) {
    if languages.is_empty() {
        println!("No language files found.");
        return;
    }

    // Clear screen
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== ADD NEW PAGE ===\x1b[0m\n");

    // Input the page path
    let theme = ColorfulTheme::default();
    let page_path_result = Input::<String>::with_theme(&theme).with_prompt("Enter the page path (e.g., 'user/profile' or 'admin/dashboard')").interact();

    let page_path = match page_path_result {
        Ok(path) => path,
        Err(_) => {
            println!("\n\x1b[31mPage path input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    if page_path.is_empty() {
        println!("\n\x1b[31mPage path cannot be empty\x1b[0m");
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return;
    }

    // Get default language
    let default_lang = languages.iter().find(|l| *l == "en").unwrap_or(&languages[0]);

    // Input title for default language
    let title_result = Input::<String>::with_theme(&theme).with_prompt(&format!("Enter title for '{}' page", page_path)).interact();

    let title = match title_result {
        Ok(title) => title,
        Err(_) => {
            println!("\n\x1b[31mTitle input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Input subtitle for default language
    let subtitle_result = Input::<String>::with_theme(&theme).with_prompt(&format!("Enter subtitle for '{}' page", page_path)).interact();

    let subtitle = match subtitle_result {
        Ok(subtitle) => subtitle,
        Err(_) => {
            println!("\n\x1b[31mSubtitle input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return;
        }
    };

    // Add to all language files
    for language in languages {
        let file_path = format!("{}/{}.json", get_locale_dir(&None), language);

        // For default language, use provided values
        // For other languages, use TBI placeholders
        let (title_value, subtitle_value) = if language == default_lang {
            (title.clone(), subtitle.clone())
        } else {
            (TBI_PLACEHOLDER.to_string(), TBI_PLACEHOLDER.to_string())
        };

        update_nested_key(&file_path, &format!("pages.{}.title", page_path), &title_value);
        update_nested_key(&file_path, &format!("pages.{}.subtitle", page_path), &subtitle_value);
    }

    println!("\n\x1b[32mAdded new page '{}' to all language files\x1b[0m", page_path);
    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Add a new language by copying the structure from the default language
fn add_language_interactive(languages: &[String]) -> Option<String> {
    if languages.is_empty() {
        println!("No language files found.");
        return None;
    }

    // Clear screen
    println!("\x1b[2J\x1b[H");
    println!("\x1b[1;34m=== ADD NEW LANGUAGE ===\x1b[0m\n");

    // Get default language for structure
    let default_lang = languages.iter().find(|l| *l == "en").unwrap_or(&languages[0]);
    let default_file_path = format!("{}/{}.json", get_locale_dir(&None), default_lang);

    let default_json = match fs::read_to_string(&default_file_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(json) => json,
            Err(e) => {
                println!("\n\x1b[31mError parsing JSON: {}\x1b[0m", e);
                println!("\nPress Enter to continue...");
                let _: String = Input::new().interact_text().unwrap_or_default();
                return None;
            }
        },
        Err(e) => {
            println!("\n\x1b[31mError reading default language file: {}\x1b[0m", e);
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return None;
        }
    };

    // Input the new language code
    let theme = ColorfulTheme::default();
    let lang_code_result = Input::<String>::with_theme(&theme).with_prompt("Enter the new language code (e.g., 'fr', 'es', 'de')").interact();

    let lang_code = match lang_code_result {
        Ok(code) => code,
        Err(_) => {
            println!("\n\x1b[31mLanguage code input cancelled\x1b[0m");
            println!("\nPress Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();
            return None;
        }
    };

    if lang_code.is_empty() {
        println!("\n\x1b[31mLanguage code cannot be empty. Operation cancelled.\x1b[0m");
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return None;
    }

    // Make sure locale directory exists
    let locale_dir = get_locale_dir(&None);
    if let Err(e) = create_dir_all(&locale_dir) {
        println!("\n\x1b[31mError creating locale directory: {}\x1b[0m", e);
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return None;
    }

    let new_file_path = format!("{}/{}.json", locale_dir, lang_code);

    // Create a new JSON with the same structure but TBI values
    let new_json = replace_with_placeholders(&default_json);

    // Write the new language file
    if let Err(e) = fs::write(&new_file_path, serde_json::to_string_pretty(&new_json).unwrap_or_default()) {
        println!("\n\x1b[31mError writing new language file: {}\x1b[0m", e);
        println!("\nPress Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();
        return None;
    }

    println!("\n\x1b[32mAdded new language '{}' with structure from {}\x1b[0m", lang_code, default_lang);
    Some(lang_code)
}

// Helper function to print JSON structure with proper indentation
fn print_json_structure(json: &Value, indent: usize) {
    let indent_str = "  ".repeat(indent);

    match json {
        Value::Object(map) => {
            println!("{}{{", indent_str);
            for (key, value) in map {
                match value {
                    Value::Object(_) => {
                        println!("{}  \x1b[1;33m{}:\x1b[0m", indent_str, key);
                        print_json_structure(value, indent + 1);
                    }
                    Value::Array(_) => {
                        println!("{}  \x1b[1;33m{}:\x1b[0m [", indent_str, key);
                        print_json_structure(value, indent + 1);
                        println!("{}  ]", indent_str);
                    }
                    _ => {
                        let value_str = match value {
                            Value::String(s) => format!("\x1b[1;32m\"{}\"\x1b[0m", s),
                            Value::Number(n) => format!("\x1b[1;36m{}\x1b[0m", n),
                            Value::Bool(b) => format!("\x1b[1;35m{}\x1b[0m", b),
                            Value::Null => "\x1b[1;31mnull\x1b[0m".to_string(),
                            _ => format!("{}", value),
                        };
                        println!("{}  \x1b[1;34m{}:\x1b[0m {}", indent_str, key, value_str);
                    }
                }
            }
            println!("{}}}", indent_str);
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                println!("{}  [{}]:", indent_str, i);
                print_json_structure(item, indent + 1);
            }
        }
        _ => {
            let value_str = match json {
                Value::String(s) => format!("\x1b[1;32m\"{}\"\x1b[0m", s),
                Value::Number(n) => format!("\x1b[1;36m{}\x1b[0m", n),
                Value::Bool(b) => format!("\x1b[1;35m{}\x1b[0m", b),
                Value::Null => "\x1b[1;31mnull\x1b[0m".to_string(),
                _ => format!("{}", json),
            };
            println!("{}{}", indent_str, value_str);
        }
    }
}

// Full-screen TUI application for locale management
struct LocaleManagerApp {
    languages: Vec<String>,                                  // Available language codes
    selected_language: usize,                                // Index of currently selected language
    current_view: View,                                      // Current view being displayed
    json_tree: HashMap<String, Vec<JsonTreeNode>>,           // Tree structure for language details view
    selected_node: Option<(String, usize)>,                  // Currently selected node (section name, index)
    message: Option<(String, bool)>,                         // Optional message to display (text, is_error)
    run: Option<termion::raw::RawTerminal<std::io::Stdout>>, // Terminal in raw mode
}

impl LocaleManagerApp {
    fn new() -> Self {
        let languages = get_language_files();
        let selected_language = languages.iter().position(|l| l == "en").unwrap_or(0);

        let mut app = Self {
            languages,
            selected_language,
            current_view: View::Overview,
            json_tree: HashMap::new(),
            selected_node: None,
            message: None,
            run: None,
        };

        app.load_json_tree();
        app
    }

    fn initialize_terminal(&mut self) {
        // Initialize the terminal in raw mode if it's not already initialized
        if self.run.is_none() {
            self.run = Some(stdout().into_raw_mode().unwrap());
        }
    }

    fn exit_raw_mode(&mut self) {
        // Take ownership of the terminal and drop it, exiting raw mode
        self.run.take();
    }

    fn run(&mut self) {
        // Initialize terminal
        self.initialize_terminal();
        let stdin = termion::async_stdin();
        let mut keys = stdin.keys();

        // Clear the screen and make the cursor invisible
        if let Some(stdout) = &mut self.run {
            write!(stdout, "{}{}", clear::All, cursor::Hide).unwrap();
            stdout.flush().unwrap();
        }

        loop {
            // Draw the current view
            self.draw_interface();

            // Check for input
            if let Some(Ok(key)) = keys.next() {
                // Handle global exit keys
                if key == Key::Esc || key == Key::Ctrl('c') {
                    break;
                }

                // Handle view-specific input
                match self.current_view {
                    View::Overview => self.handle_overview_input(key),
                    View::LanguageDetails => self.handle_language_details_input(key),
                    View::EditKey => self.handle_edit_key_input(key),
                    View::AddKey => self.handle_add_key_input(key),
                    View::AddPage => self.handle_add_page_input(key),
                    View::AddLanguage => self.handle_add_language_input(key),
                }
            }

            // Sleep to avoid CPU usage
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Restore cursor and clear screen on exit
        if let Some(stdout) = &mut self.run {
            write!(stdout, "{}{}", clear::All, cursor::Show).unwrap();
            stdout.flush().unwrap();
        }
    }

    fn draw_interface(&mut self) {
        // Clone all data before borrowing stdout
        let current_view = self.current_view;
        let languages = self.languages.clone();
        let selected_language = self.selected_language;
        let json_tree = self.json_tree.clone();
        let selected_node = self.selected_node.clone();
        let message = self.message.clone();

        // Take a reference to the terminal temporarily
        let stdout_opt = self.run.as_mut();
        if let Some(stdout) = stdout_opt {
            // Clear the screen
            write!(stdout, "{}", clear::All).unwrap();

            // Draw the specific view based on current state
            match current_view {
                View::Overview => {
                    // Draw language selection screen
                    write!(stdout, "{}{}{}== BLAST LOCALE MANAGER =={}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, style::Reset).unwrap();

                    write!(stdout, "{}Available Languages:", cursor::Goto(1, 3)).unwrap();

                    for (i, lang) in languages.iter().enumerate() {
                        let marker = if i == selected_language { ">" } else { " " };
                        write!(stdout, "{}{} {}{}", cursor::Goto(1, 5 + i as u16), if i == selected_language { "\x1b[32m" } else { "\x1b[37m" }, marker, lang).unwrap();
                    }

                    // Show help
                    write!(stdout, "{}{}Controls:{}", cursor::Goto(1, 5 + languages.len() as u16 + 2), color::Fg(color::Yellow), style::Reset).unwrap();

                    write!(stdout, "{} ↑/↓: Select language", cursor::Goto(1, 5 + languages.len() as u16 + 4)).unwrap();

                    write!(stdout, "{} Enter: View language details", cursor::Goto(1, 5 + languages.len() as u16 + 5)).unwrap();

                    write!(stdout, "{} a: Add new language", cursor::Goto(1, 5 + languages.len() as u16 + 6)).unwrap();

                    write!(stdout, "{} Esc: Exit", cursor::Goto(1, 5 + languages.len() as u16 + 7)).unwrap();

                    // Display message if any
                    if let Some((msg, is_error)) = &message {
                        write!(
                            stdout,
                            "{}{}{}{}",
                            cursor::Goto(1, 5 + languages.len() as u16 + 9),
                            if *is_error { "\x1b[31m" } else { "\x1b[32m" },
                            msg,
                            style::Reset
                        )
                        .unwrap();
                    }
                }
                View::LanguageDetails => {
                    // Draw language details screen
                    if !languages.is_empty() && selected_language < languages.len() {
                        let lang = &languages[selected_language];

                        write!(stdout, "{}{}{}== {} LANGUAGE =={}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, lang.to_uppercase(), style::Reset).unwrap();

                        // Draw sections
                        let mut y = 3;
                        for (section, nodes) in &json_tree {
                            write!(stdout, "{}{}{}:{}", cursor::Goto(1, y), color::Fg(color::Green), section, style::Reset).unwrap();
                            y += 1;

                            // Draw keys for this section
                            for (i, node) in nodes.iter().enumerate() {
                                let is_selected = selected_node.as_ref().map_or(false, |(s, idx)| s == section && *idx == i);
                                let marker = if is_selected { ">" } else { " " };

                                write!(
                                    stdout,
                                    "{}{}{}{} {}: {}{}",
                                    cursor::Goto(1, y),
                                    if is_selected { "\x1b[33m" } else { "\x1b[37m" },
                                    "  ".repeat(node.level),
                                    marker,
                                    node.key,
                                    if node.value.len() > 30 { format!("{}...", &node.value[..30]) } else { node.value.clone() },
                                    style::Reset
                                )
                                .unwrap();
                                y += 1;
                            }

                            // Add space between sections
                            y += 1;
                        }

                        // Show help
                        write!(stdout, "{}{}Controls:{}", cursor::Goto(1, y + 1), color::Fg(color::Yellow), style::Reset).unwrap();

                        write!(stdout, "{} ↑/↓: Navigate entries", cursor::Goto(1, y + 3)).unwrap();

                        write!(stdout, "{} Enter: Edit selected key", cursor::Goto(1, y + 4)).unwrap();

                        write!(stdout, "{} a: Add new key", cursor::Goto(1, y + 5)).unwrap();

                        write!(stdout, "{} p: Add new page", cursor::Goto(1, y + 6)).unwrap();

                        write!(stdout, "{} Backspace: Return to overview", cursor::Goto(1, y + 7)).unwrap();
                    }
                }
                View::EditKey => {
                    // Draw edit key screen
                    write!(stdout, "{}{}{}EDIT KEY{}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, style::Reset).unwrap();

                    write!(stdout, "{}Press any key to return...", cursor::Goto(1, 3)).unwrap();
                }
                View::AddKey => {
                    // Draw add key screen
                    write!(stdout, "{}{}{}ADD NEW KEY{}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, style::Reset).unwrap();

                    write!(stdout, "{}Press any key to return...", cursor::Goto(1, 3)).unwrap();
                }
                View::AddPage => {
                    // Draw add page screen
                    write!(stdout, "{}{}{}ADD NEW PAGE{}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, style::Reset).unwrap();

                    write!(stdout, "{}Press any key to return...", cursor::Goto(1, 3)).unwrap();
                }
                View::AddLanguage => {
                    // Draw add language screen
                    write!(stdout, "{}{}{}ADD NEW LANGUAGE{}", cursor::Goto(1, 1), color::Fg(color::Blue), style::Bold, style::Reset).unwrap();

                    write!(stdout, "{}Press any key to return...", cursor::Goto(1, 3)).unwrap();
                }
            }

            // Draw the status bar
            let (_, height) = termion::terminal_size().unwrap_or((80, 24));
            write!(
                stdout,
                "{}{}{}{}Blast Locale Manager | ESC to exit | {} language(s){}",
                cursor::Goto(1, height),
                color::Bg(color::Blue),
                color::Fg(color::White),
                style::Bold,
                languages.len(),
                style::Reset
            )
            .unwrap();

            // Flush output
            stdout.flush().unwrap();
        }
    }

    fn handle_overview_input(&mut self, key: Key) {
        match key {
            Key::Up => {
                if !self.languages.is_empty() {
                    self.selected_language = (self.selected_language + self.languages.len() - 1) % self.languages.len();
                }
            }
            Key::Down => {
                if !self.languages.is_empty() {
                    self.selected_language = (self.selected_language + 1) % self.languages.len();
                }
            }
            Key::Char('\n') => {
                if !self.languages.is_empty() {
                    // Load JSON tree for the selected language
                    self.load_json_tree();
                    self.current_view = View::LanguageDetails;
                    self.selected_node = None;
                }
            }
            Key::Char('a') => {
                self.current_view = View::AddLanguage;
                self.interactive_add_language();
            }
            _ => {}
        }
    }

    fn handle_language_details_input(&mut self, key: Key) {
        match key {
            Key::Up => {
                self.navigate_tree_up();
            }
            Key::Down => {
                self.navigate_tree_down();
            }
            Key::Char('\n') => {
                if self.selected_node.is_some() {
                    self.current_view = View::EditKey;
                    self.interactive_edit_key();
                }
            }
            Key::Char('a') => {
                self.current_view = View::AddKey;
                self.interactive_add_key();
            }
            Key::Char('p') => {
                self.current_view = View::AddPage;
                self.interactive_add_page();
            }
            Key::Backspace => {
                self.current_view = View::Overview;
            }
            _ => {}
        }
    }

    fn handle_edit_key_input(&mut self, _key: Key) {
        // This is just a placeholder, the actual editing happens in interactive_edit_key
        self.current_view = View::LanguageDetails;
    }

    fn handle_add_key_input(&mut self, _key: Key) {
        // This is just a placeholder, the actual adding happens in interactive_add_key
        self.current_view = View::LanguageDetails;
    }

    fn handle_add_page_input(&mut self, _key: Key) {
        // This is just a placeholder, the actual adding happens in interactive_add_page
        self.current_view = View::LanguageDetails;
    }

    fn handle_add_language_input(&mut self, _key: Key) {
        // This is just a placeholder, the actual adding happens in interactive_add_language
        self.current_view = View::Overview;
    }

    fn load_json_tree(&mut self) {
        if self.languages.is_empty() || self.selected_language >= self.languages.len() {
            return;
        }

        let lang = &self.languages[self.selected_language];
        let file_path = format!("{}/{}.json", get_locale_dir(&None), lang);

        let content = match read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) => {
                self.message = Some((format!("Error reading language file: {}", e), true));
                return;
            }
        };

        let json: Value = match serde_json::from_str(&content) {
            Ok(json) => json,
            Err(e) => {
                self.message = Some((format!("Error parsing JSON: {}", e), true));
                return;
            }
        };

        self.json_tree = HashMap::new();

        // Build the tree for display
        if let Some(obj) = json.as_object() {
            for (section, value) in obj {
                let mut nodes = Vec::new();
                self.build_tree_nodes(&mut nodes, section, value, "", 0);
                self.json_tree.insert(section.clone(), nodes);
            }
        }

        // Set a default selection if we have nodes
        if !self.json_tree.is_empty() {
            let first_section = self.json_tree.keys().next().unwrap().clone();
            if !self.json_tree[&first_section].is_empty() {
                self.selected_node = Some((first_section, 0));
            }
        }
    }

    fn build_tree_nodes(&self, nodes: &mut Vec<JsonTreeNode>, key: &str, value: &Value, path_prefix: &str, level: usize) {
        let path = if path_prefix.is_empty() { key.to_string() } else { format!("{}.{}", path_prefix, key) };

        match value {
            Value::Object(obj) => {
                nodes.push(JsonTreeNode {
                    path: path.clone(),
                    key: key.to_string(),
                    value: "[Object]".to_string(),
                    has_children: true,
                    expanded: true,
                    level,
                });

                for (child_key, child_value) in obj {
                    self.build_tree_nodes(nodes, child_key, child_value, &path, level + 1);
                }
            }
            Value::Array(arr) => {
                nodes.push(JsonTreeNode {
                    path: path.clone(),
                    key: key.to_string(),
                    value: format!("[Array: {} items]", arr.len()),
                    has_children: true,
                    expanded: false,
                    level,
                });
            }
            Value::String(s) => {
                nodes.push(JsonTreeNode {
                    path: path.clone(),
                    key: key.to_string(),
                    value: s.clone(),
                    has_children: false,
                    expanded: false,
                    level,
                });
            }
            _ => {
                nodes.push(JsonTreeNode {
                    path: path.clone(),
                    key: key.to_string(),
                    value: value.to_string(),
                    has_children: false,
                    expanded: false,
                    level,
                });
            }
        }
    }

    fn navigate_tree_up(&mut self) {
        if let Some((section, idx)) = &self.selected_node {
            if *idx > 0 {
                // Move up within the same section
                self.selected_node = Some((section.clone(), idx - 1));
            } else {
                // Find the previous section
                let sections: Vec<_> = self.json_tree.keys().collect();
                if let Some(pos) = sections.iter().position(|s| s == &section) {
                    if pos > 0 {
                        let prev_section = (*sections[pos - 1]).clone();
                        let prev_nodes = &self.json_tree[&prev_section];
                        if !prev_nodes.is_empty() {
                            self.selected_node = Some((prev_section, prev_nodes.len() - 1));
                        }
                    }
                }
            }
        } else {
            // No current selection, select the last item of the last section
            let sections: Vec<_> = self.json_tree.keys().collect();
            if !sections.is_empty() {
                let last_section = (*sections.last().unwrap()).clone();
                let nodes = &self.json_tree[&last_section];
                if !nodes.is_empty() {
                    self.selected_node = Some((last_section, nodes.len() - 1));
                }
            }
        }
    }

    fn navigate_tree_down(&mut self) {
        if let Some((section, idx)) = &self.selected_node {
            let nodes = &self.json_tree[section];
            if idx + 1 < nodes.len() {
                // Move down within the same section
                self.selected_node = Some((section.clone(), idx + 1));
            } else {
                // Find the next section
                let sections: Vec<_> = self.json_tree.keys().collect();
                if let Some(pos) = sections.iter().position(|s| s == &section) {
                    if pos + 1 < sections.len() {
                        let next_section = (*sections[pos + 1]).clone();
                        let next_nodes = &self.json_tree[&next_section];
                        if !next_nodes.is_empty() {
                            self.selected_node = Some((next_section, 0));
                        }
                    }
                }
            }
        } else {
            // No current selection, select the first item of the first section
            let sections: Vec<_> = self.json_tree.keys().collect();
            if !sections.is_empty() {
                let first_section = (*sections.first().unwrap()).clone();
                let nodes = &self.json_tree[&first_section];
                if !nodes.is_empty() {
                    self.selected_node = Some((first_section, 0));
                }
            }
        }
    }

    fn interactive_edit_key(&mut self) {
        if self.languages.is_empty() || self.selected_language >= self.languages.len() {
            return;
        }

        if let Some((section, idx)) = &self.selected_node {
            let lang_idx = self.selected_language;
            let lang = self.languages[lang_idx].clone();
            let file_path = format!("{}/{}.json", get_locale_dir(&None), lang);

            let section_clone = section.clone();
            let idx_clone = *idx;
            let nodes = &self.json_tree[&section_clone];
            if idx_clone >= nodes.len() {
                return;
            }

            let node = &nodes[idx_clone];
            let key_path = node.path.clone();
            let node_value = node.value.clone();

            // Completely exit raw mode before starting editor
            self.exit_raw_mode();

            // Create a temporary file explicitly
            let temp_dir = std::env::temp_dir();
            let temp_file_path = temp_dir.join(format!("blast_locale_edit_{}.txt", std::process::id()));

            // Write content to temp file
            if let Err(e) = std::fs::write(&temp_file_path, &node_value) {
                println!("Error creating temporary file: {}", e);
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }

            // Clear the terminal completely and restore normal terminal mode
            println!("\x1Bc"); // Reset terminal
            println!("\n\n=== EDIT LOCALE KEY ===");
            println!("Language: {}", lang);
            println!("Key path: {}\n", key_path);
            println!("Current value: {}\n", node_value);

            // Determine which editor to use
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

            // Launch the editor directly in a separate process
            println!("Launching {} to edit key value...", editor);
            println!("The editor will open in fullscreen mode.");
            println!("Press Enter to continue...");
            let _: String = Input::new().interact_text().unwrap_or_default();

            // Execute the editor in the foreground with specific arguments
            // For vim, we need to start in insert mode to avoid the command bar issue
            let status = if editor == "vim" || editor == "nvim" {
                std::process::Command::new(&editor)
                    .arg("-c")
                    .arg("startinsert") // Start in insert mode
                    .arg(&temp_file_path)
                    .status()
            } else {
                std::process::Command::new(&editor).arg(&temp_file_path).status()
            };

            // Process the result
            let new_value = match status {
                Ok(exit_status) if exit_status.success() => {
                    // Read the edited content
                    match std::fs::read_to_string(&temp_file_path) {
                        Ok(content) => content.trim().to_string(),
                        Err(e) => {
                            println!("Error reading edited file: {}", e);
                            println!("\nPress Enter to return...");
                            let _: String = Input::new().interact_text().unwrap_or_default();

                            // Clean up
                            let _ = std::fs::remove_file(&temp_file_path);

                            self.current_view = View::LanguageDetails;
                            self.initialize_terminal();
                            return;
                        }
                    }
                }
                Ok(_) => {
                    println!("Editor closed without saving or with error");
                    println!("\nPress Enter to return...");
                    let _: String = Input::new().interact_text().unwrap_or_default();

                    // Clean up
                    let _ = std::fs::remove_file(&temp_file_path);

                    self.current_view = View::LanguageDetails;
                    self.initialize_terminal();
                    return;
                }
                Err(e) => {
                    println!("Error launching editor: {}", e);
                    println!("\nPress Enter to return...");
                    let _: String = Input::new().interact_text().unwrap_or_default();

                    // Clean up
                    let _ = std::fs::remove_file(&temp_file_path);

                    self.current_view = View::LanguageDetails;
                    self.initialize_terminal();
                    return;
                }
            };

            // Clean up the temp file
            let _ = std::fs::remove_file(&temp_file_path);

            // Check if value changed
            if new_value != node_value {
                // Show the changed value
                println!("\nOriginal: {}", node_value);
                println!("New value: {}", new_value);

                // Update the file
                update_nested_key(&file_path, &key_path, &new_value);

                // Reload the JSON tree to reflect changes
                self.load_json_tree();
                self.message = Some((format!("Updated key '{}' in {}", key_path, lang), false));

                println!("\nKey updated successfully!");
            } else {
                println!("\nNo changes detected.");
                self.message = Some(("No changes made".to_string(), false));
            }

            println!("\nPress Enter to return to the locale manager...");
            let _: String = Input::new().interact_text().unwrap_or_default();
        }

        self.current_view = View::LanguageDetails;

        // Reinitialize the terminal for TUI
        self.initialize_terminal();
    }

    fn interactive_add_key(&mut self) {
        if self.languages.is_empty() || self.selected_language >= self.languages.len() {
            return;
        }

        let lang = self.languages[self.selected_language].clone();

        // Get sections
        let sections: Vec<String> = self.json_tree.keys().cloned().collect();

        // Temporarily exit raw mode for dialog interaction
        self.exit_raw_mode();

        // Clear screen for clean dialog
        println!("\x1b[2J\x1b[H");
        println!("\x1b[1;34m=== ADD NEW LOCALE KEY ===\x1b[0m\n");
        println!("Primary Language: {}\n", lang);

        // Use dialoguer for section selection
        let theme = ColorfulTheme::default();
        let section_selection = Select::with_theme(&theme).with_prompt("Select a section for the new key").items(&sections).default(0).interact();

        // Process section selection
        let section = match section_selection {
            Ok(idx) if idx < sections.len() => sections[idx].clone(),
            _ => {
                self.message = Some(("Section selection cancelled".to_string(), true));
                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        println!("\nSelected section: {}", section);

        // Input the key name
        let key_name_result = Input::<String>::with_theme(&theme).with_prompt("Enter new key name").interact();

        let key_name = match key_name_result {
            Ok(name) => name,
            Err(_) => {
                self.message = Some(("Key name input cancelled".to_string(), true));
                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        if key_name.is_empty() {
            self.message = Some(("Key name cannot be empty. Operation cancelled.".to_string(), true));
            self.current_view = View::LanguageDetails;
            self.initialize_terminal();
            return;
        }

        let full_key_path = format!("{}.{}", section, key_name);
        println!("\nFull key path: {}", full_key_path);

        // Create a temporary file explicitly
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join(format!("blast_locale_new_{}.txt", std::process::id()));

        // Create an empty temp file
        if let Err(e) = std::fs::write(&temp_file_path, "") {
            println!("Error creating temporary file: {}", e);
            println!("\nPress Enter to return...");
            let _: String = Input::new().interact_text().unwrap_or_default();

            self.current_view = View::LanguageDetails;
            self.initialize_terminal();
            return;
        }

        // Clear the terminal completely and restore normal terminal mode
        println!("\x1Bc"); // Reset terminal
        println!("\n\n=== ADD NEW LOCALE KEY ===");
        println!("Section: {}", section);
        println!("Key: {}\n", key_name);
        println!("Full path: {}\n", full_key_path);

        // Determine which editor to use
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

        // Launch the editor directly in a separate process
        println!("Launching {} to enter key value...", editor);
        println!("The editor will open in fullscreen mode.");
        println!("Press Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();

        // Execute the editor in the foreground with specific arguments
        // For vim, we need to start in insert mode to avoid the command bar issue
        let status = if editor == "vim" || editor == "nvim" {
            std::process::Command::new(&editor)
                .arg("-c")
                .arg("startinsert") // Start in insert mode
                .arg(&temp_file_path)
                .status()
        } else {
            std::process::Command::new(&editor).arg(&temp_file_path).status()
        };

        // Process the result
        let key_value = match status {
            Ok(exit_status) if exit_status.success() => {
                // Read the edited content
                match std::fs::read_to_string(&temp_file_path) {
                    Ok(content) => content.trim().to_string(),
                    Err(e) => {
                        println!("Error reading edited file: {}", e);
                        println!("\nPress Enter to return...");
                        let _: String = Input::new().interact_text().unwrap_or_default();

                        // Clean up
                        let _ = std::fs::remove_file(&temp_file_path);

                        self.current_view = View::LanguageDetails;
                        self.initialize_terminal();
                        return;
                    }
                }
            }
            Ok(_) => {
                println!("Editor closed without saving or with error");
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&temp_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
            Err(e) => {
                println!("Error launching editor: {}", e);
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&temp_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        // Clean up the temp file
        let _ = std::fs::remove_file(&temp_file_path);

        // Show the value that was entered
        println!("\nValue: {}", key_value);

        // Update all language files with this new key
        for language in &self.languages {
            let file_path = format!("{}/{}.json", get_locale_dir(&None), language);
            let value = if language == &lang { key_value.clone() } else { TBI_PLACEHOLDER.to_string() };

            update_nested_key(&file_path, &full_key_path, &value);
        }

        // Reload the JSON tree to reflect changes
        self.load_json_tree();
        self.message = Some((format!("Added new key '{}' to all language files", full_key_path), false));

        println!("\nNew key added successfully!");
        println!("\nPress Enter to return...");
        let _: String = Input::new().interact_text().unwrap_or_default();

        self.current_view = View::LanguageDetails;

        // Reinitialize the terminal raw mode
        self.initialize_terminal();
    }

    fn interactive_add_page(&mut self) {
        if self.languages.is_empty() {
            return;
        }

        // Temporarily exit raw mode for dialog interaction
        self.exit_raw_mode();

        // Clear screen for clean dialog
        println!("\x1b[2J\x1b[H");
        println!("\x1b[1;34m=== ADD NEW PAGE ===\x1b[0m\n");

        // Get default language
        let default_lang_idx = self.languages.iter().position(|l| l == "en").unwrap_or(0);
        let default_lang = self.languages[default_lang_idx].clone();
        println!("Primary Language: {}\n", default_lang);

        // Input the page path
        let theme = ColorfulTheme::default();
        let page_path_result = Input::<String>::with_theme(&theme).with_prompt("Enter the page path (e.g., 'user/profile' or 'admin/dashboard')").interact();

        let page_path = match page_path_result {
            Ok(path) => path,
            Err(_) => {
                self.message = Some(("Page path input cancelled".to_string(), true));
                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        if page_path.is_empty() {
            self.message = Some(("Page path cannot be empty".to_string(), true));
            self.current_view = View::LanguageDetails;
            self.initialize_terminal();
            return;
        }

        println!("\nPage path: {}", page_path);

        // Create temporary files explicitly for title and subtitle
        let temp_dir = std::env::temp_dir();
        let title_file_path = temp_dir.join(format!("blast_locale_title_{}.txt", std::process::id()));
        let subtitle_file_path = temp_dir.join(format!("blast_locale_subtitle_{}.txt", std::process::id()));

        // Create empty temp files
        if let Err(e) = std::fs::write(&title_file_path, "") {
            println!("Error creating temporary file: {}", e);
            println!("\nPress Enter to return...");
            let _: String = Input::new().interact_text().unwrap_or_default();

            self.current_view = View::LanguageDetails;
            self.initialize_terminal();
            return;
        }

        if let Err(e) = std::fs::write(&subtitle_file_path, "") {
            println!("Error creating temporary file: {}", e);
            println!("\nPress Enter to return...");
            let _: String = Input::new().interact_text().unwrap_or_default();

            // Clean up
            let _ = std::fs::remove_file(&title_file_path);

            self.current_view = View::LanguageDetails;
            self.initialize_terminal();
            return;
        }

        // Clear the terminal completely and restore normal terminal mode
        println!("\x1Bc"); // Reset terminal
        println!("\n\n=== ADD NEW PAGE ===");
        println!("Primary Language: {}", default_lang);
        println!("Page Path: {}\n", page_path);

        // Determine which editor to use
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

        // Launch editor for title
        println!("Launching {} to enter page title...", editor);
        println!("The editor will open in fullscreen mode.");
        println!("Press Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();

        // Execute the editor for title with proper vim arguments
        let title_status = if editor == "vim" || editor == "nvim" {
            std::process::Command::new(&editor)
                .arg("-c")
                .arg("startinsert") // Start in insert mode
                .arg(&title_file_path)
                .status()
        } else {
            std::process::Command::new(&editor).arg(&title_file_path).status()
        };

        // Process title result
        let title = match title_status {
            Ok(exit_status) if exit_status.success() => {
                match std::fs::read_to_string(&title_file_path) {
                    Ok(content) => content.trim().to_string(),
                    Err(e) => {
                        println!("Error reading title file: {}", e);
                        println!("\nPress Enter to return...");
                        let _: String = Input::new().interact_text().unwrap_or_default();

                        // Clean up
                        let _ = std::fs::remove_file(&title_file_path);
                        let _ = std::fs::remove_file(&subtitle_file_path);

                        self.current_view = View::LanguageDetails;
                        self.initialize_terminal();
                        return;
                    }
                }
            }
            Ok(_) => {
                println!("Editor closed without saving or with error");
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&title_file_path);
                let _ = std::fs::remove_file(&subtitle_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
            Err(e) => {
                println!("Error launching editor: {}", e);
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&title_file_path);
                let _ = std::fs::remove_file(&subtitle_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        // Now edit subtitle
        println!("\nLaunching {} to enter page subtitle...", editor);
        println!("The editor will open in fullscreen mode.");
        println!("Press Enter to continue...");
        let _: String = Input::new().interact_text().unwrap_or_default();

        // Execute the editor for subtitle with proper vim arguments
        let subtitle_status = if editor == "vim" || editor == "nvim" {
            std::process::Command::new(&editor)
                .arg("-c")
                .arg("startinsert") // Start in insert mode
                .arg(&subtitle_file_path)
                .status()
        } else {
            std::process::Command::new(&editor).arg(&subtitle_file_path).status()
        };

        // Process subtitle result
        let subtitle = match subtitle_status {
            Ok(exit_status) if exit_status.success() => {
                match std::fs::read_to_string(&subtitle_file_path) {
                    Ok(content) => content.trim().to_string(),
                    Err(e) => {
                        println!("Error reading subtitle file: {}", e);
                        println!("\nPress Enter to return...");
                        let _: String = Input::new().interact_text().unwrap_or_default();

                        // Clean up
                        let _ = std::fs::remove_file(&title_file_path);
                        let _ = std::fs::remove_file(&subtitle_file_path);

                        self.current_view = View::LanguageDetails;
                        self.initialize_terminal();
                        return;
                    }
                }
            }
            Ok(_) => {
                println!("Editor closed without saving or with error");
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&title_file_path);
                let _ = std::fs::remove_file(&subtitle_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
            Err(e) => {
                println!("Error launching editor: {}", e);
                println!("\nPress Enter to return...");
                let _: String = Input::new().interact_text().unwrap_or_default();

                // Clean up
                let _ = std::fs::remove_file(&title_file_path);
                let _ = std::fs::remove_file(&subtitle_file_path);

                self.current_view = View::LanguageDetails;
                self.initialize_terminal();
                return;
            }
        };

        // Clean up temp files
        let _ = std::fs::remove_file(&title_file_path);
        let _ = std::fs::remove_file(&subtitle_file_path);

        println!("\nTitle: {}", title);
        println!("Subtitle: {}", subtitle);

        // Add to all language files
        for language in &self.languages {
            let file_path = format!("{}/{}.json", get_locale_dir(&None), language);

            // For default language, use provided values
            // For other languages, use TBI placeholders
            let (title_value, subtitle_value) = if language == &default_lang {
                (title.clone(), subtitle.clone())
            } else {
                (TBI_PLACEHOLDER.to_string(), TBI_PLACEHOLDER.to_string())
            };

            update_nested_key(&file_path, &format!("pages.{}.title", page_path), &title_value);
            update_nested_key(&file_path, &format!("pages.{}.subtitle", page_path), &subtitle_value);
        }

        // Reload the JSON tree to reflect changes
        self.load_json_tree();
        self.message = Some((format!("Added new page '{}' to all language files", page_path), false));

        println!("\nNew page added successfully!");
        println!("\nPress Enter to return...");
        let _: String = Input::new().interact_text().unwrap_or_default();

        self.current_view = View::LanguageDetails;

        // Reinitialize the terminal raw mode
        self.initialize_terminal();
    }

    fn interactive_add_language(&mut self) {
        // Temporarily exit raw mode to use dialoguer properly
        self.exit_raw_mode();

        // Get default language
        let default_lang_idx = self.languages.iter().position(|l| l == "en").unwrap_or(0);
        let default_lang = self.languages[default_lang_idx].clone();
        let default_file_path = format!("{}/{}.json", get_locale_dir(&None), default_lang);

        let default_json = match fs::read_to_string(&default_file_path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(json) => json,
                Err(e) => {
                    self.message = Some((format!("Error parsing JSON: {}", e), true));
                    self.current_view = View::Overview;
                    self.initialize_terminal();
                    return;
                }
            },
            Err(e) => {
                self.message = Some((format!("Error reading default language file: {}", e), true));
                self.current_view = View::Overview;
                self.initialize_terminal();
                return;
            }
        };

        // Input the new language code
        let theme = ColorfulTheme::default();
        let lang_code_result = Input::<String>::with_theme(&theme).with_prompt("Enter the new language code (e.g., 'fr', 'es', 'de')").interact();

        let lang_code = match lang_code_result {
            Ok(code) => code,
            Err(_) => {
                self.message = Some(("Language code input cancelled".to_string(), true));
                self.current_view = View::Overview;
                self.initialize_terminal();
                return;
            }
        };

        if lang_code.is_empty() {
            self.message = Some(("Language code cannot be empty. Operation cancelled.".to_string(), true));
            self.current_view = View::Overview;
            self.initialize_terminal();
            return;
        }

        // Make sure locale directory exists
        let locale_dir = get_locale_dir(&None);
        if let Err(e) = create_dir_all(&locale_dir) {
            self.message = Some((format!("Error creating locale directory: {}", e), true));
            self.current_view = View::Overview;
            self.initialize_terminal();
            return;
        }

        let new_file_path = format!("{}/{}.json", locale_dir, lang_code);

        // Create a new JSON with the same structure but TBI values
        let new_json = replace_with_placeholders(&default_json);

        // Write the new language file
        if let Err(e) = fs::write(&new_file_path, serde_json::to_string_pretty(&new_json).unwrap_or_default()) {
            self.message = Some((format!("Error writing new language file: {}", e), true));
            self.current_view = View::Overview;
            self.initialize_terminal();
            return;
        }

        // Add the new language to our list
        let default_lang_copy = default_lang.clone();
        self.languages.push(lang_code.clone());
        self.message = Some((format!("Added new language '{}' with structure from {}", lang_code, default_lang_copy), false));
        self.current_view = View::Overview;

        // Reinitialize the terminal raw mode
        self.initialize_terminal();
    }
}

// Get the locale directory from config or use the default
fn get_locale_dir(config: &Option<&Config>) -> String {
    if let Some(conf) = config {
        conf.assets.get("locale").and_then(|locale| locale.get("dir")).and_then(|v| v.as_str()).unwrap_or(DEFAULT_LOCALE_DIR).to_string()
    } else {
        DEFAULT_LOCALE_DIR.to_string()
    }
}

// Get default language from config or use "en"
fn get_default_language(config: &Option<&Config>) -> String {
    if let Some(conf) = config {
        conf.assets.get("locale").and_then(|locale| locale.get("default_language")).and_then(|v| v.as_str()).unwrap_or("en").to_string()
    } else {
        "en".to_string()
    }
}

// List all available language files
fn get_language_files() -> Vec<String> {
    let locale_dir = get_locale_dir(&None);
    let mut languages = Vec::new();

    if let Ok(entries) = fs::read_dir(&locale_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Some(lang_code) = path.file_stem().and_then(|os_str| os_str.to_str()) {
                    languages.push(lang_code.to_string());
                }
            }
        }
    }

    // Sort languages but ensure 'en' is first if it exists
    languages.sort();
    if let Some(en_pos) = languages.iter().position(|lang| lang == "en") {
        if en_pos > 0 {
            languages.remove(en_pos);
            languages.insert(0, "en".to_string());
        }
    }

    languages
}

// View and browse language files
fn view_language_files() {
    let languages = get_language_files();
    if languages.is_empty() {
        println!("No language files found in {}", get_locale_dir(&None));
        return;
    }

    let theme = ColorfulTheme::default();
    let selection = Select::with_theme(&theme).with_prompt("Select a language to view").items(&languages).default(0).interact().unwrap_or(0);

    let lang_code = &languages[selection];
    view_language_content(lang_code);
}

// Display the language file content in an organized way
fn view_language_content(lang_code: &str) {
    let file_path = format!("{}/{}.json", get_locale_dir(&None), lang_code);
    let content = match read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading language file: {}", e);
            return;
        }
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing JSON: {}", e);
            return;
        }
    };

    println!("\n{} Language File Contents:\n", lang_code.to_uppercase());
    print_json_content("", &json);
    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Recursively print JSON content with proper indentation
fn print_json_content(prefix: &str, value: &Value) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                match val {
                    Value::Object(_) => {
                        println!("{}{}:", prefix, key);
                        print_json_content(&format!("{}  ", prefix), val);
                    }
                    Value::Array(arr) => {
                        println!("{}{}:", prefix, key);
                        for (i, item) in arr.iter().enumerate() {
                            println!("{}  [{}]:", prefix, i);
                            print_json_content(&format!("{}    ", prefix), item);
                        }
                    }
                    _ => println!("{}{}: {}", prefix, key, val),
                }
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                println!("{}[{}]:", prefix, i);
                print_json_content(&format!("{}  ", prefix), item);
            }
        }
        _ => println!("{}{}", prefix, value),
    }
}

// Edit a key in the language file
fn edit_existing_key() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Loading language files...");

    let languages = get_language_files();
    if languages.is_empty() {
        progress.error("No language files found");
        return;
    }

    // First select a language
    let theme = ColorfulTheme::default();
    let lang_selection = Select::with_theme(&theme).with_prompt("Select a language to edit").items(&languages).default(0).interact().unwrap_or(0);

    let lang_code = &languages[lang_selection];
    let file_path = format!("{}/{}.json", get_locale_dir(&None), lang_code);

    // Load JSON
    let content = match read_to_string(&file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading language file: {}", e);
            return;
        }
    };

    let json: Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing JSON: {}", e);
            return;
        }
    };

    // Extract all paths for selection
    let paths = extract_json_paths("", &json, Vec::new());
    if paths.is_empty() {
        println!("No keys found in the language file.");
        return;
    }

    // Select a path to edit
    let key_selection = FuzzySelect::with_theme(&theme).with_prompt("Select a key to edit").items(&paths).default(0).interact().unwrap_or(0);

    let key_path = &paths[key_selection];
    let current_value = get_value_from_path(&json, key_path).unwrap_or(&Value::Null);

    // Input new value
    let value: String = Input::new()
        .with_prompt(&format!("Enter new value for '{}'", key_path))
        .with_initial_text(current_value.as_str().unwrap_or(""))
        .interact_text()
        .unwrap();

    // Update the file
    update_nested_key(&file_path, key_path, &value);
    println!("Updated key '{}' in {}", key_path, lang_code);
}

// Add a new key to the language file
fn add_new_key() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Loading language structure...");

    // Get default language file for structure
    let default_lang = get_default_language(&None);
    let default_file_path = format!("{}/{}.json", get_locale_dir(&None), default_lang);

    let default_json = match fs::read_to_string(&default_file_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(json) => json,
            Err(e) => {
                progress.error(&format!("Error parsing JSON: {}", e));
                return;
            }
        },
        Err(e) => {
            progress.error(&format!("Error reading default language file: {}", e));
            return;
        }
    };

    // Extract section groups from the default language
    let sections: Vec<String> = if let Some(obj) = default_json.as_object() {
        obj.keys().cloned().collect()
    } else {
        vec!["pages".to_string(), "buttons".to_string(), "errors".to_string(), "placeholders".to_string()]
    };

    // Select a section
    let theme = ColorfulTheme::default();
    let section_selection = Select::with_theme(&theme).with_prompt("Select a section for the new key").items(&sections).default(0).interact().unwrap_or(0);

    let section = &sections[section_selection];

    // Input the key name
    let key_name: String = Input::new().with_prompt("Enter new key name").interact_text().unwrap_or_default();

    if key_name.is_empty() {
        println!("Key name cannot be empty. Operation cancelled.");
        return;
    }

    // Input the key value
    let key_value: String = Input::new().with_prompt("Enter value for this key").interact_text().unwrap_or_default();

    let full_key_path = format!("{}.{}", section, key_name);

    // Update all language files with this new key
    for lang in get_language_files() {
        let file_path = format!("{}/{}.json", get_locale_dir(&None), lang);
        let value = if lang == default_lang { key_value.clone() } else { TBI_PLACEHOLDER.to_string() };

        update_nested_key(&file_path, &full_key_path, &value);
    }

    println!("Added new key '{}' to all language files", full_key_path);
}

// Add a new page with title and subtitle to all language files
fn add_new_page() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Preparing to add new page...");

    // Input the page path
    let page_path: String = Input::new().with_prompt("Enter the page path (e.g., 'user/profile' or 'admin/dashboard')").interact_text().unwrap_or_default();

    if page_path.is_empty() {
        progress.error("Page path cannot be empty");
        return;
    }

    progress.set_message("Adding page to language files...");

    // Get default language for primary values
    let default_lang = get_default_language(&None);

    // Input title and subtitle for default language
    let title: String = Input::new().with_prompt(&format!("Enter title for '{}' page", page_path)).interact_text().unwrap_or_default();

    let subtitle: String = Input::new().with_prompt(&format!("Enter subtitle for '{}' page", page_path)).interact_text().unwrap_or_default();

    // Add to all language files
    for lang in get_language_files() {
        let file_path = format!("{}/{}.json", get_locale_dir(&None), lang);

        // For default language, use provided values
        // For other languages, use TBI placeholders
        let (title_value, subtitle_value) = if lang == default_lang {
            (title.clone(), subtitle.clone())
        } else {
            (TBI_PLACEHOLDER.to_string(), TBI_PLACEHOLDER.to_string())
        };

        update_nested_key(&file_path, &format!("pages.{}.title", page_path), &title_value);
        update_nested_key(&file_path, &format!("pages.{}.subtitle", page_path), &subtitle_value);
    }

    progress.success(&format!("Added new page '{}' to all language files", page_path));
}

// Add a new language by copying the structure of the default language
fn add_new_language() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Preparing to add new language...");

    // Get default language for structure
    let default_lang = get_default_language(&None);
    let default_file_path = format!("{}/{}.json", get_locale_dir(&None), default_lang);

    let default_json = match fs::read_to_string(&default_file_path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(json) => json,
            Err(e) => {
                progress.error(&format!("Error parsing JSON: {}", e));
                return;
            }
        },
        Err(e) => {
            progress.error(&format!("Error reading default language file: {}", e));
            return;
        }
    };

    // Input the new language code
    let lang_code: String = Input::new().with_prompt("Enter the new language code (e.g., 'fr', 'es', 'de')").interact_text().unwrap_or_default();

    if lang_code.is_empty() {
        println!("Language code cannot be empty. Operation cancelled.");
        return;
    }

    // Make sure locale directory exists
    let locale_dir = get_locale_dir(&None);
    if let Err(e) = create_dir_all(&locale_dir) {
        println!("Error creating locale directory: {}", e);
        return;
    }

    let new_file_path = format!("{}/{}.json", locale_dir, lang_code);

    // Create a new JSON with the same structure but TBI values
    let new_json = replace_with_placeholders(&default_json);

    // Write the new language file
    if let Err(e) = fs::write(&new_file_path, serde_json::to_string_pretty(&new_json).unwrap_or_default()) {
        println!("Error writing new language file: {}", e);
        return;
    }

    println!("Added new language '{}' with structure from {}", lang_code, default_lang);
}

// Create a new JSON with the same structure but "TBI" as values
fn replace_with_placeholders(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut new_obj = serde_json::Map::new();
            for (k, v) in obj {
                new_obj.insert(k.clone(), replace_with_placeholders(v));
            }
            Value::Object(new_obj)
        }
        Value::Array(arr) => {
            let new_arr = arr.iter().map(|v| replace_with_placeholders(v)).collect();
            Value::Array(new_arr)
        }
        Value::String(_) => Value::String(TBI_PLACEHOLDER.to_string()),
        _ => value.clone(), // Keep other types as is
    }
}

// Extract all paths from JSON for selection
fn extract_json_paths(prefix: &str, value: &Value, mut paths: Vec<String>) -> Vec<String> {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                let new_prefix = if prefix.is_empty() { key.clone() } else { format!("{}.{}", prefix, key) };
                match val {
                    Value::Object(_) => {
                        paths = extract_json_paths(&new_prefix, val, paths);
                    }
                    Value::Array(_) => {
                        // For arrays, we add the path to the array itself
                        paths.push(new_prefix);
                    }
                    _ => {
                        paths.push(new_prefix);
                    }
                }
            }
        }
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let new_prefix = format!("{}[{}]", prefix, i);
                paths = extract_json_paths(&new_prefix, item, paths);
            }
        }
        _ => {
            // Leaf node, add the path
            if !prefix.is_empty() {
                paths.push(prefix.to_string());
            }
        }
    }
    paths
}

// Get a value from a dotted path in JSON
fn get_value_from_path<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        if let Some(obj) = current.as_object() {
            if let Some(val) = obj.get(part) {
                current = val;
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    Some(current)
}

// Update a nested key in the JSON file
fn update_nested_key(file_path: &str, key_path: &str, value: &str) {
    // Read the file
    let content = match read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            println!("Error reading file {}: {}", file_path, e);
            return;
        }
    };

    // Parse the JSON
    let mut json: Value = match serde_json::from_str(&content) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing JSON in {}: {}", file_path, e);
            return;
        }
    };

    // Split the path
    let parts: Vec<&str> = key_path.split('.').collect();

    // Recursively build or update the nested structure
    update_nested_value(&mut json, &parts, value);

    // Write back to file
    let formatted_json = match serde_json::to_string_pretty(&json) {
        Ok(formatted) => formatted,
        Err(e) => {
            println!("Error formatting JSON: {}", e);
            return;
        }
    };

    match OpenOptions::new().write(true).truncate(true).open(file_path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(formatted_json.as_bytes()) {
                println!("Error writing to file {}: {}", file_path, e);
            }
        }
        Err(e) => {
            println!("Error opening file {}: {}", file_path, e);
        }
    }
}

// Recursively update or create a nested value in JSON
fn update_nested_value(json: &mut Value, parts: &[&str], value: &str) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        // Last part, set the value
        if let Some(obj) = json.as_object_mut() {
            obj.insert(parts[0].to_string(), Value::String(value.to_string()));
        }
        return;
    }

    // Handle nested structure
    let current_part = parts[0];
    let remaining_parts = &parts[1..];

    if let Some(obj) = json.as_object_mut() {
        if !obj.contains_key(current_part) {
            // Create a new object for this part
            obj.insert(current_part.to_string(), json!({}));
        }

        if let Some(next_obj) = obj.get_mut(current_part) {
            update_nested_value(next_obj, remaining_parts, value);
        }
    }
}
