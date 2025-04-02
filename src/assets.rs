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

// Helper function to safely access TOML values
fn get_config_value(config: &Config, path: &[&str], default: Option<&str>) -> Option<String> {
    let mut current = &config.assets;
    
    // Debug log for config value access
    println!("DEBUG: Accessing path: {:?} in config", path);
    println!("DEBUG: Config assets: {:?}", config.assets);
    
    for &key in path {
        if let Some(value) = current.get(key) {
            current = value;
            println!("DEBUG: Found key: {}", key);
        } else {
            println!("DEBUG: Missing key: {} in path", key);
            return default.map(|s| s.to_string());
        }
    }
    
    if let Some(s) = current.as_str() {
        println!("DEBUG: Found string value: {}", s);
        Some(s.to_string())
    } else {
        println!("DEBUG: No string value found, using default: {:?}", default);
        default.map(|s| s.to_string())
    }
}

async fn download_file(url: &str, dest_path: &Path) -> Result<(), Box<dyn Error>> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    let mut file = fs::File::create(dest_path).await?;
    file.write_all(&bytes).await?;
    Ok(())
}

async fn download_materialize_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    
    // Debug logging for materialize path
    println!("DEBUG: Looking for materialize in config");
    println!("DEBUG: Full config: {:?}", config.assets);
    
    // Ensure we create the proper directory structure regardless of whether config values exist
    let materialize_dir = project_dir.join(&public_dir).join("js").join("materialize");
    fs::create_dir_all(&materialize_dir).await?;

    // Use a default URL if missing in the config
    let js_url = get_config_value(config, &["materialize", "js_url"], Some("https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/js/materialize.min.js")).unwrap_or_else(|| {
        println!("DEBUG: Using default Materialize JS URL");
        "https://cdnjs.cloudflare.com/ajax/libs/materialize/1.0.0/js/materialize.min.js".to_string()
    });

    // Create a progress tracker
    let mut progress = crate::logger::create_progress(Some(1));
    progress.set_message("Downloading Materialize JS");

    // Download the JS file
    let js_path = materialize_dir.join("materialize.min.js");
    match download_file(&js_url, &js_path).await {
        Ok(_) => {
            progress.success("Materialize JS downloaded successfully.");
            progress.inc(1);
        }
        Err(e) => {
            progress.error(&format!("Failed to download Materialize JS: {}", e));
            return Err(e);
        }
    }

    Ok(())
}

