if [ ! -f "Cargo.toml" ]; then
    echo "Error: This script must be run from the root of a Cargo project"
    exit 1
fi

rm -rf foo
