use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, MultiSelect};
use std::io::Write;
use std::process::Command;

// Main git manager interface
pub fn launch_manager() {
    // Use ANSI clear screen to clean any existing output
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap();

    // Git operations menu
    let operations = vec!["Show Status", "Pull from Remote", "Push to Remote", "Create Commit", "View Log", "Configure Git", "Back to Main Menu"];

    loop {
        let selection = FuzzySelect::with_theme(&ColorfulTheme::default()).with_prompt("Git Operations").items(&operations).default(0).interact().unwrap();

        match operations[selection] {
            "Show Status" => git_status(),
            "Pull from Remote" => git_pull(),
            "Push to Remote" => git_push(),
            "Create Commit" => git_commit(),
            "View Log" => git_log(),
            "Configure Git" => git_config(),
            "Back to Main Menu" => break,
            _ => continue,
        }

        // Wait for a key press before continuing
        println!("\nPress Enter to continue...");
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();

        // Clear screen for next operation
        print!("\x1B[2J\x1B[1;1H");
        std::io::stdout().flush().unwrap();
    }
}

// Show repository status
pub fn git_status() {
    println!("📊 Git repository status:");
    let output = Command::new("git").args(["status"]).output().expect("Failed to execute git status");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if !output.stderr.is_empty() {
        println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
    }
}

// Pull from remote repository
pub fn git_pull() {
    println!("⬇️ Pulling from remote repository...");
    let output = Command::new("git").args(["pull"]).output().expect("Failed to execute git pull");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if !output.stderr.is_empty() {
        println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
    }
}

// Push to remote repository
pub fn git_push() {
    println!("⬆️ Pushing to remote repository...");

    // Check current branch
    let branch_output = Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output().expect("Failed to get current branch");

    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    // Let user choose to set upstream if needed
    let output = Command::new("git").args(["push"]).output();

    match output {
        Ok(result) => {
            println!("{}", String::from_utf8_lossy(&result.stdout));

            let stderr = String::from_utf8_lossy(&result.stderr);
            if stderr.contains("set the upstream") {
                println!("⚠️ {}", stderr);

                let set_upstream = Confirm::new().with_prompt(format!("Do you want to set upstream for branch '{}'?", branch)).default(true).interact().unwrap();

                if set_upstream {
                    let upstream_output = Command::new("git").args(["push", "--set-upstream", "origin", &branch]).output().expect("Failed to push with upstream");

                    println!("{}", String::from_utf8_lossy(&upstream_output.stdout));
                    println!("{}", String::from_utf8_lossy(&upstream_output.stderr));
                }
            } else if !result.stderr.is_empty() {
                println!("⚠️ {}", stderr);
            }
        }
        Err(e) => println!("⚠️ Error: {}", e),
    }
}

// Create a commit
pub fn git_commit() {
    // First show status
    git_status();

    // Ask if they want to add all files
    let add_all = Confirm::new().with_prompt("Do you want to add all files to the commit?").default(false).interact().unwrap();

    if add_all {
        let add_output = Command::new("git").args(["add", "."]).output().expect("Failed to execute git add .");

        if !add_output.stderr.is_empty() {
            println!("⚠️ Error adding files: {}", String::from_utf8_lossy(&add_output.stderr));
            return;
        }
    } else {
        // Let user select specific files to add
        let files_output = Command::new("git").args(["ls-files", "--others", "--modified", "--exclude-standard"]).output().expect("Failed to list files");

        let files_list = String::from_utf8_lossy(&files_output.stdout).lines().map(|s| s.to_string()).collect::<Vec<String>>();

        if files_list.is_empty() {
            println!("No files to add!");
            return;
        }

        println!("Select files to add (press Space to select, Enter to confirm):");
        let selections = MultiSelect::new().items(&files_list).interact().unwrap();

        if selections.is_empty() {
            println!("No files selected!");
            return;
        }

        for &idx in &selections {
            let file = &files_list[idx];
            println!("Adding file: {}", file);

            let add_output = Command::new("git").args(["add", file]).output().expect(&format!("Failed to add file: {}", file));

            if !add_output.stderr.is_empty() {
                println!("⚠️ Error adding file {}: {}", file, String::from_utf8_lossy(&add_output.stderr));
            }
        }
    }

    // Show what's staged
    let diff_output = Command::new("git").args(["diff", "--cached", "--stat"]).output().expect("Failed to show diff");

    println!("\nFiles staged for commit:");
    println!("{}", String::from_utf8_lossy(&diff_output.stdout));

    // Ask for commit message
    let commit_message: String = Input::new().with_prompt("Enter commit message").interact_text().unwrap();

    if commit_message.trim().is_empty() {
        println!("❌ Commit message cannot be empty!");
        return;
    }

    // Create the commit
    let commit_output = Command::new("git").args(["commit", "-m", &commit_message]).output().expect("Failed to create commit");

    println!("{}", String::from_utf8_lossy(&commit_output.stdout));

    if !commit_output.stderr.is_empty() {
        println!("⚠️ {}", String::from_utf8_lossy(&commit_output.stderr));
    }
}

