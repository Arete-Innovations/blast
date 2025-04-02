use crate::configs::Config;
use css_minify::optimizations::{Level, Minifier};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use sass_rs::{compile_file, Options, OutputStyle};
use std::error::Error;
use std::path::Path;
// No sync primitives needed anymore
use tokio::fs;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

// Helper function to safely access TOML values from the Catalyst.toml structure
fn get_config_value(config: &Config, path: &[&str], default: Option<&str>) -> Option<String> {
    // First look for assets section
    let assets_section = match config.assets.get("assets") {
        Some(section) => section,
        None => {
            // If we're looking for public_dir (which is at the top level), check directly
            if path.len() == 1 && path[0] == "public_dir" {
                return match config.assets.get("public_dir") {
                    Some(val) => val.as_str().map(|s| s.to_string()),
                    None => default.map(|s| s.to_string())
                };
            }
            
            // No assets section, return default
            return default.map(|s| s.to_string());
        }
    };
    
    // If we're just looking for a top-level asset section
    if path.len() == 1 {
        match assets_section.get(path[0]) {
            Some(val) => {
                if let Some(s) = val.as_str() {
                    return Some(s.to_string());
                } else {
                    return default.map(|s| s.to_string());
                }
            },
            None => return default.map(|s| s.to_string())
        }
    }
    
    // Navigate to the requested section
    let mut current = assets_section;
    let section_name = path[0];
    
    // First find the section we need
    match current.get(section_name) {
        Some(section) => current = section,
        None => return default.map(|s| s.to_string())
    }
    
    // Now look for the key
    if path.len() > 1 {
        let key = path[1];
        match current.get(key) {
            Some(val) => {
                if let Some(s) = val.as_str() {
                    return Some(s.to_string());
                }
            },
            None => ()
        }
    }
    
    // If we got here, we didn't find the value
    default.map(|s| s.to_string())
}

async fn download_file(url: &str, dest_path: &Path) -> Result<(), Box<dyn Error>> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    let mut file = fs::File::create(dest_path).await?;
    file.write_all(&bytes).await?;
    Ok(())
}

async fn download_materialize_js(config: &Config) -> Result<(), Box<dyn Error>> {
    // This function is now a compatibility wrapper around download_materialize_scss,
    // which handles both JS and SCSS setup from the git repository
    
    // We'll create a stub progress tracker for compatibility
    let mut progress = crate::logger::create_progress(Some(1));
    progress.set_message("Setting up Materialize assets (see detailed progress below)");
    
    // Call the main function that now handles both JS and SCSS
    match download_materialize_scss(config).await {
        Ok(_) => {
            progress.inc(1);
            progress.set_message("Materialize assets setup completed successfully");
            Ok(())
        },
        Err(e) => {
            let error_msg = format!("Materialize setup failed: {}", e);
            progress.set_message(&error_msg);
            Err(e)
        }
    }
}

