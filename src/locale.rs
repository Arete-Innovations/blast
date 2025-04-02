use crate::configs::Config;
use crate::progress::ProgressManager;
use dialoguer::{theme::ColorfulTheme, FuzzySelect, Input, Select};
use serde_json::Value;
use std::fs::{self, create_dir_all, read_to_string};
use std::path::Path;

// Define the modules
pub mod locale_helpers;
pub mod locale_tui;

// Re-export items that need to be public
pub use self::locale_helpers::{get_default_language, get_language_files, get_locale_dir, update_nested_key};
pub use self::locale_tui::LocaleManagerApp;

pub const TBI_PLACEHOLDER: &str = "TBI (To Be Implemented)";
pub const DEFAULT_LOCALE_DIR: &str = "src/assets/locale";

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
    let mut app = self::locale_tui::LocaleManagerApp::new();
    app.run();
}

// Displays the content of all language files
fn view_language_files() {
    let languages = locale_helpers::get_language_files();
    if languages.is_empty() {
        println!("No language files found in {}", locale_helpers::get_locale_dir(&None));
        return;
    }

    let theme = ColorfulTheme::default();
    let selection = Select::with_theme(&theme).with_prompt("Select a language to view").items(&languages).default(0).interact().unwrap_or(0);

    let lang_code = &languages[selection];
    view_language_content(lang_code);
}

// Display the language file content in an organized way
fn view_language_content(lang_code: &str) {
    let file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), lang_code);
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
    locale_helpers::print_json_content("", &json);
    println!("\nPress Enter to continue...");
    let _: String = Input::new().interact_text().unwrap_or_default();
}

// Edit a key in the language file
fn edit_existing_key() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Loading language files...");

    let languages = locale_helpers::get_language_files();
    if languages.is_empty() {
        progress.error("No language files found");
        return;
    }

    // First select a language
    let theme = ColorfulTheme::default();
    let lang_selection = Select::with_theme(&theme).with_prompt("Select a language to edit").items(&languages).default(0).interact().unwrap_or(0);

    let lang_code = &languages[lang_selection];
    let file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), lang_code);

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
    let paths = locale_helpers::extract_json_paths("", &json, Vec::new());
    if paths.is_empty() {
        println!("No keys found in the language file.");
        return;
    }

    // Select a path to edit
    let key_selection = FuzzySelect::with_theme(&theme).with_prompt("Select a key to edit").items(&paths).default(0).interact().unwrap_or(0);

    let key_path = &paths[key_selection];
    let current_value = locale_helpers::get_value_from_path(&json, key_path).unwrap_or(&Value::Null);

    // Input new value
    let value: String = Input::new()
        .with_prompt(&format!("Enter new value for '{}'", key_path))
        .with_initial_text(current_value.as_str().unwrap_or(""))
        .interact_text()
        .unwrap();

    // Update the file
    locale_helpers::update_nested_key(&file_path, key_path, &value);
    println!("Updated key '{}' in {}", key_path, lang_code);
}

// Add a new key to the language file
fn add_new_key() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Loading language structure...");

    // Get default language file for structure
    let default_lang = locale_helpers::get_default_language(&None);
    let default_file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), default_lang);

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
    for lang in locale_helpers::get_language_files() {
        let file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), lang);
        let value = if lang == default_lang { key_value.clone() } else { TBI_PLACEHOLDER.to_string() };

        locale_helpers::update_nested_key(&file_path, &full_key_path, &value);
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
    let default_lang = locale_helpers::get_default_language(&None);

    // Input title and subtitle for default language
    let title: String = Input::new().with_prompt(&format!("Enter title for '{}' page", page_path)).interact_text().unwrap_or_default();

    let subtitle: String = Input::new().with_prompt(&format!("Enter subtitle for '{}' page", page_path)).interact_text().unwrap_or_default();

    // Add to all language files
    for lang in locale_helpers::get_language_files() {
        let file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), lang);

        // For default language, use provided values
        // For other languages, use TBI placeholders
        let (title_value, subtitle_value) = if lang == default_lang {
            (title.clone(), subtitle.clone())
        } else {
            (TBI_PLACEHOLDER.to_string(), TBI_PLACEHOLDER.to_string())
        };

        locale_helpers::update_nested_key(&file_path, &format!("pages.{}.title", page_path), &title_value);
        locale_helpers::update_nested_key(&file_path, &format!("pages.{}.subtitle", page_path), &subtitle_value);
    }

    progress.success(&format!("Added new page '{}' to all language files", page_path));
}

// Add a new language by copying the structure of the default language
fn add_new_language() {
    let progress = ProgressManager::new_spinner();
    progress.set_message("Preparing to add new language...");

    // Get default language for structure
    let default_lang = locale_helpers::get_default_language(&None);
    let default_file_path = format!("{}/{}.json", locale_helpers::get_locale_dir(&None), default_lang);

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
    let locale_dir = locale_helpers::get_locale_dir(&None);
    if let Err(e) = create_dir_all(&locale_dir) {
        println!("Error creating locale directory: {}", e);
        return;
    }

    let new_file_path = format!("{}/{}.json", locale_dir, lang_code);

    // Create a new JSON with the same structure but TBI values
    let new_json = locale_helpers::replace_with_placeholders(&default_json);

    // Write the new language file
    if let Err(e) = fs::write(&new_file_path, serde_json::to_string_pretty(&new_json).unwrap_or_default()) {
        println!("Error writing new language file: {}", e);
        return;
    }

    println!("Added new language '{}' with structure from {}", lang_code, default_lang);
}
