GIX_MANIFEST = vendor/gitoxide/Cargo.toml

.PHONY: build build-agt build-gix

build: build-gix build-agt

build-agt:
	cargo build --release -p agt

build-gix:
	cargo build --release --manifest-path $(GIX_MANIFEST) -p gitoxide