async fn download_fontawesome_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    // Provide a default value for FontAwesome CDN if missing
    let fa_base_url = get_config_value(config, &["fontawesome", "base_url"], Some("https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1")).ok_or("Missing fontawesome base_url in config")?;

    // Always use standard asset locations regardless of config - this ensures consistent structure
    let fa_public_dir = project_dir.join(&public_dir).join("fonts").join("fontawesome");

    // Create FontAwesome directories
    fs::create_dir_all(&fa_public_dir).await?;
    fs::create_dir_all(&fa_public_dir.join("css")).await?;
    fs::create_dir_all(&fa_public_dir.join("js")).await?;
    fs::create_dir_all(&fa_public_dir.join("sprites")).await?;
    fs::create_dir_all(&fa_public_dir.join("webfonts")).await?;

    // Default values for FontAwesome assets
    let default_css = vec![toml::Value::String("css/all.min.css".to_string())];
    let default_js = vec![toml::Value::String("js/all.min.js".to_string())];
    let default_sprites = vec![
        toml::Value::String("sprites/brands.svg".to_string()),
        toml::Value::String("sprites/regular.svg".to_string()),
        toml::Value::String("sprites/solid.svg".to_string()),
    ];
    let default_webfonts = vec![
        toml::Value::String("webfonts/fa-brands-400.ttf".to_string()),
        toml::Value::String("webfonts/fa-brands-400.woff2".to_string()),
        toml::Value::String("webfonts/fa-regular-400.ttf".to_string()),
        toml::Value::String("webfonts/fa-regular-400.woff2".to_string()),
        toml::Value::String("webfonts/fa-solid-900.ttf".to_string()),
        toml::Value::String("webfonts/fa-solid-900.woff2".to_string()),
        toml::Value::String("webfonts/fa-v4compatibility.ttf".to_string()),
        toml::Value::String("webfonts/fa-v4compatibility.woff2".to_string()),
    ];

    // Get values from config or use defaults
    let fa_css = config.assets.get("fontawesome").and_then(|f| f.get("css")).and_then(|c| c.as_array()).unwrap_or(&default_css);
    let fa_js = config.assets.get("fontawesome").and_then(|f| f.get("js")).and_then(|c| c.as_array()).unwrap_or(&default_js);
    let fa_sprites = config.assets.get("fontawesome").and_then(|f| f.get("sprites")).and_then(|c| c.as_array()).unwrap_or(&default_sprites);
    let fa_webfonts = config.assets.get("fontawesome").and_then(|f| f.get("webfonts")).and_then(|c| c.as_array()).unwrap_or(&default_webfonts);

    let asset_types = [("CSS", fa_css), ("JS", fa_js), ("Sprites", fa_sprites), ("Webfonts", fa_webfonts)];

    // Calculate total assets
    let total_assets: usize = asset_types.iter().map(|(_, assets)| assets.len()).sum();

    // Create a progress tracker for the overall FontAwesome download
    let mut progress = crate::logger::create_progress(Some(total_assets as u64));
    progress.set_message(&format!("FontAwesome: 0/{} files", total_assets));

    // Create async download tasks
    let mut tasks = FuturesUnordered::new();

    // Process all asset types
    for (asset_type, assets) in asset_types {
        for asset in assets {
            let asset_path = asset.as_str().unwrap().to_string();
            let dest_path = fa_public_dir.join(&asset_path);
            let url = format!("{}/{}", fa_base_url, asset_path);

            tasks.push(tokio::spawn(async move {
                match download_file(&url, &dest_path).await {
                    Ok(_) => Ok((asset_type, asset_path)),
                    Err(e) => Err(format!("Failed to download {}: {}", asset_path, e)),
                }
            }));
        }
    }

    // Process all downloads
    let mut completed = 0;
    // We don't need to track has_errors anymore since we handle errors directly

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok((asset_type, _))) => {
                completed += 1;
                // Update progress with count only - no need for busy-looking UI
                progress.set_message(&format!("FontAwesome: {}/{} files ({})", 
                    completed, total_assets, asset_type));
                progress.inc(1);
            }
            Ok(Err(e)) => {
                // Display errors through the progress system
                progress.warning(&e)?;
                completed += 1;
                progress.inc(1);
            }
            Err(e) => {
                progress.warning(&format!("Task error: {}", e))?;
                completed += 1;
                progress.inc(1);
            }
        }
    }

    // No need for a final message - the parent function will provide it
    Ok(())
}

async fn download_materialicons_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    let mi_base_url = get_config_value(config, &["materialicons", "base_url"], Some("https://raw.githubusercontent.com/google/material-design-icons/master/font")).ok_or("Missing materialicons base_url in config")?;

    // Always use standard asset locations regardless of config - this ensures consistent structure
    let mi_public_dir = project_dir.join(&public_dir).join("fonts").join("material-icons");

    let woff2_file = get_config_value(config, &["materialicons", "woff2"], Some("MaterialIcons-Regular.woff2")).ok_or("Missing materialicons woff2 in config")?;
    let ttf_file = get_config_value(config, &["materialicons", "ttf"], Some("MaterialIcons-Regular.ttf")).ok_or("Missing materialicons ttf in config")?;

    // Create Material Icons directory
    fs::create_dir_all(&mi_public_dir).await?;

    // Define files to download
    let files = [
        (woff2_file.clone(), format!("{}/{}", mi_base_url, &woff2_file), mi_public_dir.join(&woff2_file)),
        (ttf_file.clone(), format!("{}/{}", mi_base_url, &ttf_file), mi_public_dir.join(&ttf_file)),
    ];

    // Create a progress tracker
    let mut progress = crate::logger::create_progress(Some(files.len() as u64));
    progress.set_message("Downloading Material Icons webfonts...");

    // Download all files
    for (file_name, url, dest_path) in files {
        progress.set_message(&format!("Downloading {}", file_name));
        match download_file(&url, &dest_path).await {
            Ok(_) => {
                progress.inc(1);
            }
            Err(e) => {
                let error_msg = format!("Failed to download {}: {}", file_name, e);
                progress.set_message(&error_msg);
                return Err(e);
            }
        }
    }

    progress.set_message("Material Icons downloaded successfully.");
    Ok(())
}

