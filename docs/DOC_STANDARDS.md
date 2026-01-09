# RFC: Discoverable and DRY Documentation Architecture for Agents and Humans

**Status:** Proposed
**Author:** Simon Massey
**Context:** Hybrid Human/Agent Repositories

## 1. Abstract
This Request for Comments (RFC) defines a strict, normative standard for repository documentation architecture. It addresses the "documentation problem" of staleness, duplication, and high cognitive load for autonomous agents. The architecture mandates a single source of truth, explicit linking (no orphans), and a flat directory structure optimized for discoverability via `ls`, utilizing naming conventions over directory nesting.

## 2. Core Philosophy
1.  **Single Source of Truth (DRY):** Information must exist in exactly one place. All other references must be signposts (links), not copies.
2.  **Discoverability:** The architecture assumes agents discover content by listing directories (`ls`), not by recursive searching.
3.  **Self-Documentation:** Executable tools document themselves via `--help`. Static documentation must not duplicate this.
4.  **Context Efficiency:** Documentation structure must minimize the tokens an agent needs to read to understand the repository layout.
5.  **Strict Separation:** User documentation (`README.md`) and Agent instructions (`AGENTS.md`) are distinct and non-overlapping.

## 3. Repository Root Requirements

The repository root **MUST** contain the following files:

### 3.1. `README.md` (Human Entry Point)
*   **Purpose:** The canonical entry point for users.
*   **Content:**
    *   **MUST** link to the `docs/` folder (Architecture).
    *   **MUST** link to the `bin/` folder (Tools).
    *   **MUST** link to `CONVENTIONS.md`.
    *   **MUST** contain a table of contents indexing any **Architectural Submodules** or **Extensions** (e.g., `specs/`, `tests/`).
*   **Constraints:**
    *   **MUST NOT** contain agent-specific instructions.
    *   **MUST NOT** duplicate command-line argument explanations (refer to tool `--help`).
    *   **MUST NOT** describe internal architectural details (refer to `docs/`).

### 3.2. `AGENTS.md` (Agent Entry Point)
*   **Purpose:** Instructions strictly for autonomous agents regarding navigation and behavior.
*   **Content:**
    *   **MUST** instruct the agent to read `README.md` for project context.
    *   **MUST** describe the directory map (e.g., "There is a `bin/` folder...").
    *   **MUST** instruct agents to use `ls` to discover content in specific folders (`bin/`, `docs/`) and avoid recursive searches unless necessary.
    *   **MUST** outline behavioral rules (e.g., "No destructive writes," "Do not push").
*   **Constraints:**
    *   **MUST NOT** duplicate user documentation found in `README.md`.

### 3.3. `CONVENTIONS.md`
*   **Purpose:** The single source of truth for coding standards, style guides, and linting rules.
*   **Linking:**
    *   **MUST** be linked from `README.md`.
    *   **MUST** be linked from `AGENTS.md`.

### 3.4. `docs/` (Architecture & Knowledge)
*   **Purpose:** Holds all architectural, design, and technical reports.
*   **Content:**
    *   **MUST** contain at least one document describing the **Project Architecture**, **Layout**, and **Build Structure**.
*   **Format:** Preferred Markdown (`.md`) or LaTeX. PDF versions **MAY** be included but source is preferred.

### 3.5. `bin/` (Tooling)
*   **Purpose:** Contains all executable scripts and tools.

## 4. Documentation Content Standards

### 4.1. Tool Documentation
*   **Primary Source:** The tool itself. Every tool in `bin/` **MUST** provide a comprehensive `--help` or `-h` output.
*   **No Duplication:** `README.md`, `AGENTS.md`, and architectural docs **MUST NOT** explain tool arguments or usage examples. They **MUST** only signpost the tool (e.g., "Run `bin/my_tool --help`").

