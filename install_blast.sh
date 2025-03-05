#!/bin/bash
# Installation script for blast

echo "Cleaning template directory artifacts..."
if [ -d "template/target" ]; then
  echo "Found target directory in template, removing..."
  rm -rf template/target
fi

# Also clean any other potential artifacts in the template
if [ -d "template/.git" ]; then
  echo "Found .git directory in template, removing..."
  rm -rf template/.git
fi

# Clean Cargo artifacts like Cargo.lock if they exist
if [ -f "template/Cargo.lock" ]; then
  echo "Found Cargo.lock in template, removing..."
  rm -f template/Cargo.lock
fi

echo "Building blast in release mode..."
cargo build --release

echo "Installing blast to ~/.local/bin..."
mkdir -p ~/.local/bin
cp target/release/blast ~/.local/bin/blast

if [ $? -eq 0 ]; then
  echo "Installation successful!"
  echo "Make sure ~/.local/bin is in your PATH."
  
  # Check if ~/.local/bin is in PATH
  if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo "Warning: ~/.local/bin is not in your PATH."
    echo "You may want to add it by adding this line to your ~/.bashrc or ~/.zshrc:"
    echo 'export PATH="$HOME/.local/bin:$PATH"'
  fi
else
  echo "Installation failed."
fi