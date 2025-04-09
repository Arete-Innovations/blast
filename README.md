# 💥 Blast CLI Tool

![License](https://img.shields.io/badge/license-AGPL--3.0-blue)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## 🌟 Overview

Blast is a powerful CLI utility tool for managing [Catalyst](https://github.com/Arete-Innovations/catalyst) web applications. It streamlines development workflow with code generation, asset management, and project automation.

## 🧘 The "Suckless" Philosophy

Blast embraces the [suckless philosophy](https://suckless.org/philosophy/) with these core principles:

### 🔍 Simplicity
- Code should be simple, minimal, and readable
- Avoid unnecessary abstractions and dependencies
- Prefer explicit over implicit behavior
- Less code means fewer bugs and easier maintenance

### 🛠️ Modularity
- Small, focused components that do one thing well
- Compose functionality from minimal building blocks
- Easy to understand, extend, and replace parts
- Clear separation between generated and custom code

### 🎯 Pragmatism
- Practical solutions over theoretical purity
- Embrace proven technologies instead of trendy frameworks
- Focus on developer productivity and maintainable code
- Balance between hand-written and generated code

## 📋 Features

### 🔄 Project Management
- 🆕 Create new projects with `blast new [project_name]`
- 🧩 Scaffold controllers, models, and views
- 🛠️ Interactive dashboard mode for project management
- 🔍 Comprehensive configuration management
- 🪝 Post-generation hooks for custom scripts and automation

### 💾 Database Operations
- 📊 Generate schemas from existing databases
- 📝 Interactive migration creation
- 🏗️ Model generation with consistent CRUD methods
- 🧪 Struct generation (NewStruct insertable types)

### 🌐 Frontend Assets
- 📦 Asset management with git source repositories and CDN fallbacks
- 🌍 Locale/internationalization system
- 🎭 SCSS transpiling with automatic minification
- 📱 Responsive design helpers
- 📊 Consistent asset organization in css/js/fonts folders
- 🧩 Simplified importing with .min.css/.min.js convention
- 🎨 Customizable theming with direct access to Materialize SCSS source

### 🧰 Development Tools
- 🏃‍♂️ Development server with hot reloading
- 📝 Code generation utilities
- 🔌 Editor integration
- 🔄 Git workflow support
- 📦 Cargo dependency management with crates.io search

## 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/Arete-Innovations/blast
cd blast

# Install the blast binary
./install_blast.sh
```

Make sure `~/.local/bin` is in your PATH.

### Template Repository

Blast uses remote Git repositories for templates instead of embedding them in the binary. When you create a new project, Blast will:

1. Clone the template from one of the following repositories (with automatic fallback):
   - Primary: https://github.com/Arete-Innovations/catalyst-template.git
   - Fallback 1: https://gitlab.com/Arete-Innovations/catalyst-template.git
   - Fallback 2: https://bitbucket.org/Arete-Innovations/catalyst-template.git

2. Configure the cloned template with your project name
3. Initialize it as a new Git repository

This approach allows for more flexibility and easier template updates without requiring a new Blast release.

## 🛠️ Usage

### Creating a New Project

```bash
# Create a new project
blast new my_project

# Change to the project directory
cd my_project

# Initialize project (migrations, seeds, assets, code generation)
blast init
```

### Running the Dashboard

```bash
# Start the interactive dashboard
blast dashboard
```

### Managing Configuration

```bash
# Toggle between development and production
blast env toggle
```

### Code Generation

```bash
# Generate a model from database
blast gen models

# Generate structs for models
blast gen structs

# Create a migration
blast migration

# Add a dependency with crates.io search
blast cargo add serde

# Remove dependencies interactively
blast cargo remove
```

### Asset Management

```bash
# Transpile SCSS to CSS
blast scss

# Minify CSS files
blast css

# Process JS files
blast js

# Download assets (now supports git repository cloning for Materialize)
blast cdn

# Manage locale/i18n
blast locale-manager
```

### Running Your Application

```bash
# Start the development server
blast serve

# Start with production settings
blast serve --production
```

### Log Management

```bash
# Truncate all logs
blast log truncate

# Truncate specific log
blast log truncate server.log
```

### Git Integration

```bash
# Launch Git manager
blast git

# Show repository status
blast git status

# Commit changes
blast git commit
```

## 📜 Log Management

Blast provides tools to manage your application logs efficiently:

- **Log Truncation**: Easily clear log files to prevent them from growing too large
- **Log Storage**:
  - Application logs in `storage/logs/` directory
  - Dashboard log in `storage/blast/blast.log`

## ⏱️ Cronjob Management

Blast provides a complete system for managing scheduled tasks:

- **Interactive TUI**: Full-featured terminal interface for managing cronjobs with dialoguer/indicatif
- **Dashboard Integration**: Dedicated cronjobs tab showing status, last run, and next run times
- **Command-line Management**: Add, toggle, and remove jobs with simple commands
- **Status Monitoring**: Track job execution and failures with dedicated logs
- **Commands**:
  - `blast cronjobs`: Launch the interactive TUI cronjob manager
  - `blast cronjobs list`: Display all scheduled jobs and their status
  - `blast cronjobs add <name> <interval>`: Add a new cronjob with name and interval in seconds
  - `blast cronjobs toggle <id>`: Toggle a job's active/paused status
  - `blast cronjobs remove <id>`: Remove a scheduled job

The interactive TUI lets you:
- View colorized job status and details
- Add new jobs with interactive prompts
- Toggle job active/paused status
- Remove jobs with confirmation dialog
- Navigate with fuzzy search selection
- See real-time progress with spinners

## 🔄 Git Integration

Blast provides Git configuration directly from the CLI:

- Set up remote repository URL
- Configure Git username and email
- Apply Git settings to the local repository
- Initialize new projects with Git automatically
- Interactive Git operations through dashboard

## 📦 Dependency Management

Blast includes cargo dependency management:

- Search crates.io for packages
- View download statistics and descriptions
- Add dependencies with version selection
- Interactively remove packages
- Manage workspace members
- Auto-update after adding dependencies

## 🎨 Asset Management

Blast provides a comprehensive asset management system:

### 📚 Materialize CSS/JS
- 🧵 Clones Materialize source repository from GitHub
- 🛠️ Uses source SCSS files for advanced theming
- 📐 Single customizable dark theme as reference
- 🔄 Falls back to CDN if git clone fails
- ⚙️ Configurable version and repository URL

### 🎭 Other Assets
- 📦 FontAwesome icons (CDN download)
- 📱 Material Icons (GitHub download)
- 🌐 HTMX for dynamic content (CDN download)

### 🔧 Environment Variables
- `BLAST_FORCE_FRESH_MATERIALIZE=1` - Force fresh clone of Materialize repository

For detailed documentation on the asset system, see the [asset management guide](docs/asset-management.md).

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📜 License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0) - see the LICENSE file for details.

The AGPL-3.0 is a strong copyleft license that requires making the complete source code available to users who interact with the software over a network. This ensures that all modifications and improvements remain free and open source.