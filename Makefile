GIX_MANIFEST = vendor/gitoxide/Cargo.toml

.PHONY: build build-agt build-gix build-worktree

build: build-gix build-worktree build-agt

build-agt:
	cargo build --release -p agt

build-gix:
	cargo build --release --manifest-path $(GIX_MANIFEST) -p gix

build-worktree:
	cargo build --release -p agt-worktree
