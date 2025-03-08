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
    let materialize_dir = public_dir.join(config.assets["assets"]["materialize"]["public_dir"].as_str().unwrap());

    fs::create_dir_all(&materialize_dir.join("js")).await?;

    let js_url = config.assets["assets"]["materialize"]["js_url"].as_str().unwrap();

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if is_interactive {
        // In interactive mode, log messages directly without progress bar
        let _ = crate::output::log("Downloading Materialize JS");

        let js_path = materialize_dir.join("js").join("materialize.min.js");
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
        let js_path = materialize_dir.join("js").join("materialize.min.js");
        download_file(js_url, &js_path).await?;
        pb.finish_with_message("Materialize JS downloaded successfully.");
    }

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
            let output_file = css_dir.join(format!("{}.css", file_stem));
            let minified_output_file = css_dir.join(format!("{}.min.css", file_stem));

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
                    // Write the CSS file
                    fs::write(&output_file, &css_content).await?;

                    // In production mode, the .min.css file is the same as the .css file
                    // since the OutputStyle is already Compressed
                    if is_production {
                        fs::write(&minified_output_file, &css_content).await?;
                    } else {
                        // In development mode, also create the .min.css variant
                        fs::write(&minified_output_file, &css_content).await?;
                    }
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
            let output_file = css_dir.join(format!("{}.css", file_stem));
            let minified_output_file = css_dir.join(format!("{}.min.css", file_stem));

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
                    // Write the CSS file
                    fs::write(&output_file, &css_content).await?;

                    // In production mode, the .min.css file is the same as the .css file
                    // since the OutputStyle is already Compressed
                    if is_production {
                        fs::write(&minified_output_file, &css_content).await?;
                    } else {
                        // In development mode, also create the .min.css variant
                        fs::write(&minified_output_file, &css_content).await?;
                    }
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
    let project_dir = &config.project_dir;
    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let is_production = config.environment == "prod" || config.environment == "production";

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    // Create a progress bar for visual feedback
    let mut css_files = Vec::new();

    // Collect files first before showing any output
    for entry in WalkDir::new(&public_dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        // Only process CSS files that:
        // 1. Have .css extension
        // 2. Don't end with .min.css (already minified)
        // 3. Didn't come from SCSS compilation (we don't reprocess SCSS-generated files)
        if path.extension().is_some_and(|ext| ext == "css") && !path.file_name().unwrap_or_default().to_string_lossy().ends_with(".min.css") {
            // Check if this is a file we just generated from SCSS
            // This is a basic heuristic - if it's in the css dir and we have a matching .scss file
            let scss_dir = project_dir.join("src/assets/sass");
            let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let potential_scss = scss_dir.join(format!("{}.scss", file_stem));

            // Only add this CSS file if there's no matching SCSS file (to avoid double processing)
            if !potential_scss.exists() {
                css_files.push(path.to_path_buf());
            }
        }
    }

    if css_files.is_empty() {
        return Ok(());
    }

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log(&format!("Processing {} CSS files...", css_files.len()));

        // Process each file with logging
        for path in css_files {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            let _ = crate::output::log(&format!("Processing {}", file_name));

            let css_content = fs::read_to_string(&path).await?;
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let new_file_path = path.with_file_name(format!("{}.min.css", stem));

            // If in production mode, minify the CSS
            if is_production {
                match Minifier::default().minify(&css_content, Level::Three) {
                    Ok(minified_css) => {
                        // Write the minified content
                        fs::write(&new_file_path, &minified_css).await?;
                    }
                    Err(_e) => {
                        // Fallback - just copy the original content if minification fails
                        fs::write(&new_file_path, &css_content).await?;
                    }
                }
            } else {
                // In development mode, just copy the file
                fs::write(&new_file_path, &css_content).await?;
            }
        }

        // Show success message based on environment
        if is_production {
            let _ = crate::output::log("CSS minification complete (production mode).");
        } else {
            let _ = crate::output::log("CSS processing complete (development mode - no minification).");
        }
    } else {
        // Only show the file count as a summary, let the progress bar track individual files

        // Set up progress bar with proper styling
        let pb = ProgressBar::new(css_files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);
        pb.set_message("Minifying CSS files");

        // Process each file with the progress bar
        for path in css_files {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            pb.set_message(format!("Processing {}", file_name));

            let css_content = fs::read_to_string(&path).await?;
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let new_file_path = path.with_file_name(format!("{}.min.css", stem));

            // If in production mode, minify the CSS
            if is_production {
                match Minifier::default().minify(&css_content, Level::Three) {
                    Ok(minified_css) => {
                        // Write the minified content
                        fs::write(&new_file_path, &minified_css).await?;

                        // Calculate size reduction but don't print individual files
                        let _reduction = (1.0 - (minified_css.len() as f64 / css_content.len() as f64)) * 100.0;
                    }
                    Err(_e) => {
                        // Fallback - just copy the original content if minification fails
                        fs::write(&new_file_path, &css_content).await?;
                    }
                }
            } else {
                // In development mode, just copy the file
                fs::write(&new_file_path, &css_content).await?;
            }

            pb.inc(1);
        }

        // Show success message based on environment
        if is_production {
            pb.finish_with_message("✅ CSS minification complete (production mode).");
        } else {
            pb.finish_with_message("✅ CSS processing complete (development mode - no minification).");
        }
    }

    Ok(())
}

