## Suite 12 - Autocommit sandbox add/rm/mv (manual)

### Purpose

Document a **manual test procedure** for `agt autocommit` to ensure it can detect:

- Newly added files
- Removed files
- Renamed / moved files

both **inside** and **outside** the session sandbox, using:

- File creation and modification times to find new/modified files
- Comparison of the shadow branch tree to on-disk files to find removed/moved files

This suite is intentionally **manual-only**. It describes the expected behavior and test steps but does not assume the implementation is complete yet.

---

### Preconditions

1. You have a working `agt` binary built from this repo.
2. You can run `agt` commands in a terminal.
3. You understand how sessions, sandboxes, and shadow branches work at a high level:
   - `sessions/<id>/sandbox/` is the jailed working tree.
   - `agtsessions/<id>` is the shadow branch where `agt autocommit` writes commits.

---

### Test 1: Added files inside sandbox are autocommitted

**Goal:** Verify that newly created files in the sandbox are detected and appear in the shadow branch after `agt autocommit`.

1. Create a fresh session:
   - `agt session new --profile=<some-profile> --id=test-autocommit-1` (or equivalent command that produces a session folder with a sandbox).
2. Change into the session sandbox working directory:
   - `cd sessions/test-autocommit-1/sandbox`
3. Create one or more new files entirely inside the sandbox:
   - `echo "inside 1" > src/inside_added_1.txt`
   - `mkdir -p src/subdir`
   - `echo "inside 2" > src/subdir/inside_added_2.txt`
4. Run autocommit from the session context:
   - `agt autocommit --session-id=test-autocommit-1`
5. Inspect the shadow branch:
   - Use `git log agtsessions/test-autocommit-1` and `git ls-tree -r HEAD` (on that branch) to check contents.
6. **Expected result:**
   - Both `src/inside_added_1.txt` and `src/subdir/inside_added_2.txt` are present in the latest shadow commit tree.
   - The autocommit did **not** require any `git add`.

---

### Test 2: Modified files inside sandbox are autocommitted

**Goal:** Verify that modifications to existing files in the sandbox are detected via mtime and captured in the shadow commit.

1. Start from the same session as Test 1 or create a fresh one with at least one file already present in the sandbox and shadow tree.
2. In `sessions/<id>/sandbox`, modify existing files:
   - `echo "updated" >> src/inside_added_1.txt`
   - `touch src/subdir/inside_added_2.txt`
3. Run:
   - `agt autocommit --session-id=<id>`
4. Inspect the latest shadow commit for that session.
5. **Expected result:**
   - The modified files are present in the new commit with updated contents / blob IDs.
   - Unmodified files are not spuriously rewritten (the implementation should use timestamps to minimize unnecessary changes).

---

### Test 3: Deleted files inside sandbox are detected via shadow tree comparison

**Goal:** Verify that removing files from the sandbox is reflected as deletions in the shadow branch by comparing the previous shadow tree to the current on-disk state.

1. Ensure the session already has a shadow commit containing one or more files (e.g., from Tests 1 and 2).
2. In `sessions/<id>/sandbox`, delete an existing file:
   - `rm src/inside_added_1.txt`
3. Optionally, delete an entire directory:
   - `rm -rf src/subdir`
4. Run:
   - `agt autocommit --session-id=<id>`
5. Inspect the latest shadow commit tree for that session.
6. **Expected result:**
   - The deleted files and directories are **absent** from the new shadow commit tree.
   - Deletions are inferred by comparing the prior shadow tree to the current filesystem; no reliance on `git status` or `git add`.

---

### Test 4: Renamed / moved files inside sandbox are detected

**Goal:** Verify that moving or renaming a file inside the sandbox is reflected in the shadow branch as a path change, ideally without losing the blob identity.

1. Ensure the session has a shadow commit containing at least one existing file, e.g. `src/file_for_rename.txt`.
2. In `sessions/<id>/sandbox`, rename/move files:
   - `git mv src/file_for_rename.txt src/renamed_inside.txt` **or** use a plain `mv`:
     - `mv src/file_for_rename.txt src/renamed_inside.txt`
3. Run:
   - `agt autocommit --session-id=<id>`
4. Inspect the latest shadow commit for that session.
5. **Expected result:**
   - The old path (`src/file_for_rename.txt`) disappears from the tree.
   - The new path (`src/renamed_inside.txt`) appears in the tree.
   - The content is preserved (same blob, unless the underlying implementation chooses to treat this as delete+add).

---

### Test 5: Added/removed/moved files **outside** the sandbox but inside the repo root

**Goal:** Verify that autocommit can also detect changes that happen in the underlying repository outside the sandbox (e.g., host-side edits) and reflect them in the shadow branch for the session.

1. Identify the original working tree (the repo root that was cloned / managed by `agt`), separate from `sessions/<id>/sandbox`.
2. With a session already created and at least one autocommit performed:
   - On the host repo root (outside `sessions/<id>/sandbox`), perform changes:
     - Add a new file: `echo "host added" > host_added.txt`
     - Remove an existing tracked file: `rm path/to/host_tracked_file.txt`
     - Move an existing file: `mv path/to/host_mv_source.txt path/to/host_mv_target.txt`
3. Run autocommit for the session:
   - `agt autocommit --session-id=<id>`
4. Inspect the new shadow commit for that session.
5. **Expected result:**
   - The shadow tree reflects the net result of the repository as visible from the session’s perspective, including:
     - New files that should be visible in the sandbox (depending on how the sandbox and host tree are synchronized).
     - Removed files that no longer exist in the repo.
     - Renamed/moved files at the appropriate paths.
   - Detection is based on filesystem timestamps and comparison with the previous shadow tree, not on manual `git add`.

---

### Test 6: `.gitignore` is **not** consulted by autocommit

**Goal:** Verify that `agt autocommit` does **not** rely on `.gitignore` and still picks up all new/modified files.

1. In the repo root (not inside the session sandbox), create or modify a `.gitignore` file that would normally ignore some paths:
   - For example:
     - `echo "ignored_dir/" >> .gitignore`
2. In `sessions/<id>/sandbox`, create files inside a directory that would be ignored by git:
   - `mkdir -p ignored_dir`
   - `echo "should be picked up by autocommit" > ignored_dir/ignored_by_git_but_not_autocommit.txt`
3. Run:
   - `agt autocommit --session-id=<id>`
4. Inspect the latest shadow commit tree.
5. **Expected result:**
   - The new file under `ignored_dir/` **is present** in the shadow commit tree.
   - The presence or contents of `.gitignore` do not change what `agt autocommit` scans.

---

### Notes for Implementers

- These tests describe the **desired behavior** in terms of:
  - Using creation and modification times to discover new/changed files.
  - Scanning the shadow branch tree and comparing it to on-disk files to find removes and moves.
- At the time of writing, parts of this behavior may **not yet be implemented**. The goal of this suite is to serve as documentation and a manual verification checklist once the features are built.
