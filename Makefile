DIST = dist
HOOKS_DIR = .githooks

.PHONY: build build-agt build-worktree clean dist fmt lint test test-snapshot check install-hooks docs

build: check build-worktree build-agt dist

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
	rm -rf .tmp

fmt:
	cargo fmt --all --check

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace --all-targets --all-features -- --nocapture

test-snapshot:
	cargo test test_snapshot_ -- --nocapture

check: fmt lint test-snapshot test

install-hooks:
	mkdir -p .git/hooks
	cp $(HOOKS_DIR)/pre-commit .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit

docs:
	mkdir -p .tmp
	cd bin && uv sync
	npm list -g @mermaid-js/mermaid-cli >/dev/null 2>&1 || npm install -g @mermaid-js/mermaid-cli
	cd bin && for design in ../DESIGN_*.md; do name=$$(basename "$$design" .md); uv run ./md2pdf "$$design" --pdf "../.tmp/$$name.pdf" --svg-dir ../.tmp; done
	@echo "PDFs and SVGs generated in .tmp/"