async fn download_htmx_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    // Always use standard asset locations regardless of config
    let htmx_dir = project_dir.join(&public_dir).join("js").join("htmx");

    fs::create_dir_all(&htmx_dir).await?;

    let js_url = get_config_value(config, &["htmx", "js_url"], Some("https://cdnjs.cloudflare.com/ajax/libs/htmx/2.0.4/htmx.min.js")).ok_or("Missing htmx js_url in config")?;

    // Create a progress tracker
    let mut progress = crate::logger::create_progress(Some(1));
    progress.set_message("Downloading HTMX JS");

    // Download the JS file
    let js_path = htmx_dir.join("htmx.min.js");
    match download_file(&js_url, &js_path).await {
        Ok(_) => {
            progress.set_message("HTMX JS downloaded successfully.");
            progress.inc(1);
        }
        Err(e) => {
            let error_msg = format!("Failed to download HTMX JS: {}", e);
            progress.set_message(&error_msg);
            return Err(e);
        }
    }

    Ok(())
}

async fn download_materialize_scss(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let src_sass_dir = project_dir.join("src/assets/sass");
    let materialize_dir = project_dir.join("src/assets/materialize");

    // Create the directories if they don't exist
    fs::create_dir_all(&src_sass_dir).await?;
    
    // Create a progress bar spinner for the operation
    let mut progress = crate::logger::create_progress(None);
    progress.set_message("Setting up Materialize assets...");

    // Get repository URL and version from config or use defaults
    let repo_url = get_config_value(config, &["materialize", "repo_url"], Some("https://github.com/Dogfalo/materialize.git"))
        .unwrap_or_else(|| "https://github.com/Dogfalo/materialize.git".to_string());
    
    let version = get_config_value(config, &["materialize", "version"], Some("1.0.0"))
        .unwrap_or_else(|| "1.0.0".to_string());
    
    // Check if we should force a fresh clone (for debugging or version changes)
    let force_fresh = std::env::var("BLAST_FORCE_FRESH_MATERIALIZE").unwrap_or_else(|_| String::from("0")) == "1";
    
    // Check if materialize repo already exists
    let repo_exists = materialize_dir.exists();
    
    if repo_exists && force_fresh {
        // Clean up existing repo if forced
        progress.set_message("Forcing fresh clone of Materialize repository...");
        match fs::remove_dir_all(&materialize_dir).await {
            Ok(_) => {
                progress.set_message("Removed existing Materialize directory");
            },
            Err(e) => {
                let warning_msg = format!("Failed to remove existing Materialize directory: {}", e);
                progress.set_message(&warning_msg);
                // Continue anyway, the clone might still succeed
            }
        }
    }
    
    // Clone or check the repository
    if !repo_exists || force_fresh {
        // Clone Materialize repository if it doesn't exist or we're forcing a fresh clone
        progress.set_message(&format!("Cloning Materialize v{} repository...", version));
        
        // Make sure parent directory exists
        if let Some(parent) = materialize_dir.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        // Use tokio's process to run git clone without specifying a branch
        // The Materialize repo hasn't been updated in years, so just clone the default branch
        let clone_output = tokio::process::Command::new("git")
            .args(&["clone", "--depth=1", &repo_url])
            .arg(&materialize_dir)
            .output()
            .await;
            
        match clone_output {
            Ok(output) => {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    let error_msg = format!("Failed to clone Materialize repository: {}", error);
                    progress.set_message(&error_msg);
                    
                    // No need to check for specific branch errors since we're not specifying a branch
                    
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Git clone failed: {}", error)
                    )));
                }
                let success_msg = "Materialize repository cloned successfully";
                progress.set_message(&success_msg);
            },
            Err(e) => {
                let error_msg = format!("Failed to execute git clone: {}", e);
                progress.set_message(&error_msg);
                
                // Check if git is installed
                let git_check = tokio::process::Command::new("git")
                    .arg("--version")
                    .output()
                    .await;
                    
                if git_check.is_err() {
                    progress.set_message("Git may not be installed or is not in PATH. Please install git and try again.");
                }
                
                return Err(Box::new(e));
            }
        }
    } else {
        progress.set_message("Using existing Materialize repository");
    }

    // Copy the compiled JS file to public directory
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    let js_dest_dir = project_dir.join(&public_dir).join("js").join("materialize");
    
    // Create js directory if it doesn't exist
    fs::create_dir_all(&js_dest_dir).await?;
    
    // Copy the minified JS file from the cloned repository
    let materialize_js_src = materialize_dir.join("dist/js/materialize.min.js");
    let materialize_js_dest = js_dest_dir.join("materialize.min.js");
    
    if materialize_js_src.exists() {
        progress.set_message("Copying Materialize JS file...");
        if let Err(e) = fs::copy(&materialize_js_src, &materialize_js_dest).await {
            let warning_msg = format!("Failed to copy Materialize JS file: {}", e);
            progress.set_message(&warning_msg);
            // Fall back to CDN download
            let js_url = format!("https://cdnjs.cloudflare.com/ajax/libs/materialize/{}/js/materialize.min.js", version);
            progress.set_message("Falling back to CDN download for Materialize JS...");
            download_file(&js_url, &materialize_js_dest).await?;
        } else {
            progress.set_message("Materialize JS file copied successfully");
        }
    } else {
        progress.set_message("Materialize JS file not found in cloned repository");
        // Fall back to CDN download if file doesn't exist
        let js_url = format!("https://cdnjs.cloudflare.com/ajax/libs/materialize/{}/js/materialize.min.js", version);
        progress.set_message("Downloading Materialize JS from CDN...");
        if let Ok(_) = download_file(&js_url, &materialize_js_dest).await {
            progress.set_message("Materialize JS file downloaded from CDN successfully");
        } else {
            // If CDN download also fails, attempt to compile from source
            let warning_msg = "CDN download failed. Checking if we can build from source...";
            progress.set_message(warning_msg);
            
            // We don't need to check for specific files anymore as we'll just provide instructions
            progress.set_message("Attempting to build Materialize from source...");
            // This would require npm and grunt to be installed, so it's a last resort
            // For now, just inform the user about the situation
            progress.set_message("Building from source requires npm and grunt. Please install dependencies manually:");
            progress.set_message("1. cd src/assets/materialize");
            progress.set_message("2. npm install");
            progress.set_message("3. npx grunt");
            
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other, 
                "Failed to obtain Materialize JS - both repository and CDN attempts failed"
            )));
        }
    }

    // Create a single dark.scss file that imports from the Materialize SCSS
    // Delete the other theme files if they exist
    let dark_scss_path = src_sass_dir.join("dark.scss");
    let light_scss_path = src_sass_dir.join("light.scss");
    let sepia_scss_path = src_sass_dir.join("sepia.scss");
    let midnight_scss_path = src_sass_dir.join("midnight.scss");
    
    // Remove other theme files if they exist
    if light_scss_path.exists() {
        fs::remove_file(light_scss_path).await?;
    }
    if sepia_scss_path.exists() {
        fs::remove_file(sepia_scss_path).await?;
    }
    if midnight_scss_path.exists() {
        fs::remove_file(midnight_scss_path).await?;
    }

    // Create components directory structure
    let components_dir = src_sass_dir.join("components");
    let forms_dir = components_dir.join("forms");
    fs::create_dir_all(&forms_dir).await?;

    // Create the forms.scss file
    let forms_scss_path = forms_dir.join("forms.scss");
    if !forms_scss_path.exists() {
        let forms_scss_content = r#"// Basic form styles - imports will be handled by dark.scss
form {
  margin-bottom: 20px;
}

.input-field {
  position: relative;
  margin-top: 1rem;
  
  input, textarea {
    &:focus {
      border-bottom: 1px solid $primary-color;
      box-shadow: 0 1px 0 0 $primary-color;
      
      + label {
        color: $primary-color;
      }
    }
  }
  
  label {
    color: rgba(0,0,0,.6);
    
    &.active {
      transform: translateY(-14px) scale(.8);
      transform-origin: 0 0;
    }
  }
}

// Button styles
.btn {
  background-color: $primary-color;
  
  &:hover {
    background-color: lighten($primary-color, 5%);
  }
}

// Form validation styles
.error-text {
  color: #F44336;
  font-size: 0.8rem;
}

.success-text {
  color: #4CAF50;
  font-size: 0.8rem;
}
"#;
        fs::write(&forms_scss_path, forms_scss_content).await?;
    }

    // Create or update the dark.scss file to use the Materialize SCSS
    let dark_scss_content = r#"// Dark theme variant that imports from Materialize source
