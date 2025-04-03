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

fn download_fontawesome(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let public_dir = get_public_dir(config);
    
    // Get FontAwesome config section
    let fa_section = &config.assets["assets"]["fontawesome"];
    let fa_base_url = fa_section["base_url"].as_str()
        .ok_or_else(|| "Missing fontawesome base_url in config")?;
        
    // Standard directory structure
    let fa_public_dir = project_dir.join(public_dir).join("fonts").join("fontawesome");

    // Create FontAwesome directories
    for subdir in ["css", "js", "sprites", "webfonts"] {
        std::fs::create_dir_all(fa_public_dir.join(subdir)).map_err(|e| e.to_string())?;
    }

    // All FontAwesome asset paths are in Catalyst.toml - no defaults needed

    // Extract asset paths directly from config (no defaults needed as they're in Catalyst.toml)
    let get_string_array = |key: &str| -> Result<Vec<String>, String> {
        fa_section.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
            )
            .ok_or_else(|| format!("Missing '{}' in fontawesome config", key))
    };

    // Get asset lists from config
    let css_files = get_string_array("css")?;
    let js_files = get_string_array("js")?;
    let sprite_files = get_string_array("sprites")?;
    let webfont_files = get_string_array("webfonts")?;

    // Download all assets sequentially 
    let all_assets: Vec<String> = css_files.into_iter()
        .chain(js_files.into_iter())
        .chain(sprite_files.into_iter())
        .chain(webfont_files.into_iter())
        .collect();
        
    crate::logger::info(&format!("Downloading {} FontAwesome assets", all_assets.len()))?;

    // Simple sequential download
    for asset_path in all_assets {
        let url = format!("{}/{}", fa_base_url, asset_path);
        let dest_path = fa_public_dir.join(&asset_path);
        
        // Create parent directory if needed
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        
        match download_file(&url, &dest_path) {
            Ok(_) => crate::logger::debug(&format!("Downloaded {}", asset_path)).map_err(|e| e.to_string())?,
            Err(e) => crate::logger::warning(&format!("Failed to download {}: {}", asset_path, e)).map_err(|e| e.to_string())?,
        }
    }

    Ok(())
}

