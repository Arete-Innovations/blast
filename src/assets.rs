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
    
    for &key in path {
        if let Some(value) = current.get(key) {
            current = value;
        } else {
            return default.map(|s| s.to_string());
        }
    }
    
    if let Some(s) = current.as_str() {
        Some(s.to_string())
    } else {
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
    let materialize_dir = project_dir.join(public_dir).join("js").join("materialize");

    fs::create_dir_all(&materialize_dir).await?;

    let js_url = get_config_value(config, &["materialize", "js_url"], None).ok_or("Missing materialize js_url in config")?;

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
    let fa_base_url = get_config_value(config, &["fontawesome", "base_url"], None).ok_or("Missing fontawesome base_url in config")?;
    let fa_public_dir = project_dir.join(public_dir).join("fonts").join("fontawesome");

    // Create FontAwesome directories
    fs::create_dir_all(&fa_public_dir).await?;
    fs::create_dir_all(&fa_public_dir.join("css")).await?;
    fs::create_dir_all(&fa_public_dir.join("js")).await?;
    fs::create_dir_all(&fa_public_dir.join("sprites")).await?;
    fs::create_dir_all(&fa_public_dir.join("webfonts")).await?;

    // Get FA assets to download from config - need to handle arrays differently
    // This is a temporary solution - ideally we'd extend our helper function to handle arrays
    let fa_css = if let Some(array) = config.assets.get("fontawesome").and_then(|f| f.get("css")).and_then(|c| c.as_array()) {
        array
    } else {
        return Err("Missing fontawesome css configuration".into());
    };
    
    let fa_js = if let Some(array) = config.assets.get("fontawesome").and_then(|f| f.get("js")).and_then(|c| c.as_array()) {
        array
    } else {
        return Err("Missing fontawesome js configuration".into());
    };
    
    let fa_sprites = if let Some(array) = config.assets.get("fontawesome").and_then(|f| f.get("sprites")).and_then(|c| c.as_array()) {
        array
    } else {
        return Err("Missing fontawesome sprites configuration".into());
    };
    
    let fa_webfonts = if let Some(array) = config.assets.get("fontawesome").and_then(|f| f.get("webfonts")).and_then(|c| c.as_array()) {
        array
    } else {
        return Err("Missing fontawesome webfonts configuration".into());
    };
    
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

    let mi_base_url = get_config_value(config, &["materialicons", "base_url"], None).ok_or("Missing materialicons base_url in config")?;
    let mi_public_dir = project_dir.join(public_dir).join("fonts").join("material-icons");

    let woff2_file = get_config_value(config, &["materialicons", "woff2"], None).ok_or("Missing materialicons woff2 in config")?;
    let ttf_file = get_config_value(config, &["materialicons", "ttf"], None).ok_or("Missing materialicons ttf in config")?;

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

    let js_url = get_config_value(config, &["htmx", "js_url"], None).ok_or("Missing htmx js_url in config")?;

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
    
    // Keep only dark.scss if it exists (don't overwrite it)
    let dark_scss_path = src_sass_dir.join("dark.scss");
    let has_dark_scss = dark_scss_path.exists();
    
    // Create a progress bar spinner for the download operation
    let mut progress = crate::logger::create_progress(None);
    progress.set_message("Downloading Materialize SCSS files from GitHub...");
    
    // URL to the Materialize CSS repo's sass directory
    const MATERIALIZE_REPO_URL: &str = "https://github.com/materializecss/materialize/archive/refs/heads/main.zip";
    
    // Download the zip file to a temporary location
    let temp_dir = std::env::temp_dir().join("materialize_download");
    fs::create_dir_all(&temp_dir).await?;
    let zip_path = temp_dir.join("materialize.zip");
    
    // Download the zip file
    match download_file(MATERIALIZE_REPO_URL, &zip_path).await {
        Ok(_) => {
            progress.set_message("Extracting Materialize SCSS files...");
            
            // Extract the zip file (need to use sync API)
            let extract_result = tokio::task::spawn_blocking(move || {
                let zip_file = std::fs::File::open(&zip_path)?;
                let mut archive = zip::ZipArchive::new(zip_file)?;
                
                // Extract only the sass directory
                for i in 0..archive.len() {
                    let mut file = archive.by_index(i)?;
                    let file_path = file.name();
                    
                    // Check if this is a sass file we want to extract
                    if file_path.contains("materialize-main/sass/") && 
                       (file_path.ends_with(".scss") || file.name().contains("/")) {
                        
                        // Get the path relative to the sass directory
                        let rel_path = file_path.replace("materialize-main/sass/", "");
                        
                        // Skip dark.scss if we have a custom one
                        if has_dark_scss && rel_path == "dark.scss" {
                            continue;
                        }
                        
                        let target_path = src_sass_dir.join(&rel_path);
                        
                        if file.is_dir() {
                            std::fs::create_dir_all(&target_path)?;
                        } else {
                            // Create parent directory
                            if let Some(parent) = target_path.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            
                            let mut target_file = std::fs::File::create(&target_path)?;
                            std::io::copy(&mut file, &mut target_file)?;
                        }
                    }
                }
                
                // Clean up the temporary directory
                std::fs::remove_dir_all(temp_dir)?;
                
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }).await?;
            
            match extract_result {
                Ok(_) => (),
                Err(e) => return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Error extracting zip: {}", e)))),
            };
            
            progress.success("Materialize SCSS files downloaded and extracted successfully.");
            Ok(())
        }
        Err(e) => {
            progress.error(&format!("Failed to download Materialize SCSS: {}", e));
            Err(e)
        }
    }
}

pub async fn download_assets_async(config: &Config) -> Result<(), Box<dyn Error>> {
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;
    download_materialize_scss(config).await?;
    download_htmx_js(config).await?;

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
    // First download all CDN assets
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;
    download_materialize_scss(config).await?;
    download_htmx_js(config).await?;

    // Process SCSS files - they will be automatically compressed in production mode
    transpile_all_scss(config).await?;

    // Process regular CSS files (non-SCSS generated) - this should only affect
    // CSS files that weren't generated from SCSS
    minify_css_files(config).await?;

    // Process JS files based on environment
    process_js(config).await?;

    // Publish CSS files from src/assets/css to public/css
    publish_css(config).await?;

    Ok(())
}
