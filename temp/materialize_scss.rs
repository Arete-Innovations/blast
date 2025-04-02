use flate2::read::GzDecoder;
use tar::Archive;
use std::path::Path;
use std::error::Error;

async fn download_materialize_scss(config: &Config) -> Result<(), Box<dyn Error>> {
    let project_dir = &config.project_dir;
    let src_sass_dir = project_dir.join("src/assets/sass");
    
    // Create the directory if it doesn't exist
    fs::create_dir_all(&src_sass_dir).await?;
    
    // Create a progress bar spinner for the download operation
    let mut progress = crate::logger::create_progress(None);
    progress.set_message("Downloading Materialize SCSS files...");
    
    // Get the SCSS URL from config or use the default stable 1.0.0 release
    let scss_url = get_config_value(
        config, 
        &["materialize", "scss_url"], 
        Some("https://github.com/Dogfalo/materialize/archive/refs/tags/1.0.0.tar.gz")
    ).unwrap_or_else(|| "https://github.com/Dogfalo/materialize/archive/refs/tags/1.0.0.tar.gz".to_string());
    
    // Download the tar.gz file to a temporary location
    let temp_dir = std::env::temp_dir().join("materialize_download");
    fs::create_dir_all(&temp_dir).await?;
    let tar_path = temp_dir.join("materialize.tar.gz");
    
    // Step 1: Download the archive
    match download_file(&scss_url, &tar_path).await {
        Ok(_) => {
            progress.set_message("Extracting Materialize SCSS files...");
            
            // Step 2: Extract the archive in a blocking task
            let extract_result = tokio::task::spawn_blocking({
                let temp_dir = temp_dir.clone();
                let src_sass_dir = src_sass_dir.clone();
                
                move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    // Open the tar.gz file
                    let tar_file = std::fs::File::open(&tar_path)?;
                    let gz_decoder = GzDecoder::new(tar_file);
                    let mut archive = Archive::new(gz_decoder);
                    
                    // Extract relevant SCSS files
                    for entry_result in archive.entries()? {
                        let mut entry = entry_result?;
                        let path = entry.path()?;
                        let path_str = path.to_string_lossy();
                        
                        // Find all files in the sass directory
                        if path_str.contains("sass/") && !path_str.ends_with("/") {
                            // Get path relative to sass directory (materialize-1.0.0/sass/ prefix)
                            if let Some(rel_path) = path_str.split("sass/").nth(1) {
                                if !rel_path.is_empty() {
                                    // Target path in our sass directory
                                    let target_path = src_sass_dir.join(rel_path);
                                    
                                    // Create parent directory if needed
                                    if let Some(parent) = target_path.parent() {
                                        std::fs::create_dir_all(parent)?;
                                    }
                                    
                                    // Extract the file
                                    entry.unpack(&target_path)?;
                                }
                            }
                        }
                    }
                    
                    // Clean up the temporary directory
                    let _ = std::fs::remove_dir_all(temp_dir);
                    
                    Ok(())
                }
            }).await?;
            
            // Step 3: Add our custom theme files if extraction worked
            if let Err(e) = extract_result {
                progress.warning(&format!("Error extracting Materialize SCSS: {}", e));
                progress.set_message("Creating basic SCSS files as fallback...");
                
                // Fallback to creating our own files
                create_theme_files_async(&src_sass_dir).await?;
                
                progress.success("Basic SCSS files created successfully (as fallback).");
            } else {
                // Even with successful extraction, add our custom theme files
                // that might not be in the original archive
                progress.set_message("Adding custom theme files...");
                
                create_theme_files_async(&src_sass_dir).await?;
                
                progress.success("Materialize SCSS files downloaded and custom themes added.");
            }
            
            Ok(())
        }
        Err(e) => {
            progress.warning(&format!("Failed to download Materialize SCSS: {}", e));
            
            // Fallback to creating basic files
            progress.set_message("Creating basic SCSS files instead...");
            
            // Create necessary directories
            let components_dir = src_sass_dir.join("components");
            let forms_dir = components_dir.join("forms");
            fs::create_dir_all(&forms_dir).await?;
            
            // Create theme files
            create_theme_files_async(&src_sass_dir).await?;
            
            progress.success("Basic SCSS files created as fallback.");
            Ok(())
        }
    }
}

// Helper function to create theme files asynchronously
async fn create_theme_files_async(src_sass_dir: &Path) -> Result<(), Box<dyn Error>> {
    // Check if we already have these files
    let dark_scss_path = src_sass_dir.join("dark.scss");
    let light_scss_path = src_sass_dir.join("light.scss");
    let sepia_scss_path = src_sass_dir.join("sepia.scss");
    let midnight_scss_path = src_sass_dir.join("midnight.scss");
    
    // Create components/forms directory if it doesn't exist
    let forms_dir = src_sass_dir.join("components").join("forms");
    fs::create_dir_all(&forms_dir).await?;
    
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
    
    Ok(())
}