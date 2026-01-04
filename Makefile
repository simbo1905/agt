GIX_MANIFEST = vendor/gitoxide/Cargo.toml
DIST = dist

.PHONY: build build-agt build-gix build-worktree clean dist

build: build-gix build-worktree build-agt dist

build-agt:
	cargo build --release -p agt

build-gix:
	cargo build --release --manifest-path $(GIX_MANIFEST) -p gitoxide --bin gix

build-worktree:
	cargo build --release -p agt-worktree

dist:
	mkdir -p $(DIST)
	cp target/release/agt $(DIST)/
	cp target/release/agt-worktree $(DIST)/
	cp vendor/gitoxide/target/release/gix $(DIST)/

clean:
	cargo clean
	cargo clean --manifest-path $(GIX_MANIFEST)
	rm -rf $(DIST)