async fn download_fontawesome_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;

    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let fa_dir = public_dir.join(config.assets["assets"]["fontawesome"]["public_dir"].as_str().unwrap());

    fs::create_dir_all(&fa_dir.join("css")).await?;
    fs::create_dir_all(&fa_dir.join("js")).await?;
    fs::create_dir_all(&fa_dir.join("sprites")).await?;
    fs::create_dir_all(&fa_dir.join("webfonts")).await?;

    let base_url = config.assets["assets"]["fontawesome"]["base_url"].as_str().expect("base_url not found");

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    let total_files = config.assets["assets"]["fontawesome"]
        .as_table()
        .map_or(0, |table| table.values().filter_map(|value| value.as_array()).map(|arr| arr.len()).sum());

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log(&format!("Downloading {} FontAwesome files...", total_files));

        // Use a simple counter to track progress
        let counter = Arc::new(Mutex::new(0));
        let mut futures = FuturesUnordered::new();

        let process_files = |files: &Option<&Vec<Value>>, dir: &Path| {
            if let Some(file_list) = files {
                for file in *file_list {
                    if let Some(path) = file.as_str() {
                        let url = format!("{}/{}", base_url, path);
                        let filename = path.split('/').last().unwrap().to_string();
                        let dest_path = dir.join(filename.clone());
                        let counter_clone = Arc::clone(&counter);
                        futures.push(async move {
                            download_file(&url, &dest_path).await?;
                            let mut count = counter_clone.lock().unwrap();
                            *count += 1;
                            Ok::<(), Box<dyn Error>>(())
                        });
                    }
                }
            }
        };

        process_files(&config.assets["assets"]["fontawesome"]["css"].as_array(), &fa_dir.join("css"));
        process_files(&config.assets["assets"]["fontawesome"]["js"].as_array(), &fa_dir.join("js"));
        process_files(&config.assets["assets"]["fontawesome"]["sprites"].as_array(), &fa_dir.join("sprites"));
        process_files(&config.assets["assets"]["fontawesome"]["webfonts"].as_array(), &fa_dir.join("webfonts"));

        while futures.next().await.is_some() {}

        let _ = crate::output::log("FontAwesome assets downloaded successfully.");
    } else {
        // In CLI mode, use progress bar
        let pb = Arc::new(Mutex::new(ProgressBar::new(total_files as u64)));
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.lock().unwrap().set_style(pb_style);

        let mut futures = FuturesUnordered::new();

        let process_files = |files: &Option<&Vec<Value>>, dir: &Path| {
            if let Some(file_list) = files {
                for file in *file_list {
                    if let Some(path) = file.as_str() {
                        let url = format!("{}/{}", base_url, path);
                        let filename = path.split('/').last().unwrap().to_string();
                        let dest_path = dir.join(filename.clone());
                        let pb_clone = Arc::clone(&pb);
                        futures.push(async move {
                            download_file(&url, &dest_path).await?;
                            let pb = pb_clone.lock().unwrap();
                            pb.set_message(format!("Downloaded {}", filename));
                            pb.inc(1);
                            Ok::<(), Box<dyn Error>>(())
                        });
                    }
                }
            }
        };

        process_files(&config.assets["assets"]["fontawesome"]["css"].as_array(), &fa_dir.join("css"));
        process_files(&config.assets["assets"]["fontawesome"]["js"].as_array(), &fa_dir.join("js"));
        process_files(&config.assets["assets"]["fontawesome"]["sprites"].as_array(), &fa_dir.join("sprites"));
        process_files(&config.assets["assets"]["fontawesome"]["webfonts"].as_array(), &fa_dir.join("webfonts"));

        while futures.next().await.is_some() {}

        pb.lock().unwrap().finish_with_message("FontAwesome assets downloaded successfully.");
    }

    Ok(())
}