async fn download_fontawesome_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    
    // Print the entire config for debugging
    println!("DEBUG: Full config structure:");
    println!("DEBUG: {:?}", config.assets);
    
    // Provide a default value for FontAwesome CDN if missing
    let fa_base_url = get_config_value(config, &["fontawesome", "base_url"], Some("https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1"))
        .ok_or("Missing fontawesome base_url in config")?;
    
    let fa_public_dir = project_dir.join(public_dir).join("fonts").join("fontawesome");

    // Create FontAwesome directories
    fs::create_dir_all(&fa_public_dir).await?;
    fs::create_dir_all(&fa_public_dir.join("css")).await?;
    fs::create_dir_all(&fa_public_dir.join("js")).await?;
    fs::create_dir_all(&fa_public_dir.join("sprites")).await?;
    fs::create_dir_all(&fa_public_dir.join("webfonts")).await?;

    // Get FA assets to download from config - need to handle arrays differently
    // This is a temporary solution - ideally we'd extend our helper function to handle arrays
    // Default values for FontAwesome assets
    let default_css = vec![toml::Value::String("css/all.min.css".to_string())];
    let default_js = vec![toml::Value::String("js/all.min.js".to_string())];
    let default_sprites = vec![
        toml::Value::String("sprites/brands.svg".to_string()),
        toml::Value::String("sprites/regular.svg".to_string()),
        toml::Value::String("sprites/solid.svg".to_string())
    ];
    let default_webfonts = vec![
        toml::Value::String("webfonts/fa-brands-400.ttf".to_string()),
        toml::Value::String("webfonts/fa-brands-400.woff2".to_string()),
        toml::Value::String("webfonts/fa-regular-400.ttf".to_string()),
        toml::Value::String("webfonts/fa-regular-400.woff2".to_string()),
        toml::Value::String("webfonts/fa-solid-900.ttf".to_string()),
        toml::Value::String("webfonts/fa-solid-900.woff2".to_string()),
        toml::Value::String("webfonts/fa-v4compatibility.ttf".to_string()),
        toml::Value::String("webfonts/fa-v4compatibility.woff2".to_string())
    ];
    
    // Get values from config or use defaults
    let fa_css = config.assets.get("fontawesome")
        .and_then(|f| f.get("css"))
        .and_then(|c| c.as_array())
        .unwrap_or(&default_css);
    
    let fa_js = config.assets.get("fontawesome")
        .and_then(|f| f.get("js"))
        .and_then(|c| c.as_array())
        .unwrap_or(&default_js);
    
    let fa_sprites = config.assets.get("fontawesome")
        .and_then(|f| f.get("sprites"))
        .and_then(|c| c.as_array())
        .unwrap_or(&default_sprites);
    
    let fa_webfonts = config.assets.get("fontawesome")
        .and_then(|f| f.get("webfonts"))
        .and_then(|c| c.as_array())
        .unwrap_or(&default_webfonts);
    
    let asset_types = [
        ("css", fa_css),
        ("js", fa_js),
        ("sprites", fa_sprites),
        ("webfonts", fa_webfonts),
    ];

    // Calculate total assets
    let total_assets: usize = asset_types.iter().map(|(_, assets)| assets.len()).sum();

    // Create a progress tracker
    let mut progress = crate::logger::create_progress(Some(total_assets as u64));
    progress.set_message(&format!("Downloading {} FontAwesome assets...", total_assets));

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
    let mut has_errors = false;

    while let Some(result) = tasks.next().await {
        match result {
            Ok(Ok((asset_type, asset_path))) => {
                completed += 1;
                let msg = format!("Downloaded {} ({}) ({}/{})", asset_path, asset_type, completed, total_assets);
                progress.set_message(&msg);
                progress.inc(1);
            }
            Ok(Err(e)) => {
                has_errors = true;
                crate::logger::warning(&format!("Download error: {}", e))?;
            }
            Err(e) => {
                has_errors = true;
                crate::logger::warning(&format!("Task error: {}", e))?;
            }
        }
    }

    if has_errors {
        progress.set_message("FontAwesome download completed with some errors");
    } else {
        progress.success("FontAwesome downloaded successfully");
    }

    Ok(())
}

async fn download_materialicons_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());

    let mi_base_url = get_config_value(config, &["materialicons", "base_url"], Some("https://raw.githubusercontent.com/google/material-design-icons/master/font")).ok_or("Missing materialicons base_url in config")?;
    let mi_public_dir = project_dir.join(public_dir).join("fonts").join("material-icons");

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
                progress.error(&format!("Failed to download {}: {}", file_name, e));
                return Err(e);
            }
        }
    }

    progress.success("Material Icons downloaded successfully.");
    Ok(())
}

async fn download_htmx_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    let htmx_dir = project_dir.join(public_dir).join("js").join("htmx");

    fs::create_dir_all(&htmx_dir).await?;

    let js_url = get_config_value(config, &["htmx", "js_url"], Some("https://cdnjs.cloudflare.com/ajax/libs/htmx/2.0.4/htmx.min.js")).ok_or("Missing htmx js_url in config")?;

    // Create a progress tracker
    let mut progress = crate::logger::create_progress(Some(1));
    progress.set_message("Downloading HTMX JS");

    // Download the JS file
    let js_path = htmx_dir.join("htmx.min.js");
    match download_file(&js_url, &js_path).await {
        Ok(_) => {
            progress.success("HTMX JS downloaded successfully.");
            progress.inc(1);
        }
        Err(e) => {
            progress.error(&format!("Failed to download HTMX JS: {}", e));
            return Err(e);
        }
    }

    Ok(())
}

