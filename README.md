# 💥 Blast CLI Tool

![License](https://img.shields.io/badge/license-AGPL--3.0-blue)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## 🌟 Overview

Blast is a powerful CLI utility tool for managing [Catalyst](https://github.com/Arete-Innovations/catalyst) web applications. It streamlines development workflow with code generation, asset management, and project automation. The Catalyst framework follows a "suckless" philosophy, emphasizing simplicity, modularity, and performance.

## 📋 Features

### 🔄 Project Management
- 🆕 Create new projects with `blast new [project_name]`
- 🧩 Scaffold controllers, models, and views
- 🛠️ Interactive dashboard mode for project management
- 🔍 Comprehensive configuration management
- 🪝 Post-generation hooks for custom scripts and automation

### 💾 Database Operations
- 📊 Generate schemas from existing databases
- 📝 Interactive migration creation and management
- 🏗️ Model generation with consistent CRUD methods
- 🧪 Struct generation (NewStruct insertable types)
- 🗄️ Database seeding with support for specific seed files

### 🌐 Frontend Assets
- 📦 Asset management with git source repositories and CDN fallbacks
- 🌍 Locale/internationalization system
- 🎭 SCSS transpiling with automatic minification
- 📱 Responsive design helpers
- 📊 Consistent asset organization in css/js/fonts folders
- 🧩 Simplified importing with .min.css/.min.js convention
- 🎨 Customizable theming with direct access to Materialize SCSS source

### ⏱️ Cronjob Management
- 📊 Interactive TUI for managing scheduled tasks
- 🕒 Status tracking with last and next run times
- 📝 Dedicated logging for cronjob execution
- 🔄 Toggle jobs active/inactive without removing them
- 📋 Live table view with auto-refresh

### 🧰 Development Tools
- 🏃‍♂️ Development server with multiple run modes
- 👀 Watch mode for auto-restarting on code changes
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

# Use development branch (latest features)
blast new my_project --dev

# Change to the project directory
cd my_project

# Initialize project (migrations, seeds, assets, code generation)
blast init
```

### Running the Dashboard

```bash
# Start the interactive dashboard (default when run without arguments)
blast

# Explicitly start the dashboard
blast dashboard

# Run the interactive CLI
blast cli
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

# Publish CSS to public directory
blast publish-css

# Process JS files
blast js

# Download assets (git repo cloning for Materialize, CDN for others)
blast cdn
```

### Running Your Application

```bash
# Start the development server
blast run
# Or
blast serve

# Start with production settings
blast run-prod
# Or
blast serve-prod

# Stop a running server
blast stop

# Watch mode - auto-restart on code changes
blast watch
```

### Log Management

```bash
# Truncate all logs
blast log truncate

# Truncate specific log
blast log truncate server.log
```

### Cronjob Management

```bash
# Launch interactive TUI cronjob manager
blast cronjobs

# List all scheduled jobs
blast cronjobs list

# Add a new cronjob (name, interval in seconds)
blast cronjobs add job_name 300

# Toggle a job's active status
blast cronjobs toggle 1

# Remove a scheduled job
blast cronjobs remove 1

# Display live auto-refreshing table
blast cronjobs table
```

### Spark Plugins

Sparks are modular plugins that can be added to your Catalyst application:

```bash
# Add a spark plugin from a git repository
blast spark add https://github.com/user/repo
```

Sparks can also be defined in your Catalyst.toml configuration:

```toml
[sparks]
auth = "https://github.com/catalyst-framework/auth"
plznohac = "https://github.com/catalyst-framework/plznohac"
```

## 📁 Project Structure

When you create a new Catalyst project with Blast, it follows a clear separation between generated and custom code:

```
my_project/
├── Cargo.toml              # Rust project dependencies
├── Catalyst.toml           # Framework configuration
├── Rocket.toml             # Web server configuration  
├── diesel.toml             # ORM configuration
├── public/                 # Public web assets
│   ├── css/                # Compiled/minified CSS
│   │   └── app/
│   ├── fonts/              # Font resources
│   │   ├── fontawesome/
│   │   └── material-icons/
│   └── js/                 # Compiled/minified JS
│       ├── app/
│       ├── htmx/
│       └── materialize/
├── src/
│   ├── assets/             # Frontend source assets
│   │   ├── css/            # CSS source files
│   │   ├── js/             # JavaScript source files
│   │   ├── locale/         # Internationalization JSON files
│   │   ├── materialize/    # Materialize SCSS source
│   │   │   └── sass/       # SCSS components
│   │   └── sass/           # SCSS source files
│   ├── bootstrap.rs        # Application bootstrapping
│   ├── database/           # Database management
│   │   ├── db.rs           # Database connection pool
│   │   ├── migrations/     # Database migrations
│   │   │   └── ...         # Migration directories with up.sql/down.sql
│   │   ├── schema.rs       # Generated DB schema
│   │   └── seeds/          # Database seed files
│   ├── lib.rs              # Library entry point
│   ├── main.rs             # Application entry point
│   ├── middleware/         # Request/response middleware
│   │   ├── api_logger.rs   # API request logging
│   │   ├── app_context.rs  # Application context
│   │   ├── cache.rs        # Response caching
│   │   ├── catchers.rs     # Error catchers
│   │   ├── compress.rs     # Response compression
│   │   ├── guards.rs       # Request guards
│   │   ├── htmx.rs         # HTMX integration
│   │   └── jwt.rs          # JWT authentication
│   ├── models/             # Database models
│   │   ├── auth/           # Authentication models
│   │   ├── custom/         # Your custom models (never overwritten)
│   │   └── generated/      # Generated models (can be overwritten)
│   ├── routes/             # Route handlers
│   │   ├── admin.rs        # Admin routes
│   │   ├── api/            # API routes
│   │   │   ├── v1.rs       # API version 1
│   │   │   └── ...         # Other API endpoints
│   │   ├── home.rs         # Homepage routes
│   │   └── user.rs         # User routes
│   ├── services/           # Business logic services
│   │   ├── builders/       # Query builders
│   │   ├── context/        # Context services
│   │   ├── default/        # Default services
│   │   └── sparks/         # Spark plugins
│   │       ├── makeuse.rs  # Utility for sparks
│   │       ├── plznohac/   # Security spark
│   │       └── vigil/      # Monitoring spark
│   └── structs/            # Data structures
│       ├── auth/           # Authentication structs
│       ├── custom/         # Your custom structs (never overwritten)
│       └── generated/      # Generated structs (can be overwritten)
│           └── insertable/ # NewStruct types for insertions
├── storage/                # Storage directory
│   ├── blast/              # Blast-specific files
│   │   ├── blast.log       # Blast tool log
│   │   └── dashboard.kdl   # Dashboard configuration
│   └── logs/               # Application logs
│       ├── debug.log       # Debug level logs
│       ├── error.log       # Error level logs
│       ├── info.log        # Info level logs
│       ├── server.log      # Server output log
│       └── warning.log     # Warning level logs
└── templates/              # Tera templates for views
    ├── admin/              # Admin area templates
    ├── auth/               # Authentication templates
    ├── oops/               # Error pages
    ├── partials/           # Shared template components
    │   ├── footer.tera
    │   ├── header.tera
    │   └── navbar.tera
    └── user/               # User area templates
```

## 🔄 "Suckless" Philosophy

The Catalyst framework and Blast CLI follow the "suckless" philosophy:

- **Simplicity**: Minimalist code with clear purpose
- **Modularity**: Small components that do one thing well
- **Pragmatism**: Practical solutions over theoretical purity
- **Performance**: Lightweight and efficient implementation
- **Mental Model**: Consistent patterns throughout the codebase

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
