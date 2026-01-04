DIST = dist

.PHONY: build build-agt build-worktree clean dist test

build: build-worktree build-agt dist

build-agt:
	cargo build --release -p agt

build-worktree:
	cargo build --release -p agt-worktree

dist:
	mkdir -p $(DIST)
	cp target/release/agt $(DIST)/
	cp target/release/agt-worktree $(DIST)/

clean:
	cargo clean
	rm -rf $(DIST)

test:
	cargo test
