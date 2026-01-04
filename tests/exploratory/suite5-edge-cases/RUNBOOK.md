# Suite 5: Edge Cases and Special Files

## Objective

Test handling of:
- Binary files
- Symlinks
- File deletions
- File renames/moves
- .gitignore bypass
- Empty files
- Special characters in filenames
- Very long filenames
- Deep directory structures

## Working Directory

`.tmp/suite5`

## Setup

1. Build all binaries: `make build`
2. Create directory: `mkdir -p .tmp/suite5 && cd .tmp/suite5`
3. Initialize an agt-managed repo

## Reference

Read `docs/agt.1.txt`:
- "ignores .gitignore" in autocommit description
- Timestamp-based scanning

## Scenarios

### Scenario 5.1: Binary Files

Test that binary files are captured.

Steps:
1. Fork session
2. Create a binary file (e.g., random bytes, image, compiled binary)
3. Autocommit
4. Verify binary file is in commit
5. Checkout and verify binary is intact (compare checksums)

Success: Binary files captured and restored correctly

### Scenario 5.2: Symlinks

Test symbolic link handling.

Steps:
1. Create a regular file
2. Create a symlink pointing to it
3. Create a symlink pointing outside the worktree
4. Autocommit
5. Verify symlinks are captured (as symlinks, not dereferenced)
6. Test broken symlinks

Success: Symlinks preserved as symlinks

### Scenario 5.3: File Deletions

Test that deletions are captured.

Steps:
1. Create files A, B, C
2. Autocommit
3. Delete file B
4. Autocommit
5. Verify commit shows file B deleted
6. Reset to previous commit, verify B exists

Success: Deletions properly tracked

### Scenario 5.4: File Renames

Test move/rename tracking.

Steps:
1. Create file with content
2. Autocommit
3. Rename file (mv old new)
4. Autocommit
5. Git log --follow should track the rename
6. Git diff should show rename (if content unchanged)

Success: Renames tracked, history preserved with --follow

### Scenario 5.5: .gitignore Bypass

Verify autocommit ignores .gitignore.

Steps:
1. Create .gitignore with patterns
2. Create files that match .gitignore patterns
3. Regular `git status` should not show these files
4. Autocommit
5. Verify .gitignore'd files ARE in the commit

Success: Autocommit captures everything, ignoring .gitignore

### Scenario 5.6: Empty Files

Test zero-byte files.

Steps:
1. Create empty file: `touch empty.txt`
2. Autocommit
3. Verify empty file is in commit
4. Modify to add content, autocommit
5. Clear to empty again, autocommit
6. Verify all states captured

Success: Empty files handled correctly

### Scenario 5.7: Special Characters in Filenames

Test Unicode and special characters.

Steps:
1. Create file with spaces: `"file with spaces.txt"`
2. Create file with Unicode: `"文件.txt"`, `"émoji.txt"`
3. Create file with special chars: `"file[1].txt"`, `"file@#$.txt"`
4. Autocommit
5. Verify all files captured with correct names
6. Clone/checkout and verify names preserved

Success: All valid filenames handled

### Scenario 5.8: Very Long Filenames

Test filename length limits.

Steps:
1. Create file with maximum length filename (255 chars typically)
2. Autocommit
3. Verify captured correctly
4. Test slightly-too-long filename (should fail gracefully)

Success: Max length works, over-max fails gracefully

### Scenario 5.9: Deep Directory Structure

Test deeply nested paths.

Steps:
1. Create deeply nested directory: `a/b/c/d/e/f/g/h/i/j/file.txt`
2. Autocommit
3. Verify full path preserved
4. Modify file at depth, autocommit
5. Verify only that file shows in diff

Success: Deep paths work correctly

### Scenario 5.10: Hidden Files (Dotfiles)

Test hidden files are captured.

Steps:
1. Create `.hidden` file
2. Create `.config/settings.json`
3. Autocommit
4. Verify hidden files are captured
5. Verify .git directory itself is NOT captured

Success: Dotfiles captured, .git excluded

### Scenario 5.11: File Permission Changes

Test executable bit changes.

Steps:
1. Create regular file
2. Autocommit
3. Make it executable: `chmod +x file`
4. Autocommit
5. Verify permission change captured

Success: Permission changes tracked

### Scenario 5.12: Large Files

Test large file handling.

Steps:
1. Create 100MB file
2. Autocommit (may take time)
3. Verify captured correctly
4. Modify 1 byte, autocommit
5. Observe: entire file re-stored (not delta)

Success: Large files work (note performance characteristics)

### Scenario 5.13: Many Files at Once

Test bulk file creation.

Steps:
1. Create 1000 small files
2. Autocommit
3. Verify all captured
4. Modify 100 of them
5. Autocommit
6. Verify only modified 100 in new commit

Success: Timestamp scanning handles many files

### Scenario 5.14: Files Created and Deleted Before Autocommit

Test ephemeral files.

Steps:
1. Create temp file
2. Delete temp file
3. Autocommit
4. Verify temp file not in commit (doesn't exist at scan time)

Success: Only files that exist at autocommit time are captured

### Scenario 5.15: Directory Removal

Test removing directories with contents.

Steps:
1. Create directory with multiple files
2. Autocommit
3. Delete entire directory: `rm -rf dir/`
4. Autocommit
5. Verify all files in directory marked as deleted

Success: Directory deletion tracked correctly

## Success Criteria

All edge cases handled correctly:
- Files captured regardless of type or name
- .gitignore bypassed
- Deletions tracked
- Permissions tracked
- Large/many files work (note performance)

## Failure Modes

- Binary corruption
- Symlink dereferencing when shouldn't
- .gitignore'd files missed
- Unicode filenames corrupted
- Deep paths fail
- Hidden files missed
- Large files cause OOM or timeout

## Notes

Some edge cases may reveal platform-specific behavior (Windows vs Unix). Document any differences.
