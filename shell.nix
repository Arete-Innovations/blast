{
	pkgs ? import <nixpkgs> { }
}:

pkgs.mkShell {
	buildInputs = with pkgs; [
		cargo
		rustc
		rust-analyzer
		rustfmt
		openssl
		postgresql
		pkg-config
	];
}