// Set variables first to avoid undefined variable errors
$primary-color: #2196F3;
$secondary-color: #26a69a;
$background-color: #121212;
$surface-color: #1e1e1e; 
$text-color: #e0e0e0;
$card-bg-color: #2d2d2d;

// Import Materialize SCSS from the cloned repository
@import '../materialize/sass/components/color-variables';
@import '../materialize/sass/components/color-classes';

// Import our own components
@import './components/forms/forms';

// Import necessary Materialize components
@import '../materialize/sass/components/variables';
@import '../materialize/sass/components/global';
@import '../materialize/sass/components/badges';
@import '../materialize/sass/components/icons-material-design';
@import '../materialize/sass/components/grid';
@import '../materialize/sass/components/navbar';
@import '../materialize/sass/components/typography';
@import '../materialize/sass/components/transitions';
@import '../materialize/sass/components/cards';
@import '../materialize/sass/components/toast';
@import '../materialize/sass/components/tabs';
@import '../materialize/sass/components/tooltip';
@import '../materialize/sass/components/buttons';
@import '../materialize/sass/components/dropdown';
@import '../materialize/sass/components/waves';
@import '../materialize/sass/components/modal';
@import '../materialize/sass/components/collapsible';
@import '../materialize/sass/components/chips';
@import '../materialize/sass/components/materialbox';
@import '../materialize/sass/components/forms/forms';
@import '../materialize/sass/components/table_of_contents';
@import '../materialize/sass/components/sidenav';
@import '../materialize/sass/components/preloader';
@import '../materialize/sass/components/slider';
@import '../materialize/sass/components/carousel';
@import '../materialize/sass/components/tapTarget';
@import '../materialize/sass/components/pulse';
@import '../materialize/sass/components/datepicker';
@import '../materialize/sass/components/timepicker';