async fn download_materialicons_async(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;

    let public_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public"));
    let mi_dir = public_dir.join(config.assets["assets"]["materialicons"]["public_dir"].as_str().unwrap());

    // Create necessary directories
    fs::create_dir_all(&mi_dir).await?;

    let base_url = config.assets["assets"]["materialicons"]["base_url"].as_str().expect("base_url not found");

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log("Downloading Material Icons...");

        if let Some(woff2_path) = config.assets["assets"]["materialicons"]["woff2"].as_str() {
            let url = format!("{}/{}", base_url, woff2_path);
            let filename = woff2_path.split('/').last().unwrap();
            let dest_path = mi_dir.join(filename);
            let _ = crate::output::log(&format!("Downloading {}", filename));
            download_file(&url, &dest_path).await?;
        }

        if let Some(ttf_path) = config.assets["assets"]["materialicons"]["ttf"].as_str() {
            let url = format!("{}/{}", base_url, ttf_path);
            let filename = ttf_path.split('/').last().unwrap();
            let dest_path = mi_dir.join(filename);
            let _ = crate::output::log(&format!("Downloading {}", filename));
            download_file(&url, &dest_path).await?;
        }

        let _ = crate::output::log("Material Icons downloaded successfully.");
    } else {
        // In CLI mode, use progress bar
        let total_files = 2;
        let pb = ProgressBar::new(total_files);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        if let Some(woff2_path) = config.assets["assets"]["materialicons"]["woff2"].as_str() {
            let url = format!("{}/{}", base_url, woff2_path);
            let filename = woff2_path.split('/').last().unwrap();
            let dest_path = mi_dir.join(filename);
            pb.set_message(format!("Downloading {}", filename));
            download_file(&url, &dest_path).await?;
            pb.inc(1);
        }

        if let Some(ttf_path) = config.assets["assets"]["materialicons"]["ttf"].as_str() {
            let url = format!("{}/{}", base_url, ttf_path);
            let filename = ttf_path.split('/').last().unwrap();
            let dest_path = mi_dir.join(filename);
            pb.set_message(format!("Downloading {}", filename));
            download_file(&url, &dest_path).await?;
            pb.inc(1);
        }

        pb.finish_with_message("Material Icons downloaded successfully.");
    }

    Ok(())
}

