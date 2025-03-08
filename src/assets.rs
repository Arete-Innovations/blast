use crate::configs::Config;
use css_minify::optimizations::{Level, Minifier};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use sass_rs::{compile_file, Options, OutputStyle};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use toml::Value;
use walkdir::WalkDir;

async fn download_file(url: &str, dest_path: &Path) -> Result<(), Box<dyn Error>> {
    let response = reqwest::get(url).await?;
    let bytes = response.bytes().await?;
    let mut file = fs::File::create(dest_path).await?;
    file.write_all(&bytes).await?;
    Ok(())
}

async fn download_materialize_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;

    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let materialize_dir = public_dir.join("js").join("materialize");

    fs::create_dir_all(&materialize_dir).await?;

    let js_url = config.assets["assets"]["materialize"]["js_url"].as_str().unwrap();

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if is_interactive {
        // In interactive mode, log messages directly without progress bar
        let _ = crate::output::log("Downloading Materialize JS");

        let js_path = materialize_dir.join("materialize.min.js");
        download_file(js_url, &js_path).await?;

        let _ = crate::output::log("Materialize JS downloaded successfully.");
    } else {
        // In CLI mode, use progress bar
        let pb = ProgressBar::new(1);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        pb.set_message("Downloading Materialize JS");
        let js_path = materialize_dir.join("materialize.min.js");
        download_file(js_url, &js_path).await?;
        pb.finish_with_message("Materialize JS downloaded successfully.");
    }

    Ok(())
}