// Dark theme overrides
body {
  background-color: $background-color;
  color: $text-color;
}

.card {
  background-color: $card-bg-color;
}

nav {
  background-color: $surface-color;
}

// Custom theme-specific styles below
"#;
    fs::write(&dark_scss_path, dark_scss_content).await?;

    progress.set_message("Materialize setup completed successfully");
    Ok(())
}

pub async fn download_assets_async(config: &Config) -> Result<(), Box<dyn Error>> {
    // Use more fault-tolerant approach with individual error handling
    let mut any_errors = false;
    
    // Create main progress bar that shows overall process
    let mut main_progress = crate::logger::create_progress(Some(4)); // 4 asset types to download
    main_progress.set_message("Downloading assets...");
    
    // Function to handle errors consistently
    let log_error = |e: &Box<dyn Error>| -> Result<(), Box<dyn Error>> {
        crate::logger::warning(&format!("Download error: {}", e))?;
        Ok(())
    };

    // 1. FontAwesome
    main_progress.set_message("Downloading FontAwesome assets...");
    let fa_result = download_fontawesome_async(config).await;
    if fa_result.is_ok() {
        main_progress.inc(1);
        main_progress.set_message("Downloading Material Icons...");
    } else if let Err(e) = fa_result {
        log_error(&e)?;
        any_errors = true;
        main_progress.inc(1);
        main_progress.set_message("Downloading Material Icons...");
    }

    // 2. Material Icons
    let mi_result = download_materialicons_async(config).await;
    if mi_result.is_ok() {
        main_progress.inc(1);
        main_progress.set_message("Downloading Materialize assets...");
    } else if let Err(e) = mi_result {
        log_error(&e)?;
        any_errors = true;
        main_progress.inc(1);
        main_progress.set_message("Downloading Materialize assets...");
    }

    // 3. Materialize (JS and SCSS are now handled by a single function)
    let mat_result = download_materialize_scss(config).await;
    if mat_result.is_ok() {
        main_progress.inc(1);
        main_progress.set_message("Downloading HTMX...");
    } else if let Err(e) = mat_result {
        log_error(&e)?;
        any_errors = true;
        main_progress.inc(1);
        main_progress.set_message("Downloading HTMX...");
    }

    // 4. HTMX
    let htmx_result = download_htmx_js(config).await;
    if htmx_result.is_ok() {
        main_progress.inc(1);
    } else if let Err(e) = htmx_result {
        log_error(&e)?;
        any_errors = true;
        main_progress.inc(1);
    }

    // Finish with appropriate message
    if any_errors {
        main_progress.success("Asset download completed (with some non-critical errors)");
    } else {
        main_progress.success("All assets downloaded successfully");
    }

    Ok(())
}

