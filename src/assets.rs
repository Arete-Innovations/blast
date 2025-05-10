use crate::configs::Config;
use css_minify::optimizations::{Level, Minifier};
// Remove unused imports
// Removed unused imports
use sass_rs::{compile_file, Options, OutputStyle};
use std::path::Path;
use std::io::Write;
// No sync primitives needed anymore
// No longer using tokio for file operations
use walkdir::WalkDir;

// Helper function to get public_dir with fallback
fn get_public_dir(config: &Config) -> &str {
    // Get from config or use default
    config.assets.get("public_dir")
        .and_then(|v| v.as_str())
        .unwrap_or("public")
}

fn download_file(url: &str, dest_path: &Path) -> Result<(), String> {
    let response = reqwest::blocking::get(url).map_err(|e| e.to_string())?;
    let bytes = response.bytes().map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(dest_path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn download_htmx_js(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let public_dir = get_public_dir(config);

    // Get the HTMX config section
    let htmx_section = &config.assets["assets"]["htmx"];

    // Create standard directory
    let htmx_dir = project_dir.join(public_dir).join("js").join("htmx");
    std::fs::create_dir_all(&htmx_dir).map_err(|e| e.to_string())?;

    // Get the JS URL
    let js_url = htmx_section["js_url"].as_str()
        .ok_or_else(|| "Missing htmx js_url in config")?;

    crate::logger::info("Downloading HTMX JS...")?;

    // Download the JS file
    let js_path = htmx_dir.join("htmx.min.js");
    download_file(&js_url, &js_path)?;

    Ok(())
}

pub fn download_assets(config: &Config) -> Result<(), String> {
    // Use fresh config to ensure we have the latest settings
    let fresh_config = crate::configs::get_fresh_config(&config.project_dir).map_err(|e| e.to_string())?;

    // Verify required config sections exist
    let assets = &fresh_config.assets;
    if !assets.as_table().map_or(false, |t| t.contains_key("assets")) {
        return Err("Missing [assets] section in Catalyst.toml".into());
    }

    crate::logger::info("Downloading CDN assets...")?;

    // Simple linear download of all assets - now only HTMX
    let asset_downloads = [
        ("HTMX", download_htmx_js(&fresh_config)),
    ];

    let mut success_count = 0;
    let total_count = asset_downloads.len();

    // Process results
    for (name, result) in asset_downloads.iter() {
        match result {
            Ok(_) => {
                crate::logger::success(&format!("{} downloaded successfully", name))?;
                success_count += 1;
            },
            Err(e) => {
                crate::logger::error(&format!("{} download failed: {}", name, e))?;
            }
        }
    }

    // Report overall status
    if success_count < total_count {
        crate::logger::warning(&format!("CDN downloads: {}/{} assets completed successfully", success_count, total_count))?;
    } else {
        crate::logger::success("All CDN assets downloaded successfully")?;
    }

    Ok(())
}

pub fn transpile_all_scss(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let is_production = config.environment == "prod" || config.environment == "production";
    let sass_dir = project_dir.join("src/assets/sass");
    let public_dir = get_public_dir(config);
    let css_app_dir = project_dir.join(&public_dir).join("css").join("app");

    // Create directories
    std::fs::create_dir_all(&css_app_dir).map_err(|e| e.to_string())?;

    // Check if sass directory exists and create it if needed
    if !sass_dir.exists() {
        std::fs::create_dir_all(&sass_dir).map_err(|e| e.to_string())?;
        crate::logger::info("Created SCSS directory (no files to process)").map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Find all SCSS files
    let entries = std::fs::read_dir(&sass_dir).map_err(|e| e.to_string())?;
    let mut scss_files = vec![];
    let mut file_count = 0;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().extension().is_some_and(|ext| ext == "scss") {
            scss_files.push(entry.path());
            file_count += 1;
        }
    }

    if scss_files.is_empty() {
        crate::logger::info("No SCSS files found!").map_err(|e| e.to_string())?;
        return Ok(());
    }

    crate::logger::info(&format!("Transpiling {} SCSS files", file_count)).map_err(|e| e.to_string())?;

    // Counter for error tracking
    let mut error_count = 0;
    let mut success_count = 0;

    // Process each file
    for scss_file in &scss_files {
        let file_stem = scss_file.file_stem().unwrap().to_str().unwrap();
        // Put all SCSS files in the app directory with .min.css extension
        let output_file = css_app_dir.join(format!("{}.min.css", file_stem));

        crate::logger::debug(&format!("Transpiling {} to {}", scss_file.display(), output_file.display()))?;

        // Setup SCSS compilation options (create new options for each file)
        let mut sass_options = Options::default();
        if is_production {
            sass_options.output_style = OutputStyle::Compressed;
        } else {
            sass_options.output_style = OutputStyle::Expanded;
        }

        // Compile SCSS to CSS
        match compile_file(scss_file.to_str().unwrap(), sass_options) {
            Ok(css_content) => {
                // Write the CSS file (always as .min.css)
                std::fs::write(&output_file, &css_content).map_err(|e| e.to_string())?;
                success_count += 1;
            }
            Err(e) => {
                error_count += 1;
                crate::logger::warning(&format!("Error compiling {}: {}", file_stem, e)).map_err(|e| e.to_string())?;
            }
        }
    }

    // Show completion message
    if error_count > 0 {
        if error_count == file_count {
            crate::logger::error(&format!("SCSS processing failed - all {} files had errors", error_count)).map_err(|e| e.to_string())?;
        } else {
            crate::logger::warning(&format!("SCSS processing completed: {} succeeded, {} failed",
                success_count, error_count)).map_err(|e| e.to_string())?;
        }
    } else {
        crate::logger::success(&format!("All {} SCSS files processed successfully", file_count)).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn minify_css_files(config: &Config) -> Result<(), String> {
    // In the new asset system, all CSS processing is handled by publish_css
    crate::logger::info("CSS minification now handled by publish-css command").map_err(|e| e.to_string())?;
    
    // Forward to the new implementation
    publish_css(config)
}

pub fn process_js(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let public_dir = get_public_dir(config);
    let public_path = project_dir.join(&public_dir);
    let is_production = config.environment == "prod" || config.environment == "production";

    // Source and destination directories
    let src_js_dir = project_dir.join("src").join("assets").join("js");
    let dest_js_dir = public_path.join("js");

    // Create directories
    std::fs::create_dir_all(&dest_js_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&src_js_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dest_js_dir.join("app")).map_err(|e| e.to_string())?;

    // Check if the source directory exists and return early if no files
    if !src_js_dir.exists() {
        crate::logger::info("No JS source directory found, created empty directory").map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Find all JS files
    let mut js_files = Vec::new();
    for entry in WalkDir::new(&src_js_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "js") {
            js_files.push(path.to_path_buf());
        }
    }

    if js_files.is_empty() {
        crate::logger::info("No JS files found to process!").map_err(|e| e.to_string())?;
        return Ok(());
    }

    crate::logger::info(&format!("Processing {} JS files...", js_files.len())).map_err(|e| e.to_string())?;

    // Process each file
    for js_file in &js_files {
        // Get relative path and create destination path
        let rel_path = js_file.strip_prefix(&src_js_dir).unwrap();
        let min_dest_path = dest_js_dir.join("app").join(
            rel_path.with_file_name(format!("{}.min.js", rel_path.file_stem().unwrap().to_str().unwrap()))
        );

        // Create parent directory if needed
        if let Some(parent) = min_dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Read the file content
        let content = std::fs::read_to_string(js_file).map_err(|e| e.to_string())?;

        // Process based on environment
        if is_production {
            crate::logger::debug(&format!("Copying {} to {}", rel_path.display(), min_dest_path.display())).map_err(|e| e.to_string())?;
            // TODO: Implement actual JS minification in the future
            std::fs::write(&min_dest_path, &content).map_err(|e| e.to_string())?;
        } else {
            crate::logger::debug(&format!("Copying {} to {}", rel_path.display(), min_dest_path.display())).map_err(|e| e.to_string())?;
            std::fs::write(&min_dest_path, &content).map_err(|e| e.to_string())?;
        }
    }

    // Success message
    let mode = if is_production { "production" } else { "development" };
    crate::logger::success(&format!("Processed {} JS files in {} mode", js_files.len(), mode)).map_err(|e| e.to_string())?;

    Ok(())
}

// Publish CSS files from src/assets/css to public/css with environment-based minification
pub fn publish_css(config: &Config) -> Result<(), String> {
    let is_production = config.environment == "prod" || config.environment == "production";
    let project_dir = &config.project_dir;

    // Source and destination directories
    let src_css_dir = project_dir.join("src").join("assets").join("css");
    let public_dir = get_public_dir(config);
    let dest_css_dir = project_dir.join(&public_dir).join("css");

    // Create directories
    std::fs::create_dir_all(&dest_css_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dest_css_dir.join("app")).map_err(|e| e.to_string())?;
    
    // Create source directory if it doesn't exist
    if !src_css_dir.exists() {
        std::fs::create_dir_all(&src_css_dir).map_err(|e| e.to_string())?;
        crate::logger::info("Created CSS source directory (no files to process)").map_err(|e| e.to_string())?;
        return Ok(());
    }

    crate::logger::info(&format!("Publishing CSS files ({} mode)...", 
        if is_production { "production" } else { "development" })).map_err(|e| e.to_string())?;

    // Get all CSS files in the source directory (excluding already minified ones)
    let css_files = WalkDir::new(&src_css_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file() && 
            e.path().extension().map_or(false, |ext| ext == "css") && 
            !e.path().to_str().unwrap_or("").contains(".min.css")
        })
        .collect::<Vec<_>>();

    if css_files.is_empty() {
        crate::logger::info("No CSS files found to process").map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Process each CSS file
    for entry in &css_files {
        let src_path = entry.path();
        let rel_path = src_path.strip_prefix(&src_css_dir).unwrap();
        let min_dest_path = dest_css_dir.join("app").join(
            rel_path.with_file_name(format!("{}.min.css", rel_path.file_stem().unwrap().to_str().unwrap()))
        );

        // Create parent directory if needed
        if let Some(parent) = min_dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Read the file content
        let content = std::fs::read_to_string(src_path).map_err(|e| e.to_string())?;
        crate::logger::debug(&format!("Processing {}", rel_path.display())).map_err(|e| e.to_string())?;

        // Process based on environment
        if is_production {
            // Minify the content
            let minified = Minifier::default()
                .minify(&content, Level::Three)
                .map_err(|e| format!("CSS minification error: {}", e))?;

            // Write the minified content
            std::fs::write(&min_dest_path, &minified).map_err(|e| e.to_string())?;
        } else {
            // In development mode, write expanded content but still use .min.css extension
            std::fs::write(&min_dest_path, &content).map_err(|e| e.to_string())?;
        }
    }

    // Success message
    let mode_msg = if is_production { "minified for production" } else { "expanded for development" };
    crate::logger::success(&format!("Published {} CSS files as .min.css ({})", css_files.len(), mode_msg)).map_err(|e| e.to_string())?;

    Ok(())
}

