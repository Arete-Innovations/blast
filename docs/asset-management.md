# Asset Management in Blast

This document describes how the Blast CLI manages assets for Catalyst web applications.

## Overview

Blast handles asset management for Catalyst applications through several commands:

- `blast cdn` - Download required assets from CDNs or source repositories
- `blast scss` - Process and transpile SCSS to CSS
- `blast publish-css` - Publish CSS files with minification
- `blast publish-js` - Process JS files

## Asset Types

### 1. Materialize CSS/JS

#### New Approach in Latest Version

Instead of relying solely on CDN, Blast now clones the Materialize source repository. This provides several advantages:

- **Access to SCSS source**: Allows for deeper customization of themes
- **Offline development**: No need for internet access after initial clone
- **Version control**: Exact control over which version is used
- **Better theming**: Direct access to all Materialize components

#### Implementation Details

When running `blast cdn`, the system:

1. Clones the Materialize GitHub repository (configurable in Catalyst.toml)
2. Uses the source SCSS files for theme customization via imports
3. Copies the compiled JS to the public directory 

If the JS file is not available in the cloned repository, it falls back to downloading from CDN.

#### Configuration in Catalyst.toml

```toml
[assets.materialize]
# The Materialize framework version to use (cloned from GitHub)
version = "1.0.0"
# Where to put the compiled JS file
public_dir = "js/materialize"
# Repository URL for git clone
repo_url = "https://github.com/Dogfalo/materialize.git"
```

#### Directory Structure

```
project/
├── src/
│   ├── assets/
│   │   ├── materialize/    # Cloned Materialize repository
│   │   │   ├── sass/       # Original Materialize SCSS source
│   │   │   └── dist/js/    # Compiled Materialize JS
│   │   └── sass/
│   │       ├── dark.scss   # Theme file that imports from Materialize
│   │       └── components/ # Custom components
│   │           └── forms/
│   │               └── forms.scss
└── public/
    └── js/
        └── materialize/
            └── materialize.min.js
```

#### Theming

Blast uses a single theme file (`src/assets/sass/dark.scss`) that imports from the Materialize source. This allows you to:

1. Set custom theme variables
2. Import only the Materialize components you need
3. Add custom styles on top of Materialize

Example `dark.scss` file:

```scss
// Import Materialize SCSS from the cloned repository
@import '../materialize/sass/components/color-variables';
@import '../materialize/sass/components/color-classes';

// Import our own components
@import './components/forms/forms';

// Set dark theme variables
$primary-color: #2196F3;
$secondary-color: #26a69a;
$background-color: #121212;
$surface-color: #1e1e1e; 
$text-color: #e0e0e0;
$card-bg-color: #2d2d2d;

// Import necessary Materialize components
@import '../materialize/sass/components/variables';
@import '../materialize/sass/components/global';
@import '../materialize/sass/components/badges';
// ... other components as needed

// Custom theme overrides
body {
  background-color: $background-color;
  color: $text-color;
}
```

### 2. Other Assets

The following assets are still downloaded from CDNs:

#### FontAwesome

FontAwesome icons are downloaded from CDN and stored in the public directory.

Configuration in Catalyst.toml:
```toml
[assets.fontawesome]
base_url = "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1"
public_dir = "fonts/fontawesome"
css = ["css/all.min.css"]
js = ["js/all.min.js"]
sprites = ["sprites/brands.svg", "sprites/regular.svg", "sprites/solid.svg"]
webfonts = [
  "webfonts/fa-brands-400.ttf",
  "webfonts/fa-brands-400.woff2",
  "webfonts/fa-regular-400.ttf",
  "webfonts/fa-regular-400.woff2",
  "webfonts/fa-solid-900.ttf",
  "webfonts/fa-solid-900.woff2",
  "webfonts/fa-v4compatibility.ttf",
  "webfonts/fa-v4compatibility.woff2"
]
```

#### Material Icons

Google Material Icons fonts are downloaded from GitHub and stored in the public directory.

Configuration in Catalyst.toml:
```toml
[assets.materialicons]
base_url = "https://raw.githubusercontent.com/google/material-design-icons/master/font"
public_dir = "fonts/material-icons"
woff2 = "MaterialIcons-Regular.woff2"
ttf = "MaterialIcons-Regular.ttf"
```

#### HTMX

HTMX JavaScript library is downloaded from CDN and stored in the public directory.

Configuration in Catalyst.toml:
```toml
[assets.htmx]
js_url = "https://cdnjs.cloudflare.com/ajax/libs/htmx/2.0.4/htmx.min.js"
public_dir = "js/htmx"
```

## SCSS Processing

When running `blast scss`, the system:

1. Finds all `.scss` files in the `src/assets/sass` directory
2. Compiles them to CSS using the sass-rs crate
3. In production mode, compresses the output
4. Saves the compiled CSS to `public/css/*.min.css`

## CSS Publishing

When running `blast publish-css`, the system:

1. Finds all `.css` files in the `src/assets/css` directory
2. In production mode, minifies them using css-minify
3. Saves them to `public/css/app/*.min.css`

## JS Processing

When running `blast publish-js`, the system:

1. Finds all `.js` files in the `src/assets/js` directory
2. In production mode, would minify them (future feature)
3. Saves them to `public/js/app/*.min.js`

## Environment Variables

- `BLAST_INTERACTIVE=1` - Enable interactive mode (dashboard)
- `BLAST_FORCE_FRESH_MATERIALIZE=1` - Force a fresh clone of Materialize repository

## Troubleshooting

### Git Clone Issues

If you encounter issues with git clone:
1. Check if git is installed and in your PATH
2. Verify the version specified in Catalyst.toml exists (e.g., "1.0.0")
3. Ensure you have internet access
4. Try setting `BLAST_FORCE_FRESH_MATERIALIZE=1` to force a fresh clone

### SCSS Compilation Issues

If SCSS compilation fails:
1. Check if the Materialize repository was cloned correctly
2. Verify the import paths in your SCSS files
3. Ensure the sass-rs crate is working properly