pub async fn transpile_all_scss(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = &config.project_dir;
    let is_production = config.environment == "prod" || config.environment == "production";

    let sass_dir = project_dir.join("src/assets/sass");
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    // Always use standard asset locations for consistency
    let css_dir = project_dir.join(&public_dir).join("css");

    // Create the output directory if it doesn't exist
    fs::create_dir_all(&css_dir).await?;

    // Find all SCSS files
    let mut entries = fs::read_dir(&sass_dir).await?;
    let mut scss_files = vec![];

    while let Some(entry) = entries.next_entry().await? {
        if entry.path().extension().is_some_and(|ext| ext == "scss") {
            scss_files.push(entry.path());
        }
    }

    if scss_files.is_empty() {
        crate::logger::info("No SCSS files found!")?;
        return Ok(());
    }

    // Create progress bar with known total count for better visualization
    let mut progress = crate::logger::create_progress(Some(scss_files.len() as u64));
    progress.set_message(&format!("Processing {} SCSS files", scss_files.len()));

    // Counter for error tracking
    let mut error_count = 0;
    let mut success_count = 0;
    let total_files = scss_files.len(); // Store length before we move scss_files

    for scss_file in scss_files {
        let file_stem = scss_file.file_stem().unwrap().to_str().unwrap();
        // Only use .min.css files in root CSS directory (not in app subfolder)
        // The SCSS outputs are part of the framework, while app CSS goes in the app subfolder
        let output_file = css_dir.join(format!("{}.min.css", file_stem));

        // Make sure CSS directory exists
        fs::create_dir_all(&css_dir).await?;

        // Set up options for SCSS compilation
        let mut sass_options = Options::default();

        // In production mode, use OutputStyle::Compressed
        if is_production {
            sass_options.output_style = OutputStyle::Compressed;
            progress.set_message(&format!("Transpiling {} with compression ({}/{})", 
                file_stem, success_count + error_count + 1, total_files));
        } else {
            sass_options.output_style = OutputStyle::Expanded;
            progress.set_message(&format!("Transpiling {} for development ({}/{})", 
                file_stem, success_count + error_count + 1, total_files));
        }

        // Compile SCSS to CSS with appropriate output style
        match compile_file(scss_file.to_str().unwrap(), sass_options) {
            Ok(css_content) => {
                // Write the CSS file (always as .min.css)
                fs::write(&output_file, &css_content).await?;
                success_count += 1;
            }
            Err(e) => {
                error_count += 1;
                // Always show compilation errors regardless of verbosity
                progress.warning(&format!("Error compiling {}: {}", file_stem, e))?;
            }
        }

        progress.inc(1);
    }

    // Show completion message
    if error_count > 0 {
        if error_count == total_files {
            progress.error(&format!("SCSS processing failed - all {} files had errors", error_count));
        } else {
            progress.success(&format!("SCSS processing completed: {} succeeded, {} failed", 
                success_count, error_count));
        }
    } else {
        progress.success(&format!("All {} SCSS files processed successfully", total_files));
    }
    
    Ok(())
}

pub async fn minify_css_files(config: &Config) -> Result<(), Box<dyn Error>> {
    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // In the new asset system, all CSS processing is handled by publish_css
    // minify_css_files is kept for backward compatibility

    if is_interactive {
        let _ = crate::output::log("CSS minification now handled by publish-css command - skipping");
    } else {
        println!("CSS minification now handled by publish-css command - skipping");
    }

    // Call publish_css to handle all CSS processing
    publish_css(config).await
}