// Show git log
fn git_log() {
    println!("📜 Recent commits:");
    let output = Command::new("git")
        .args(["log", "--oneline", "--graph", "--decorate", "--all", "-n", "10"])
        .output()
        .expect("Failed to execute git log");

    println!("{}", String::from_utf8_lossy(&output.stdout));

    if !output.stderr.is_empty() {
        println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
    }
}

// Configure git settings
fn git_config() {
    println!("⚙️ Git Configuration");

    let options = vec!["Set Username", "Set Email", "Set Remote Origin URL", "Show Current Config", "Back"];

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select configuration option")
        .items(&options)
        .default(0)
        .interact()
        .unwrap();

    match options[selection] {
        "Set Username" => {
            let username: String = Input::new().with_prompt("Enter Git username").interact_text().unwrap();

            if !username.trim().is_empty() {
                let output = Command::new("git").args(["config", "user.name", &username]).output().expect("Failed to set username");

                if output.status.success() {
                    println!("✅ Username set to: {}", username);
                } else {
                    println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
        "Set Email" => {
            let email: String = Input::new().with_prompt("Enter Git email").interact_text().unwrap();

            if !email.trim().is_empty() {
                let output = Command::new("git").args(["config", "user.email", &email]).output().expect("Failed to set email");

                if output.status.success() {
                    println!("✅ Email set to: {}", email);
                } else {
                    println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
        "Set Remote Origin URL" => {
            let url: String = Input::new().with_prompt("Enter remote origin URL").interact_text().unwrap();

            if !url.trim().is_empty() {
                // Check if remote origin exists
                let remote_check = Command::new("git").args(["remote"]).output().expect("Failed to check remotes");

                let remotes = String::from_utf8_lossy(&remote_check.stdout);

                let output = if remotes.contains("origin") {
                    // Set new URL for existing origin
                    Command::new("git").args(["remote", "set-url", "origin", &url]).output().expect("Failed to set remote URL")
                } else {
                    // Add new origin
                    Command::new("git").args(["remote", "add", "origin", &url]).output().expect("Failed to add remote")
                };

                if output.status.success() {
                    println!("✅ Remote origin set to: {}", url);
                } else {
                    println!("⚠️ Error: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
        "Show Current Config" => {
            println!("\nCurrent Git Configuration:");

            // Get username
            let username_output = Command::new("git").args(["config", "user.name"]).output().ok();

            if let Some(output) = username_output {
                if output.status.success() {
                    println!("Username: {}", String::from_utf8_lossy(&output.stdout).trim());
                } else {
                    println!("Username: Not set");
                }
            }

            // Get email
            let email_output = Command::new("git").args(["config", "user.email"]).output().ok();

            if let Some(output) = email_output {
                if output.status.success() {
                    println!("Email: {}", String::from_utf8_lossy(&output.stdout).trim());
                } else {
                    println!("Email: Not set");
                }
            }

            // Get remote URL
            let remote_output = Command::new("git").args(["remote", "get-url", "origin"]).output().ok();

            if let Some(output) = remote_output {
                if output.status.success() {
                    println!("Remote Origin: {}", String::from_utf8_lossy(&output.stdout).trim());
                } else {
                    println!("Remote Origin: Not set");
                }
            }

            // Get current branch
            let branch_output = Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output().ok();

            if let Some(output) = branch_output {
                if output.status.success() {
                    println!("Current Branch: {}", String::from_utf8_lossy(&output.stdout).trim());
                }
            }
        }
        "Back" => (),
        _ => (),
    }
}
