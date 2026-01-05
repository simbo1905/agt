DIST = dist

.PHONY: build build-agt build-worktree clean dist test docs

build: test build-worktree build-agt dist

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

test:
	cargo test

docs:
	mkdir -p .tmp
	cd bin && uv sync
	npm list -g @mermaid-js/mermaid-cli >/dev/null 2>&1 || npm install -g @mermaid-js/mermaid-cli
	cd bin && uv run ./md2pdf ../DESIGN_20260104.md --pdf ../.tmp/DESIGN_20260104.pdf --svg-dir ../.tmp
	cd bin && uv run ./md2pdf ../DESIGN_20260105.md --pdf ../.tmp/DESIGN_20260105.pdf --svg-dir ../.tmp
	@echo "PDFs and SVGs generated in .tmp/"