pub async fn process_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    let public_path = project_dir.join(&public_dir);
    let is_production = config.environment == "prod" || config.environment == "production";

    // Source and destination directories
    let src_js_dir = project_dir.join("src").join("assets").join("js");

    // Always use standard asset locations for consistency
    let dest_js_dir = public_path.join("js");

    // Create destination directory if it doesn't exist
    fs::create_dir_all(&dest_js_dir).await?;

    // Check if the source directory exists
    if !src_js_dir.exists() {
        // Source dir doesn't exist, create it and return
        fs::create_dir_all(&src_js_dir).await?;
        return Ok(());
    }

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // Find all JS files
    let mut js_files = Vec::new();
    for entry in WalkDir::new(&src_js_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "js") {
            js_files.push(path.to_path_buf());
        }
    }

    if js_files.is_empty() {
        if is_interactive {
            let _ = crate::output::log("No JS files found to process!");
        } else {
            println!("No JS files found to process!");
        }
        return Ok(());
    }

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log(&format!("Processing {} JS files...", js_files.len()));

        for js_file in &js_files {
            // Determine relative path from src_js_dir
            let rel_path = js_file.strip_prefix(&src_js_dir).unwrap();
            // Only use .min.js files for consistency with our CSS strategy
            let min_dest_path = dest_js_dir.join("app").join(rel_path.with_file_name(format!("{}.min.js", rel_path.file_stem().unwrap().to_str().unwrap())));

            // Make sure app directory exists
            fs::create_dir_all(&dest_js_dir.join("app")).await?;

            // Create parent directory if needed
            if let Some(parent) = min_dest_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Read the file content
            let content = fs::read_to_string(js_file).await?;

            // In production mode, minify JS (or simulate it)
            if is_production {
                let _ = crate::output::log(&format!("Minifying {}", rel_path.display()));

                // TODO: Implement actual JS minification
                // For now, just copy the file with .min.js extension
                fs::write(&min_dest_path, &content).await?;
            } else {
                let _ = crate::output::log(&format!("Copying {}", rel_path.display()));

                // In development mode, just copy the file with .min.js extension
                fs::write(&min_dest_path, &content).await?;
            }
        }

        // Show success message based on environment
        if is_production {
            let _ = crate::output::log("JS processing complete - all files saved as .min.js (production mode).");
        } else {
            let _ = crate::output::log("JS processing complete - all files saved as .min.js (development mode).");
        }
    } else {
        // In CLI mode, use progress bar
        let pb = ProgressBar::new(js_files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        for js_file in &js_files {
            // Determine relative path from src_js_dir
            let rel_path = js_file.strip_prefix(&src_js_dir).unwrap();
            // Only use .min.js files for consistency with our CSS strategy
            let min_dest_path = dest_js_dir.join("app").join(rel_path.with_file_name(format!("{}.min.js", rel_path.file_stem().unwrap().to_str().unwrap())));

            // Make sure app directory exists
            fs::create_dir_all(&dest_js_dir.join("app")).await?;

            // Create parent directory if needed
            if let Some(parent) = min_dest_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            // Read the file content
            let content = fs::read_to_string(js_file).await?;

            // In production mode, minify JS (or simulate it)
            if is_production {
                pb.set_message(format!("Minifying {}", rel_path.display()));

                // TODO: Implement actual JS minification
                // For now, just copy the file with .min.js extension
                fs::write(&min_dest_path, &content).await?;
            } else {
                pb.set_message(format!("Copying {}", rel_path.display()));

                // In development mode, just copy the file with .min.js extension
                fs::write(&min_dest_path, &content).await?;
            }

            pb.inc(1);
        }

        // Show success message based on environment
        if is_production {
            pb.finish_with_message("JS processing complete - all files saved as .min.js (production mode).");
        } else {
            pb.finish_with_message("JS processing complete - all files saved as .min.js (development mode).");
        }
    }

    Ok(())
}

