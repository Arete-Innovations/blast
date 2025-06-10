echo "Testing blast init"

if [ ! -f "Cargo.toml" ]; then
    echo "Error: This script must be run from the root of a Cargo project"
    exit 1
fi

cargo run -- new foo

cd foo
cargo run --manifest-path=../Cargo.toml init