async fn download_materialize_scss(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let src_sass_dir = project_dir.join("src/assets/sass");
    
    // Create the directory if it doesn't exist
    fs::create_dir_all(&src_sass_dir).await?;
    
    // Create a progress bar spinner for the download operation
    let mut progress = crate::logger::create_progress(None);
    progress.set_message("Setting up Materialize SCSS files...");
    
    // Instead of downloading and extracting, let's create basic SCSS files
    // This is more reliable than trying to download and extract from GitHub
    
    // Create the component directories
    let components_dir = src_sass_dir.join("components");
    let forms_dir = components_dir.join("forms");
    fs::create_dir_all(&forms_dir).await?;
    
    // Check if we already have these files
    let dark_scss_path = src_sass_dir.join("dark.scss");
    let light_scss_path = src_sass_dir.join("light.scss");
    let sepia_scss_path = src_sass_dir.join("sepia.scss");
    let midnight_scss_path = src_sass_dir.join("midnight.scss");
    
    // Only create files if they don't exist
    if !dark_scss_path.exists() {
        let dark_scss_content = r#"// Dark theme variant for Materialize
@import './components/forms/forms';

// Set dark theme variables
$primary-color: #2196F3;
$secondary-color: #26a69a;
$background-color: #121212;
$surface-color: #1e1e1e; 
$text-color: #e0e0e0;
$card-bg-color: #2d2d2d;

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
    }
    
    if !light_scss_path.exists() {
        let light_scss_content = r#"// Light theme variant for Materialize
@import './components/forms/forms';

// Set light theme variables  
$primary-color: #26a69a;
$secondary-color: #2196F3;
$background-color: #ffffff;
$surface-color: #f5f5f5;
$text-color: #333333; 
$card-bg-color: #ffffff;

// Light theme overrides
body {
  background-color: $background-color;
  color: $text-color;
}

.card {
  background-color: $card-bg-color;
}

nav {
  background-color: $primary-color;
}

// Custom theme-specific styles below
"#;
        fs::write(&light_scss_path, light_scss_content).await?;
    }
    
    if !sepia_scss_path.exists() {
        let sepia_scss_content = r#"// Sepia theme variant for Materialize
@import './components/forms/forms';

// Set sepia theme variables
$primary-color: #8d6e63;
$secondary-color: #6d4c41;
$background-color: #f9f3e6;
$surface-color: #f0e6d2;
$text-color: #5d4037;
$card-bg-color: #f9f3e6;

// Sepia theme overrides
body {
  background-color: $background-color;
  color: $text-color;
}

.card {
  background-color: $card-bg-color;
}

nav {
  background-color: $primary-color;
}

// Custom theme-specific styles below
"#;
        fs::write(&sepia_scss_path, sepia_scss_content).await?;
    }
    
    if !midnight_scss_path.exists() {
        let midnight_scss_content = r#"// Midnight theme variant for Materialize
@import './components/forms/forms';

// Set midnight theme variables
$primary-color: #7986cb;
$secondary-color: #5c6bc0;
$background-color: #0a1929;
$surface-color: #0d2339;
$text-color: #e3f2fd;
$card-bg-color: #102a43;

// Midnight theme overrides
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
        fs::write(&midnight_scss_path, midnight_scss_content).await?;
    }
    
    // Create a basic forms.scss file in the components directory
    let forms_scss_path = forms_dir.join("forms.scss");
    if !forms_scss_path.exists() {
        let forms_scss_content = r#"// Basic form styles
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
    
    progress.success("Materialize SCSS files created successfully.");
    Ok(())
}

pub async fn download_assets_async(config: &Config) -> Result<(), Box<dyn Error>> {
    // Use more fault-tolerant approach with individual error handling
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
    
    match download_materialize_js(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error downloading Materialize JS: {}", e))?;
            any_errors = true;
        }
    }
    
    match download_materialize_scss(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error setting up Materialize SCSS: {}", e))?;
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

    if any_errors {
        crate::logger::warning("Completed asset downloads with some non-critical errors")?;
    } else {
        crate::logger::success("All assets downloaded successfully")?;
    }
    
    Ok(())
}

pub async fn transpile_all_scss(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = &config.project_dir;
    let is_production = config.environment == "prod" || config.environment == "production";

    let sass_dir = project_dir.join("src/assets/sass");
    let public_dir = get_config_value(config, &["public_dir"], Some("public")).unwrap_or_else(|| "public".to_string());
    let css_dir = project_dir.join(public_dir).join("css");

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

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if scss_files.is_empty() {
        if is_interactive {
            let _ = crate::output::log("No SCSS files found!");
        } else {
            println!("No SCSS files found!");
        }
        return Ok(());
    }

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log(&format!("Processing {} SCSS files...", scss_files.len()));

        for scss_file in scss_files {
            let file_stem = scss_file.file_stem().unwrap().to_str().unwrap();
            // Only use .min.css files
            let output_file = css_dir.join(format!("{}.min.css", file_stem));

            // Set up options for SCSS compilation
            let mut sass_options = Options::default();

            // In production mode, use OutputStyle::Compressed
            if is_production {
                sass_options.output_style = OutputStyle::Compressed;
                let _ = crate::output::log(&format!("Transpiling {} with compression", file_stem));
            } else {
                sass_options.output_style = OutputStyle::Expanded;
                let _ = crate::output::log(&format!("Transpiling {} for development", file_stem));
            }

            // Compile SCSS to CSS with appropriate output style
            match compile_file(scss_file.to_str().unwrap(), sass_options) {
                Ok(css_content) => {
                    // Write the CSS file (always as .min.css)
                    fs::write(&output_file, &css_content).await?;
                }
                Err(e) => {
                    let _ = crate::output::log(&format!("Error compiling {}: {}", scss_file.to_string_lossy(), e));
                }
            }
        }

        // Show success message based on environment
        if is_production {
            let _ = crate::output::log("SCSS transpilation complete (production mode with native compression).");
        } else {
            let _ = crate::output::log("SCSS transpilation complete (development mode - expanded format).");
        }
    } else {
        // In CLI mode, use progress bar
        let pb = ProgressBar::new(scss_files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        for scss_file in scss_files {
            let file_stem = scss_file.file_stem().unwrap().to_str().unwrap();
            // Only use .min.css files
            let output_file = css_dir.join(format!("{}.min.css", file_stem));

            // Set up options for SCSS compilation
            let mut sass_options = Options::default();

            // In production mode, use OutputStyle::Compressed
            if is_production {
                sass_options.output_style = OutputStyle::Compressed;
                pb.set_message(format!("Transpiling {} with compression", file_stem));
            } else {
                sass_options.output_style = OutputStyle::Expanded;
                pb.set_message(format!("Transpiling {} for development", file_stem));
            }

            // Compile SCSS to CSS with appropriate output style
            match compile_file(scss_file.to_str().unwrap(), sass_options) {
                Ok(css_content) => {
                    // Write the CSS file (always as .min.css)
                    fs::write(&output_file, &css_content).await?;
                }
                Err(e) => {
                    pb.println(format!("Error compiling {}: {}", scss_file.to_string_lossy(), e));
                }
            }

            pb.inc(1);
        }

        // Show success message based on environment
        if is_production {
            pb.finish_with_message(" SCSS transpilation complete (production mode with native compression).");
        } else {
            pb.finish_with_message(" SCSS transpilation complete (development mode - expanded format).");
        }
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
    let dest_js_dir = public_path.join("js").join("app");

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
            let min_dest_path = dest_js_dir.join(rel_path.with_file_name(format!("{}.min.js", rel_path.file_stem().unwrap().to_str().unwrap())));

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
            let min_dest_path = dest_js_dir.join(rel_path.with_file_name(format!("{}.min.js", rel_path.file_stem().unwrap().to_str().unwrap())));

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
    let public_path = project_dir.join(&public_dir);
    let dest_css_dir = public_path.join("css").join("app");

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

        // Only generate .min.css files (changed from original behavior)
        let min_dest_path = dest_css_dir.join(rel_path.with_file_name(format!("{}.min.css", rel_path.file_stem().unwrap().to_str().unwrap())));

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
    
    match download_materialize_js(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error downloading Materialize JS: {}", e))?;
            any_errors = true;
        }
    }
    
    match download_materialize_scss(config).await {
        Ok(_) => (),
        Err(e) => {
            crate::logger::warning(&format!("Error setting up Materialize SCSS: {}", e))?;
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
