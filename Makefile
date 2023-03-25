all:
	cargo build --release

install:
	cargo install --path .

local:
	cargo build --config 'patch.crates-io.stam.path="../stam-rust/"'