async fn download_fontawesome_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let fa_base_url = config.assets["assets"]["fontawesome"]["base_url"].as_str().unwrap();
    let fa_public_dir = public_dir.join("fonts").join("fontawesome");

    // Create FontAwesome directories
    fs::create_dir_all(&fa_public_dir).await?;
    fs::create_dir_all(&fa_public_dir.join("css")).await?;
    fs::create_dir_all(&fa_public_dir.join("js")).await?;
    fs::create_dir_all(&fa_public_dir.join("sprites")).await?;
    fs::create_dir_all(&fa_public_dir.join("webfonts")).await?;

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // Get FA assets to download from config
    let css_assets = config.assets["assets"]["fontawesome"]["css"].as_array().unwrap();
    let js_assets = config.assets["assets"]["fontawesome"]["js"].as_array().unwrap();
    let sprite_assets = config.assets["assets"]["fontawesome"]["sprites"].as_array().unwrap();
    let webfont_assets = config.assets["assets"]["fontawesome"]["webfonts"].as_array().unwrap();

    let total_assets = css_assets.len() + js_assets.len() + sprite_assets.len() + webfont_assets.len();

    if is_interactive {
        // In interactive mode, log progress directly
        let _ = crate::output::log(&format!("Downloading {} FontAwesome assets...", total_assets));

        // Download CSS files
        for css in css_assets {
            let css_path = css.as_str().unwrap();
            let dest_path = fa_public_dir.join(css_path);
            let url = format!("{}/{}", fa_base_url, css_path);
            let _ = crate::output::log(&format!("Downloading {}", css_path));
            download_file(&url, &dest_path).await?;
        }

        // Download JS files
        for js in js_assets {
            let js_path = js.as_str().unwrap();
            let dest_path = fa_public_dir.join(js_path);
            let url = format!("{}/{}", fa_base_url, js_path);
            let _ = crate::output::log(&format!("Downloading {}", js_path));
            download_file(&url, &dest_path).await?;
        }

        // Download sprite files
        for sprite in sprite_assets {
            let sprite_path = sprite.as_str().unwrap();
            let dest_path = fa_public_dir.join(sprite_path);
            let url = format!("{}/{}", fa_base_url, sprite_path);
            let _ = crate::output::log(&format!("Downloading {}", sprite_path));
            download_file(&url, &dest_path).await?;
        }

        // Download webfont files
        for font in webfont_assets {
            let font_path = font.as_str().unwrap();
            let dest_path = fa_public_dir.join(font_path);
            let url = format!("{}/{}", fa_base_url, font_path);
            let _ = crate::output::log(&format!("Downloading {}", font_path));
            download_file(&url, &dest_path).await?;
        }

        let _ = crate::output::log("FontAwesome downloaded successfully.");
    } else {
        // Create a progress bar for visual feedback
        let pb = ProgressBar::new(total_assets as u64);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        // Use a shared progress counter
        let progress = Arc::new(Mutex::new(0usize));
        let mut tasks = FuturesUnordered::new();

        // Download CSS files
        for css in css_assets {
            let css_path = css.as_str().unwrap().to_string();
            let dest_path = fa_public_dir.join(&css_path);
            let url = format!("{}/{}", fa_base_url, css_path);
            let pb_clone = pb.clone();
            let progress_clone = Arc::clone(&progress);

            tasks.push(tokio::spawn(async move {
                match download_file(&url, &dest_path).await {
                    Ok(_) => {
                        let mut progress = progress_clone.lock().unwrap();
                        *progress += 1;
                        pb_clone.set_message(format!("Downloaded {} ({}/{})", css_path, *progress, total_assets));
                        pb_clone.inc(1);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to download {}: {}", css_path, e)),
                }
            }));
        }

        // Download JS files
        for js in js_assets {
            let js_path = js.as_str().unwrap().to_string();
            let dest_path = fa_public_dir.join(&js_path);
            let url = format!("{}/{}", fa_base_url, js_path);
            let pb_clone = pb.clone();
            let progress_clone = Arc::clone(&progress);

            tasks.push(tokio::spawn(async move {
                match download_file(&url, &dest_path).await {
                    Ok(_) => {
                        let mut progress = progress_clone.lock().unwrap();
                        *progress += 1;
                        pb_clone.set_message(format!("Downloaded {} ({}/{})", js_path, *progress, total_assets));
                        pb_clone.inc(1);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to download {}: {}", js_path, e)),
                }
            }));
        }

        // Download sprite files
        for sprite in sprite_assets {
            let sprite_path = sprite.as_str().unwrap().to_string();
            let dest_path = fa_public_dir.join(&sprite_path);
            let url = format!("{}/{}", fa_base_url, sprite_path);
            let pb_clone = pb.clone();
            let progress_clone = Arc::clone(&progress);

            tasks.push(tokio::spawn(async move {
                match download_file(&url, &dest_path).await {
                    Ok(_) => {
                        let mut progress = progress_clone.lock().unwrap();
                        *progress += 1;
                        pb_clone.set_message(format!("Downloaded {} ({}/{})", sprite_path, *progress, total_assets));
                        pb_clone.inc(1);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to download {}: {}", sprite_path, e)),
                }
            }));
        }

        // Download webfont files
        for font in webfont_assets {
            let font_path = font.as_str().unwrap().to_string();
            let dest_path = fa_public_dir.join(&font_path);
            let url = format!("{}/{}", fa_base_url, font_path);
            let pb_clone = pb.clone();
            let progress_clone = Arc::clone(&progress);

            tasks.push(tokio::spawn(async move {
                match download_file(&url, &dest_path).await {
                    Ok(_) => {
                        let mut progress = progress_clone.lock().unwrap();
                        *progress += 1;
                        pb_clone.set_message(format!("Downloaded {} ({}/{})", font_path, *progress, total_assets));
                        pb_clone.inc(1);
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to download {}: {}", font_path, e)),
                }
            }));
        }

        // Wait for all downloads to complete
        let mut has_errors = false;
        while let Some(result) = tasks.next().await {
            if let Ok(Err(e)) = result {
                println!("Error: {}", e);
                has_errors = true;
            }
        }

        if has_errors {
            pb.finish_with_message("FontAwesome download completed with some errors.");
        } else {
            pb.finish_with_message("FontAwesome downloaded successfully.");
        }
    }

    Ok(())
}

async fn download_materialicons_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    
    let mi_base_url = config.assets["assets"]["materialicons"]["base_url"].as_str().unwrap();
    let mi_public_dir = public_dir.join("fonts").join("material-icons");
    
    let woff2_file = config.assets["assets"]["materialicons"]["woff2"].as_str().unwrap();
    let ttf_file = config.assets["assets"]["materialicons"]["ttf"].as_str().unwrap();
    
    // Create Material Icons directory
    fs::create_dir_all(&mi_public_dir).await?;
    
    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";
    
    if is_interactive {
        // In interactive mode, log progress directly
        let _ = crate::output::log("Downloading Material Icons webfonts...");
        
        // Download WOFF2
        let woff2_url = format!("{}/{}", mi_base_url, woff2_file);
        let woff2_path = mi_public_dir.join(woff2_file);
        let _ = crate::output::log(&format!("Downloading {}", woff2_file));
        download_file(&woff2_url, &woff2_path).await?;
        
        // Download TTF
        let ttf_url = format!("{}/{}", mi_base_url, ttf_file);
        let ttf_path = mi_public_dir.join(ttf_file);
        let _ = crate::output::log(&format!("Downloading {}", ttf_file));
        download_file(&ttf_url, &ttf_path).await?;
        
        let _ = crate::output::log("Material Icons downloaded successfully.");
    } else {
        // Create a progress bar for visual feedback
        let pb = ProgressBar::new(2);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);
        
        // Download WOFF2
        let woff2_url = format!("{}/{}", mi_base_url, woff2_file);
        let woff2_path = mi_public_dir.join(woff2_file);
        pb.set_message(format!("Downloading {}", woff2_file));
        download_file(&woff2_url, &woff2_path).await?;
        pb.inc(1);
        
        // Download TTF
        let ttf_url = format!("{}/{}", mi_base_url, ttf_file);
        let ttf_path = mi_public_dir.join(ttf_file);
        pb.set_message(format!("Downloading {}", ttf_file));
        download_file(&ttf_url, &ttf_path).await?;
        pb.inc(1);
        
        pb.finish_with_message("Material Icons downloaded successfully.");
    }
    
    Ok(())
}

async fn download_htmx_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;

    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let htmx_dir = public_dir.join("js").join("htmx");

    fs::create_dir_all(&htmx_dir).await?;

    let js_url = config.assets["assets"]["htmx"]["js_url"].as_str().unwrap();

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if is_interactive {
        // In interactive mode, log messages directly without progress bar
        let _ = crate::output::log("Downloading HTMX JS");

        let js_path = htmx_dir.join("htmx.min.js");
        download_file(js_url, &js_path).await?;

        let _ = crate::output::log("HTMX JS downloaded successfully.");
    } else {
        // In CLI mode, use progress bar
        let pb = ProgressBar::new(1);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        pb.set_message("Downloading HTMX JS");
        let js_path = htmx_dir.join("htmx.min.js");
        download_file(js_url, &js_path).await?;
        pb.finish_with_message("HTMX JS downloaded successfully.");
    }

    Ok(())
}

pub async fn download_assets_async(config: &Config) -> Result<(), Box<dyn Error>> {
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;
    download_htmx_js(config).await?;

    Ok(())
}

pub async fn transpile_all_scss(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let project_dir = &config.project_dir;
    let is_production = config.environment == "prod" || config.environment == "production";

    let sass_dir = project_dir.join("src/assets/sass");
    let css_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public")).join("css");

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
    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let is_production = config.environment == "prod" || config.environment == "production";

    // Source and destination directories
    let src_js_dir = project_dir.join("src").join("assets").join("js");
    let dest_js_dir = public_dir.join("js").join("app");

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
            let min_dest_path = dest_js_dir.join(rel_path.with_file_name(format!(
                "{}.min.js",
                rel_path.file_stem().unwrap().to_str().unwrap()
            )));

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
            let min_dest_path = dest_js_dir.join(rel_path.with_file_name(format!(
                "{}.min.js",
                rel_path.file_stem().unwrap().to_str().unwrap()
            )));

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
    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let dest_css_dir = public_dir.join("css").join("app");

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
pub async fn process_all_assets(config: &Config) -> Result<(), Box<dyn Error>> {
    // First download all CDN assets
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;
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