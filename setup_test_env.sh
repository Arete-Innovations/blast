#!/bin/bash
# Test environment setup script for blast features

# Check if a project name was provided
if [ $# -eq 0 ]; then
  PROJECT_NAME="testproject"
  echo "No project name specified, using default: $PROJECT_NAME"
else
  PROJECT_NAME="$1"
  echo "Setting up test environment for project: $PROJECT_NAME"
fi

# Set up directories
mkdir -p tests
cd tests

# Clean up previous test project if it exists
if [ -d "$PROJECT_NAME" ]; then
    echo "Removing existing test project: $PROJECT_NAME"
    # Make sure any lock files are removed first to avoid permission issues
    find "$PROJECT_NAME" -name "*.lock" -type f -delete 2>/dev/null
    # Force remove write-protected files if needed (e.g., compiled binaries)
    chmod -R +w "$PROJECT_NAME" 2>/dev/null
    # Remove the directory recursively
    rm -rf "$PROJECT_NAME"
fi

# First build the main blast tool
echo "Building blast tool..."
cd ..
cargo build --release

# Create a fresh test project
echo "Creating fresh test project..."
cd tests
../target/release/blast new "$PROJECT_NAME"
cd ..

echo "Test project created at tests/$PROJECT_NAME"
echo ""
echo "To test your changes:"
echo "1. cd tests/$PROJECT_NAME"
echo "2. Make your changes"
echo "3. cargo build --release"
echo ""
echo "When finished, update the template and rebuild blast:"
echo "4. Update template files"
echo "5. cd ../.. (back to blast root)"
echo "6. cargo build --release"
echo "7. mkdir -p ~/.local/bin"
echo "8. cp target/release/blast ~/.local/bin/blast"

# Return to the original directory
cd tests