### 4.2. Extended Manuals (Man Pages)
*   **Location:** If a tool requires complex documentation beyond `--help`, a manual file (e.g., `toolname.1.md`) **MUST** reside in `docs/`.
*   **Signposting:** The tool's `--help` output **MUST** refer the user to the manual in `docs/` (e.g., "See `docs/toolname.1.md` for details"). It **MUST NOT** imply the help text *is* the manual.

## 5. Directory Structure & Naming

### 5.1. Naming Conventions
*   **Documentation:** Use **SHOUTING_SNAKE_CASE** (e.g., `README.md`, `DESIGN_INGESTION.md`, `AGENTS.md`) to distinguish prose from code.
*   **Tools/Scripts:** Use **snake_case** (e.g., `test_deployment.sh`, `run_server.py`).

### 5.2. Flat vs. Nested (The Prefix Rule)
*   **Rule:** Do **NOT** use subfolders to categorize files by type (e.g., do not create a `scripts/` or `misc/` folder).
*   **Solution:** Use **Prefix Naming** in top-level folders.
    *   *Example:* Test scripts go in `bin/` prefixed with `test_` (e.g., `bin/test_regression.sh`).
*   **Reasoning:** Flat structures allow agents to discover all available capabilities via a single `ls` command, reducing context switching and hidden files.

### 5.3. Architectural Submodules (The Exception)
*   **Definition:** Subfolders are reserved **ONLY** for independently deployable subsystems or architecturally significant modules (Monorepo pattern).
*   **Depth:** Nesting **SHOULD** be limited to one level deep.
*   **Requirements:**
    *   Top-level `README.md` **MUST** index these submodules.
    *   Each submodule **MUST** have a corresponding `DESIGN_<MODULE>.md` in `docs/`.
    *   **Design Doc Rule:** The design doc **MUST** point to the specific file defining the Client API. This allows an agent to understand the module by reading one doc, without scanning the module's source code.

## 6. Extensions (Specs & Manual Tests)

Extensions are "plugins" to the repo structure. When added, they **MUST** be introduced in `AGENTS.md` with instructions to `ls` the new folder.

### 6.1. `specs/` (Specifications)
*   **Purpose:** For SpecKit, Cairo, or similar generated specifications.
*   **Indexing:** Top-level `README.md` **MUST** have a table linking to the top-level folders/files within `specs/`.

### 6.2. `tests/` (Integration/Regression)
*   **Purpose:** For manual, deployment, regression, or "slow" integration test specifications.
*   **Constraint:** **MUST NOT** contain unit tests (JUnit, etc.), which belong with the code.
*   **Content:**
    *   Modular specifications prefixed by suite (e.g., `suite1_deployment.md`).
    *   **MUST** reference tools in `bin/` (e.g., `bin/test_setup_env`) by name.
    *   **MUST NOT** repeat tool usage instructions.

## 7. Agent Compatibility Protocols

### 7.1. Symlinks for Tool Specificity
*   **Problem:** Some AI tools ignore `AGENTS.md` and look for `claude.md`, `gemini.md`, or `cord.md`.
*   **Rule:** `AGENTS.md` is the authoritative file. All tool-specific names **MUST** be symlinks pointing to `AGENTS.md`.
*   **Scope:** This applies at the root and within any subfolder containing agent instructions.

## 8. Validation Checklist

A repository is compliant only if:
1.  [ ] Root contains `README.md`, `AGENTS.md`, `docs/`, `bin/`.
2.  [ ] `README.md` links to all top-level folders and `CONVENTIONS.md`.
3.  [ ] No content in `AGENTS.md` overlaps with `README.md`.
4.  [ ] `docs/` contains an architectural overview.
5.  [ ] Tools in `bin/` support `--help`.
6.  [ ] No documentation explains how to use a tool (except to say "use --help").
7.  [ ] No subfolders exist purely for categorization (prefixes used instead).
8.  [ ] Any `claude.md` or similar is a symlink to `AGENTS.md`.