DIST = dist
HOOKS_DIR = .githooks

.PHONY: build build-agt build-worktree clean dist fmt lint test test-snapshot check check-windows install-hooks docs

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

check-windows:
	@if command -v rustup >/dev/null 2>&1 && rustup target list --installed | grep -qx 'x86_64-pc-windows-msvc'; then \
		cargo check --workspace --all-targets --all-features --target x86_64-pc-windows-msvc; \
	else \
		printf '%s\n' "Skipping Windows cross-check (target x86_64-pc-windows-msvc not installed)"; \
	fi

check: fmt lint test-snapshot test check-windows

install-hooks:
	mkdir -p .git/hooks
	cp $(HOOKS_DIR)/pre-commit .git/hooks/pre-commit
	chmod +x .git/hooks/pre-commit

docs:
	mkdir -p .tmp
	cd bin && uv sync
	npm list -g @mermaid-js/mermaid-cli >/dev/null 2>&1 || npm install -g @mermaid-js/mermaid-cli
	cd bin && uv run ./md2pdf ../DESIGN_20260104.md --pdf ../.tmp/DESIGN_20260104.pdf --svg-dir ../.tmp
	cd bin && uv run ./md2pdf ../DESIGN_20260105.md --pdf ../.tmp/DESIGN_20260105.pdf --svg-dir ../.tmp
	@echo "PDFs and SVGs generated in .tmp/"
