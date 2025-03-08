# 💥 Blast the CLI Tool for
# 🔥Catalyst Web Framework

![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/language-Rust-orange)

## 🌟 Overview

Blast is a powerful CLI utility tool for managing Catalyst webapp framework built with:

- 🚀 Rocket for web server
- ⛽ Diesel for ORM/database
- 🔧 Tera templates for views
- ⚡ HTMX for interactive frontend
- 🎨 MaterializeCSS for styling



## 🧘 The "Suckless" Philosophy

This framework embraces the [suckless philosophy](https://suckless.org/philosophy/) with these core principles:

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

### 🚀 Performance
- Lightweight and efficient implementations
- Minimal runtime overhead
- Server-side rendering with targeted interactivity
- Built with Rust for memory safety and speed

### 🧠 Mental Model
- Consistent patterns throughout the codebase
- Clear, predictable structure
- Low cognitive load for developers
- Easy to reason about the system holistically

This approach results in a framework that's powerful enough for real-world applications, yet simple enough to fully understand and customize to your specific needs.

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
- 📦 CDN asset management and downloading
- 🌍 Locale/internationalization system
- 🎭 SCSS transpiling with automatic minification
- 📱 Responsive design helpers
- 📊 Consistent asset organization in css/js/fonts folders
- 🧩 Simplified importing with .min.css/.min.js convention

### 🧰 Development Tools
- 🏃‍♂️ Development server with hot reloading
- 📝 Code generation utilities
- 🔌 Editor integration
- 🔄 Git workflow support
- 📦 Cargo dependency management with crates.io search
- 🔄 Post-generation hooks for custom scripts

## 🚀 Installation

```bash
# Clone the repository
git clone https://github.com/Arete-Innovations/blast
cd blast

# Install the blast binary
./install_blast.sh
```

Make sure `~/.local/bin` is in your PATH.

## 🛠️ Usage

### Creating a New Project

```bash
# Create a new project
blast new my_project

# Change to the project directory
cd my_project
```

### Running the Dashboard

```bash
# Start the interactive dashboard
blast dashboard
```

### Managing Configuration

```bash
# Edit project configuration
blast config

# Toggle between development and production
blast env toggle
```

### Code Generation

```bash
# Generate a model from database
blast generate model User

# Generate struct for a model
blast generate struct User

# Create a migration
blast migration create

# Add a dependency with crates.io search
blast cargo add serde

# Remove dependencies interactively
blast cargo remove
```

### Running Your Application

```bash
# Start the development server
blast serve

# Start with production settings
blast serve --production
```

## 📊 Project Structure

Blast manages projects with the following default structure:

```
my_project/
├── Cargo.toml                # Rust dependencies
├── Catalyst.toml             # Framework configuration
├── Rocket.toml               # Rocket server configuration
├── diesel.toml               # Diesel ORM configuration
├── src/
│   ├── main.rs               # Application entry point
│   ├── lib.rs                # Library exports
│   │
│   ├── routes/               # Routes
│   │   ├── mod.rs            # Route module exports
│   │   ├── public/           # Public routes accessible without authentication
│   │   │   ├── auth.rs       # Authentication routes (login, register)
│   │   │   ├── home.rs       # Homepage and public pages
│   │   │   └── mod.rs        # Public routes module exports
│   │   └── private/          # Protected routes requiring authentication
│   │       ├── admin.rs      # Admin panel routes
│   │       ├── api.rs        # API endpoints
│   │       ├── user.rs       # User dashboard routes
│   │       └── mod.rs        # Private routes module exports
│   │
│   ├── models/               # Database models
│   │   ├── mod.rs            # Model module exports
│   │   ├── auth/             # Authentication models
│   │   │   ├── users.rs      # User model with authentication methods
│   │   │   └── mod.rs        # Auth models exports
│   │   ├── custom/           # Custom user-defined models (never overwritten)
│   │   │   └── mod.rs        # Custom models exports
│   │   └── generated/        # Auto-generated models (don't edit!)
│   │       └── mod.rs        # Generated models exports
│   │
│   ├── structs/              # Data structures
│   │   ├── mod.rs            # Struct module exports
│   │   ├── auth/             # Authentication structs
│   │   │   ├── users.rs      # User structs (DTO, form structs)
│   │   │   └── mod.rs        # Auth structs exports
│   │   ├── custom/           # Custom user-defined structs (never overwritten)
│   │   │   ├── insertable/   # Custom insertable structs
│   │   │   └── mod.rs        # Custom structs exports
│   │   └── generated/        # Auto-generated structs (don't edit!)
│   │       ├── insertable/   # Generated insertable structs
│   │       │   └── mod.rs    # Generated insertable exports
│   │       └── mod.rs        # Generated structs exports
│   │
│   ├── middleware/           # Rocket request guards and middleware
│   │   ├── mod.rs            # Middleware module exports
│   │   ├── catchers.rs       # Error handlers
│   │   ├── guards.rs         # Authentication guards
│   │   ├── jwt.rs            # JWT token handling
│   │   ├── cache.rs          # Response caching
│   │   └── compress.rs       # Response compression
│   │
│   ├── services/             # Business logic layer
│   │   ├── mod.rs            # Services module exports 
│   │   ├── builders/         # UI builders for components
│   │   │   ├── context.rs    # Template context builder
│   │   │   ├── list.rs       # List component builder
│   │   │   ├── select.rs     # Dropdown builder
│   │   │   ├── table.rs      # HTML table builder
│   │   │   └── mod.rs        # Builder module exports
│   │   ├── logger.rs         # Logging services
│   │   ├── storage.rs        # File storage services
│   │   └── cronjobs.rs       # Scheduled tasks
│   │
│   ├── assets/               # Frontend assets
│   │   ├── css/              # CSS files
│   │   ├── js/               # JavaScript files
│   │   ├── img/              # Images
│   │   ├── locale/           # Internationalization files
│   │   │   └── en.json       # English translations
│   │   └── sass/             # SCSS source files
│   │       └── components/   # SCSS components
│   │
│   ├── database/             # Database connection and migrations
│   │   ├── mod.rs            # Database module exports
│   │   ├── db.rs             # Connection pool management
│   │   ├── schema.rs         # Database schema (generated by Diesel)
│   │   ├── migrations/       # Database migrations
│   │   └── seeds/            # Seed data for database
│   │
│   └── bin/                  # Cronjob executables
│
├── templates/                # Tera template files
│   ├── index.html.tera       # Main index template
│   ├── admin/                # Admin panel templates
│   │   └── index.html.tera   # Admin dashboard
│   ├── auth/                 # Authentication templates
│   │   ├── login.html.tera   # Login form
│   │   └── register.html.tera # Registration form
│   ├── user/                 # User dashboard templates
│   │   └── index.html.tera   # User homepage
│   ├── oops/                 # Error pages
│   │   └── index.html.tera   # Error page template
│   └── partials/             # Reusable template parts
│       ├── header.tera       # Page header
│       ├── footer.tera       # Page footer
│       └── navbar.tera       # Navigation bar
│
├── public/                   # Static files (auto-generated)
│
└── storage/                  # Storage directory
    ├── logs/                 # Log files
    │   ├── debug.log         # Debug level logs
    │   ├── error.log         # Error level logs
    │   ├── info.log          # Info level logs
    │   ├── server.log        # Server logs
    │   └── warning.log       # Warning level logs
    └── blast/                # Blast utilities
```

### 🔐 Authentication System

The template includes a robust authentication system in the `auth/` directories:

- **User Model** (`models/auth/users.rs`): Complete user management with:
  - Password hashing and verification using bcrypt
  - Role-based permissions (admin, user, custom roles)
  - Account activation/deactivation
  - Profile management
  - Password reset workflows
  - Security features (transaction-based updates)

- **Auth Routes** (`routes/public/auth.rs`): Ready-to-use authentication endpoints:
  - Login and registration
  - Password reset/recovery
  - Account verification

- **Auth Middleware** (`middleware/guards.rs` & `middleware/jwt.rs`):
  - JWT token authentication
  - Role-based authorization guards
  - Session management

- **Auth Templates** (`templates/auth/`):
  - Login and registration forms
  - Password reset interfaces

### 📁 Generated vs. Custom Folders

The framework follows a clear separation between generated and custom code:

- **Generated Folders** (`*/generated/`):
  - ⚠️ Files in these directories are automatically generated by Blast
  - ⚠️ Manual changes will be overwritten when running generation commands
  - Generated from database schema and migrations
  - Contains boilerplate CRUD operations and database models

- **Custom Folders** (`*/custom/`):
  - ✅ Safe places for your custom implementation
  - Never overwritten by the framework
  - Automatically imported via the module system
  - Ideal for business logic and application-specific code

This separation ensures that you can regenerate database models without losing your custom implementation logic.

### 📝 The `cata_log!` Macro

The `cata_log!` macro provides structured logging with source location tracking:

```rust
// Basic usage with different log levels
cata_log!(Debug, "Processing user request");
cata_log!(Info, "User logged in successfully");
cata_log!(Warning, "Rate limit approaching");
cata_log!(Error, "Database connection failed");
cata_log!(Trace, "Function enter: process_payment");
```

The actual implementation from `services/logger.rs` provides:

- 🔄 Automatic file name, line number, and module path inclusion
- 🎨 Color-coded output in the console by log level
- ⏱️ Unix timestamp and local time with timezone
- 🗂️ Automatic log file organization by log level
- 💾 File logging without ANSI color codes
- 🚨 Integrated panic hook for capturing application crashes
- 🔄 Automatic log rotation and organization

Example output format:

```
1708562309-2025-01-01 12:34:56 EST [INFO] 🔍 [src/routes/auth.rs:42::login] User login successful: johndoe
```

### 🏛️ Framework Architecture

The framework follows a clear layered architecture that separates concerns:

1. **Routing Layer** (via Rocket in `routes/`):
   - Handles HTTP requests and routing
   - Separated into public and private routes
   - Uses Rocket's path, query, and form extractors
   - Returns appropriate responses (Template, JSON, Redirect)

2. **Middleware Layer** (in `middleware/`):
   - JWT authentication and token validation
   - Request guards for authorization
   - Response caching and compression
   - Error catchers for handling exceptions

3. **Service Layer** (in `services/`):
   - Contains business logic separate from routes
   - Provides UI component builders
   - Manages file storage operations
   - Implements logging services and scheduled tasks

4. **Model Layer** (in `models/`):
   - Implements domain logic with database operations
   - Uses transaction-based operations for data integrity
   - Provides CRUD operations and validation
   - Authentication and user management

5. **Data Layer** (via Diesel ORM in `database/`):
   - Schema definitions and migrations
   - Connection pooling and management
   - Seed data for initial setup

6. **View Layer** (via Tera templates in `templates/`):
   - Organizes templates by functional area
   - Uses partials for reusable components
   - Provides layouts and includes

7. **Asset Management** (in `assets/` and compiled to `public/`):
   - **CSS Management**:
     - SCSS compilation with automatic minification
     - All CSS files output as .min.css for consistent imports
     - Custom CSS files from src/assets/css
   - **JS Processing**:
     - All JS files output as .min.js
     - Organized in library-specific folders
   - **Directory Structure**:
     - `/public/css/` - For all CSS files
     - `/public/js/` - For all JavaScript files
     - `/public/fonts/` - For all font files
   - **Environment Handling**:
     - Production: Minified content with .min.css/.min.js extensions
     - Development: Readable content with .min.css/.min.js extensions

This architecture makes the codebase maintainable and testable by ensuring each component has a single responsibility and clear boundaries between layers.

## 📝 Configuration System

Blast uses `Catalyst.toml` as its main configuration file with a hierarchical structure that controls all aspects of your application:

### Core Settings

```toml
[settings]
# Controls environment-specific behavior (dev/prod)
environment = "dev"
# Toggle compiler warnings display in console
show_compiler_warnings = false
# Project name used in many auto-generated features
project_name = "my_awesome_app"
```

### Code Generation Configuration

```toml
[codegen]
# Directory paths for generated code
structs_dir = "src/structs/generated"
models_dir = "src/models/generated"
schema_file = "src/database/schema.rs"

# Post-generation hooks - scripts to run after code generation
[codegen.hooks]
enabled = true
post_structs = [
  "scripts/format_structs.sh",
  "cargo fmt"
]
post_models = [
  "scripts/validate_models.sh"
]
post_any = [
  "scripts/notify_generation_complete.sh"
]

# Tables to ignore in code generation
[codegen.models]
ignore = ["migrations", "schema_migrations"]

# Struct generation configuration
[codegen.structs]
# Tables to ignore in struct generation 
ignore = ["migrations", "schema_migrations"]
# Traits to derive on generated structs
derives = [
  "Debug",
  "Queryable",
  "Clone", 
  "Serialize", 
  "Deserialize"
]
# Automatic imports for generated structs
imports = [
  "serde::Serialize",
  "serde::Deserialize",
  "diesel::Queryable"
]
```


### Git Configuration

```toml
[git]
remote_url = "https://github.com/username/repo.git"
username = "Your Name"
email = "your.email@example.com"
```


## 🌍 Locale Management System

Blast includes a sophisticated internationalization system that's fully integrated with the framework:

### 🗂️ Structure and Organization

Locale files are JSON-structured with a hierarchical organization:

```json
{
  "app": {
    "name": "My Application",
    "welcome": "Welcome to our app!"
  },
  "nav": {
    "home": "Home",
    "about": "About",
    "login": "Log in",
    "register": "Register"
  },
  "pages": {
    "home": {
      "title": "Welcome Home",
      "subtitle": "Your journey starts here"
    },
    "about": {
      "title": "About Us",
      "team": {
        "title": "Our Team",
        "description": "Meet the people who make it happen"
      }
    }
  },
  "errors": {
    "notFound": "Page not found",
    "serverError": "Something went wrong"
  }
}
```

### 🖥️ Management Interface

The locale management system provides:

- **Interactive CLI**: Easy navigation through locale keys
- **TUI Dashboard**: Full-screen management interface with tree view
- **Tree Visualization**: Visual representation of your locale hierarchy
- **Multi-language Support**: Add, edit, and compare languages side-by-side
- **Key Operations**: Add, edit, or delete translation keys across all languages
- **Page Management**: Streamlined process for adding new page titles and content


### 🚀 Advanced Features

- **Language Detection**: Automatic language detection from browser settings
- **Fallbacks**: Configurable fallback languages when translations are missing
- **Dev Mode**: Shows missing translation keys in development
- **Pluralization Rules**: Built-in support for proper pluralization
- **Language Switching**: Easy language switching with session persistence

## ⚡ HTMX Integration

Blast integrates [HTMX](https://htmx.org/) for creating dynamic, interactive web experiences with minimal JavaScript:

### 🧩 Server-Side Templates With Client-Side Interactivity

The framework encourages using HTMX for common web interactions:

```html
<!-- Example of a dynamic form submission -->
<form hx-post="/auth/login" hx-target="#response-div">
  <input name="username" type="text">
  <input name="password" type="password">
  <button type="submit">Login</button>
</form>

<!-- Content loading when element enters viewport -->
<div hx-get="/api/dashboard-stats" hx-trigger="revealed">
  <p>Loading dashboard stats...</p>
</div>

<!-- Lazy-loaded content -->
<div hx-get="/api/recent-activity" hx-trigger="load delay:500ms">
  <div class="spinner"></div>
</div>
```


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

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📜 License

This project is licensed under the MIT License - see the LICENSE file for details.