pub async fn process_js(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let is_production = config.environment == "prod" || config.environment == "production";

    let js_dir = project_dir.join("src/assets/js");
    let public_js_dir = project_dir.join(config.assets["assets"]["public_dir"].as_str().unwrap_or("public")).join("js");

    // Create the output directory if it doesn't exist
    fs::create_dir_all(&public_js_dir).await?;

    // Find all JS files
    let mut js_files = Vec::new();
    if js_dir.exists() {
        let mut entries = fs::read_dir(&js_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().is_some_and(|ext| ext == "js") {
                js_files.push(entry.path());
            }
        }
    }

    if js_files.is_empty() {
        return Ok(());
    }

    // Check if we're in interactive mode
    let is_interactive = std::env::var("BLAST_INTERACTIVE").unwrap_or_else(|_| String::from("0")) == "1";

    if is_interactive {
        // In interactive mode, log directly without progress bar
        let _ = crate::output::log(&format!("Processing {} JS files...", js_files.len()));

        for js_file in js_files {
            let file_stem = js_file.file_stem().unwrap().to_str().unwrap();
            let output_file = public_js_dir.join(format!("{}.js", file_stem));
            let minified_output_file = public_js_dir.join(format!("{}.min.js", file_stem));

            let _ = crate::output::log(&format!("Processing {}", file_stem));

            // Read the JS file
            let js_content = fs::read_to_string(&js_file).await?;

            // Always create the .js file with original content
            fs::write(&output_file, &js_content).await?;

            // In production mode, minify JS (simulated for now)
            // For actual minification, you would use a JS minifier library
            if is_production {
                // Placeholder for actual minification
                fs::write(&minified_output_file, &js_content).await?;
            } else {
                // In development mode, just copy the file
                fs::write(&minified_output_file, &js_content).await?;
            }
        }

        // Show success message based on environment
        if is_production {
            let _ = crate::output::log("JS processing complete (production mode).");
        } else {
            let _ = crate::output::log("JS processing complete (development mode).");
        }
    } else {
        // Set up progress bar for CLI mode
        let pb = ProgressBar::new(js_files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("#>-");
        pb.set_style(pb_style);

        for js_file in js_files {
            let file_stem = js_file.file_stem().unwrap().to_str().unwrap();
            let output_file = public_js_dir.join(format!("{}.js", file_stem));
            let minified_output_file = public_js_dir.join(format!("{}.min.js", file_stem));

            pb.set_message(format!("Processing {}", file_stem));

            // Read the JS file
            let js_content = fs::read_to_string(&js_file).await?;

            // Always create the .js file with original content
            fs::write(&output_file, &js_content).await?;

            // In production mode, minify JS (simulated for now)
            // For actual minification, you would use a JS minifier library
            if is_production {
                // Placeholder for actual minification
                // In a real implementation, you would use a minifier like minify-js
                // Example: let minified = minify_js(&js_content);
                fs::write(&minified_output_file, &js_content).await?;
            } else {
                // In development mode, just copy the file
                fs::write(&minified_output_file, &js_content).await?;
            }

            pb.inc(1);
        }

        // Show success message based on environment
        if is_production {
            pb.finish_with_message("JS processing complete (production mode).");
        } else {
            pb.finish_with_message("JS processing complete (development mode).");
        }
    }

    Ok(())
}

// Download all CDN assets without processing other assets
pub async fn download_assets_async(config: &Config) -> Result<(), Box<dyn Error>> {
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;

    Ok(())
}

// Complete asset processing pipeline - downloads and processes all assets
pub async fn process_all_assets(config: &Config) -> Result<(), Box<dyn Error>> {
    // First download all CDN assets
    download_fontawesome_async(config).await?;
    download_materialicons_async(config).await?;
    download_materialize_js(config).await?;

    // Process SCSS files - they will be automatically compressed in production mode
    transpile_all_scss(config).await?;

    // Process regular CSS files (non-SCSS generated) - this should only affect
    // CSS files that weren't generated from SCSS
    minify_css_files(config).await?;

    // Process JS files based on environment
    process_js(config).await?;

    Ok(())
}