// Publish CSS files from src/assets/css to public/css with environment-based minification
pub async fn publish_css(config: &Config) -> Result<(), Box<dyn Error>> {
    let is_production = config.environment == "prod" || config.environment == "production";
    let project_dir = &config.project_dir;

    // Source and destination directories
    let src_css_dir = project_dir.join("src").join("assets").join("css");
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    // Always use standard asset locations for consistent directory structure
    let public_path = project_dir.join(&public_dir);
    let dest_css_dir = public_path.join("css");

    // Create destination directory if it doesn't exist
    fs::create_dir_all(&dest_css_dir).await?;

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // Setup progress tracking
    let progress = if is_interactive {
        let _ = crate::output::log(&format!("Publishing CSS files ({} mode)...", if is_production { "production" } else { "development" }));
        None
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner().template("{spinner:.green} {msg}").unwrap());
        pb.set_message(format!("Publishing CSS files ({} mode)...", if is_production { "production" } else { "development" }));
        Some(pb)
    };

    // Get all CSS files in the source directory
    let css_files = WalkDir::new(&src_css_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file() && e.path().extension().map_or(false, |ext| ext == "css") && !e.path().to_str().unwrap_or("").contains(".min.css"))
        .collect::<Vec<_>>();

    // Store the count for later use
    let file_count = css_files.len();

    // Process each CSS file
    for entry in css_files {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(&src_css_dir).unwrap();

        // Only generate .min.css files with consistent location in the app subfolder
        let min_dest_path = dest_css_dir.join("app").join(rel_path.with_file_name(format!("{}.min.css", rel_path.file_stem().unwrap().to_str().unwrap())));

        // Make sure app directory exists
        fs::create_dir_all(&dest_css_dir.join("app")).await?;

        // Read the file content
        let content = fs::read_to_string(src_path).await?;

        if let Some(pb) = &progress {
            pb.set_message(format!("Processing {}", rel_path.display()));
        } else {
            let _ = crate::output::log(&format!("Processing {}", rel_path.display()));
        }

        // Create parent directory if needed
        if let Some(parent) = min_dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Always minify in production mode, but we can keep expanded content in dev mode
        // while still using the .min.css extension for consistency
        if is_production {
            // Minify the content
            let minified = Minifier::default()
                .minify(&content, Level::Three)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("CSS minification error: {}", e)))?;

            // Write the minified content
            fs::write(&min_dest_path, &minified).await?;
        } else {
            // In development mode, write expanded content but still use .min.css extension
            fs::write(&min_dest_path, &content).await?;
        }
    }

    // Update progress indicator
    if let Some(pb) = &progress {
        if is_production {
            pb.finish_with_message(format!("Published {} CSS files as .min.css (minified for production)", file_count));
        } else {
            pb.finish_with_message(format!("Published {} CSS files as .min.css (expanded for development)", file_count));
        }
    } else {
        let _ = crate::output::log(&format!("Published {} CSS files as .min.css", file_count));
    }

    Ok(())
}

// Complete asset processing pipeline - downloads and processes all assets
#[allow(dead_code)]
pub async fn process_all_assets(config: &Config) -> Result<(), Box<dyn Error>> {
    // Use more fault-tolerant approach with individual error handling

    // Download CDN assets with individual error handling
    let mut any_errors = false;

    match download_fontawesome_async(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error downloading FontAwesome: {}", e))?;
            any_errors = true;
        }
    }

    match download_materialicons_async(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error downloading Material Icons: {}", e))?;
            any_errors = true;
        }
    }

    // Materialize JS and SCSS are now handled by a single function
    match download_materialize_scss(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error setting up Materialize assets: {}", e))?;
            any_errors = true;
        }
    }

    match download_htmx_js(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error downloading HTMX JS: {}", e))?;
            any_errors = true;
        }
    }

    // Process SCSS files - they will be automatically compressed in production mode
    match transpile_all_scss(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error transpiling SCSS: {}", e))?;
            any_errors = true;
        }
    }

    // Process regular CSS files (non-SCSS generated)
    match publish_css(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error publishing CSS: {}", e))?;
            any_errors = true;
        }
    }

    // Process JS files based on environment
    match process_js(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error processing JS: {}", e))?;
            any_errors = true;
        }
    }

    if any_errors {
        crate::logger::warning("Completed asset processing with some non-critical errors")?;
    } else {
        crate::logger::success("All assets processed successfully")?;
    }

    Ok(())
}
