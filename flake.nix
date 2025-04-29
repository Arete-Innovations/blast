{
	description = "Blast Development Environment";

	inputs = {
		nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
		flake-utils.url = "github:numtide/flake-utils";
		rust-overlay.url = "github:oxalica/rust-overlay";
	};

	outputs = { self, nixpkgs, flake-utils, rust-overlay }:
		flake-utils.lib.eachDefaultSystem (system:
			let
				pkgs = import nixpkgs {
					system = system;
					overlays = [ rust-overlay.overlays.default ];
				};
				rustToolchain = pkgs.rust-bin.stable.latest.default;
			in {
				devShells.default =
					pkgs.mkShell {
						nativeBuildInputs = with pkgs; [
							rustToolchain
							rust-analyzer
							rustfmt
							openssl
							postgresql
							pkg-config
							diesel-cli
							diesel-cli-ext
						];
					};

				packages = rec {
					blast = pkgs.rustPlatform.buildRustPackage {
						pname = "blast";
						version = "0.1.0";

						src = ./.;

						buildInputs = with pkgs; [
							openssl
							postgresql
						];
						nativeBuildInputs = with pkgs; [
							pkg-config
						];

						cargoLock = {
							lockFile = ./Cargo.lock;
						};
					};

					default = blast;
				};
			}
		);
}