fn download_materialicons(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let public_dir = get_public_dir(config);
    
    // Get the materialicons section
    let mi_section = &config.assets["assets"]["materialicons"];
    
    // Get base URL and file names
    let mi_base_url = mi_section["base_url"].as_str()
        .ok_or_else(|| "Missing materialicons base_url in config")?;
    let woff2_file = mi_section["woff2"].as_str()
        .ok_or_else(|| "Missing materialicons woff2 in config")?;
    let ttf_file = mi_section["ttf"].as_str()
        .ok_or_else(|| "Missing materialicons ttf in config")?;

    // Create standard directory
    let mi_public_dir = project_dir.join(public_dir).join("fonts").join("material-icons");
    std::fs::create_dir_all(&mi_public_dir).map_err(|e| e.to_string())?;

    // Define files to download
    let files = [
        (format!("{}/{}", mi_base_url, woff2_file), mi_public_dir.join(woff2_file)),
        (format!("{}/{}", mi_base_url, ttf_file), mi_public_dir.join(ttf_file)),
    ];

    crate::logger::info("Downloading Material Icons webfonts...")?;

    // Download all files
    for (url, dest_path) in &files {
        download_file(url, dest_path)?;
    }

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

fn download_materialize_scss(config: &Config) -> Result<(), String> {
    let project_dir = &config.project_dir;
    let src_sass_dir = project_dir.join("src/assets/sass");
    let materialize_dir = project_dir.join("src/assets/materialize");

    // Create the directories if they don't exist
    std::fs::create_dir_all(&src_sass_dir).map_err(|e| e.to_string())?;
    
    // Create a progress bar spinner for the operation
    let mut progress = crate::logger::create_progress(None);
    progress.set_message("Setting up Materialize assets...");
    
    // Direct access to the materialize section we know exists
    let mat_section = &config.assets["assets"]["materialize"];
    
    // Get repository URL and version directly from the config structure
    let repo_url = mat_section["repo_url"].as_str()
        .ok_or_else(|| "Missing materialize repo_url in config")?;
    
    let version = mat_section["version"].as_str()
        .ok_or_else(|| "Missing materialize version in config")?;
    
    // Check if we should force a fresh clone (for debugging or version changes)
    let force_fresh = std::env::var("BLAST_FORCE_FRESH_MATERIALIZE").unwrap_or_else(|_| String::from("0")) == "1";
    
    // Check if materialize repo already exists
    let repo_exists = materialize_dir.exists();
    
    if repo_exists && force_fresh {
        // Clean up existing repo if forced
        progress.set_message("Forcing fresh clone of Materialize repository...");
        match std::fs::remove_dir_all(&materialize_dir) {
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
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        
        // Use standard process to run git clone
        // The Materialize repo hasn't been updated in years, so just clone the default branch
        let clone_output = std::process::Command::new("git")
            .args(&["clone", "--depth=1", &repo_url])
            .arg(&materialize_dir)
            .output();
            
        match clone_output {
            Ok(output) => {
                if !output.status.success() {
                    let error = String::from_utf8_lossy(&output.stderr);
                    let error_msg = format!("Failed to clone Materialize repository: {}", error);
                    progress.set_message(&error_msg);
                    
                    return Err(format!("Git clone failed: {}", error));
                }
                
                // Successfully cloned, now extract the compiled JS file if it exists and keep only the sass folder
                progress.set_message("Extracting JS file and keeping sass folder...");
                
                // First make sure the sass directory exists
                let sass_dir = materialize_dir.join("sass");
                if !sass_dir.exists() {
                    return Err("Could not find sass directory in the cloned repository".to_string());
                }
                
                // Check if the dist directory with compiled JS exists
                // Use the exact path: dist/js/materialize.min.js
                let js_file_path = materialize_dir.join("dist").join("js").join("materialize.min.js");
                let public_dir = get_public_dir(config);
                let js_dest_dir = project_dir.join(&public_dir).join("js").join("materialize");
                let materialize_js_dest = js_dest_dir.join("materialize.min.js");
                
                // Create js directory if it doesn't exist
                std::fs::create_dir_all(&js_dest_dir).map_err(|e| e.to_string())?;
                
                // Copy JS file from dist directory if it exists
                let js_from_dist = if js_file_path.exists() {
                    progress.set_message("Found Materialize JS in dist folder, copying...");
                    match std::fs::copy(&js_file_path, &materialize_js_dest) {
                        Ok(_) => {
                            progress.set_message("Materialize JS copied from dist folder successfully");
                            true
                        },
                        Err(e) => {
                            progress.set_message(&format!("Failed to copy Materialize JS from dist: {}", e));
                            false
                        }
                    }
                } else {
                    progress.set_message("Materialize JS not found in dist folder");
                    false
                };
                
                // Get all entries in the materialize directory
                for entry in std::fs::read_dir(&materialize_dir).map_err(|e| e.to_string())? {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        
                        // Skip the sass directory
                        if path.file_name().map_or(false, |name| name == "sass") {
                            continue;
                        }
                        
                        // Remove the entry - handle both files and directories
                        if path.is_dir() {
                            std::fs::remove_dir_all(&path).map_err(|e| 
                                format!("Failed to remove directory {}: {}", path.display(), e))?;
                        } else {
                            std::fs::remove_file(&path).map_err(|e| 
                                format!("Failed to remove file {}: {}", path.display(), e))?;
                        }
                    }
                }
                
                progress.set_message("Materialize repository cloned and cleaned - kept only sass directory");
                
                // Return whether we successfully copied the JS file from dist
                if js_from_dist {
                    return Ok(());
                }
            },
            Err(e) => {
                let error_msg = format!("Failed to execute git clone: {}", e);
                progress.set_message(&error_msg);
                
                // Check if git is installed
                let git_check = std::process::Command::new("git")
                    .arg("--version")
                    .output();
                    
                if git_check.is_err() {
                    progress.set_message("Git may not be installed or is not in PATH. Please install git and try again.");
                }
                
                return Err(e.to_string());
            }
        }
    } else {
        progress.set_message("Using existing Materialize repository");
    }

    // If we already copied the JS file from the distribution folder, skip this part
    // Otherwise, fall back to using CDN for JS
    let public_dir = get_public_dir(config);
    let js_dest_dir = project_dir.join(&public_dir).join("js").join("materialize");
    let materialize_js_dest = js_dest_dir.join("materialize.min.js");
    
    // Check if the JS file already exists (from the local copy operation)
    if !materialize_js_dest.exists() {
        // Create js directory if it doesn't exist
        std::fs::create_dir_all(&js_dest_dir).map_err(|e| e.to_string())?;
        
        // Fallback to CDN for the JS file
        let js_url = config.assets["assets"]["materialize"]["js_url"].as_str()
            .ok_or_else(|| "Missing materialize js_url in config")?;
            
        progress.set_message("JS file not found locally, downloading Materialize JS from CDN...");
        match download_file(&js_url, &materialize_js_dest) {
            Ok(_) => {
                progress.set_message("Materialize JS file downloaded from CDN successfully");
            },
            Err(e) => {
                progress.set_message(&format!("Failed to download Materialize JS: {}", e));
                return Err(format!("Failed to download Materialize JS: {}", e));
            }
        }
    } else {
        progress.set_message("Using Materialize JS file copied from local repository");
    }

    // Don't create SCSS files - they're already in the template
    // Just make sure the directory exists for future operations
    std::fs::create_dir_all(&src_sass_dir).map_err(|e| e.to_string())?;

    progress.set_message("Materialize setup completed successfully");
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
    
    // Simple linear download of all assets
    let asset_downloads = [
        ("FontAwesome", download_fontawesome(&fresh_config)),
        ("Material Icons", download_materialicons(&fresh_config)),
        ("Materialize", download_materialize_scss(&fresh_config)),
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
    let css_dir = project_dir.join(&public_dir).join("css");

    // Create directories
    std::fs::create_dir_all(&css_dir).map_err(|e| e.to_string())?;
    
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
        let output_file = css_dir.join(format!("{}.min.css", file_stem));
        
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

