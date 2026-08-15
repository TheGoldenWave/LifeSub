# LifeSub Latest Installer Path Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish the newest signed DMG for the current Mac architecture to the stable local path `output/installers/LifeSub-latest.dmg`.

**Architecture:** A focused Node.js module selects the newest matching signed Tauri DMG and replaces the stable copy through a same-directory temporary file. The same module provides a thin CLI, while dependency injection around filesystem operations makes destructive failure paths testable without touching real build artifacts.

**Tech Stack:** Node.js 22 ESM, built-in `node:test`, npm scripts, Tauri macOS DMG output

---

## Chunk 1: Tested Publisher And Local Release

### Task 1: Define Publisher Acceptance Tests

**Files:**
- Create: `tests/specs/publish-latest-installer.test.mjs`
- Create: `tests/specs/publish-latest-installer-cli.test.mjs`

- [ ] **Step 1: Write failing core publisher tests**

Create `tests/specs/publish-latest-installer.test.mjs` using `node:test`, `node:assert/strict`, temporary directories, and imported `createInstallerPublisher` / `publishLatestInstaller` exports. Cover:

```js
test('publishes the newest signed DMG for the requested architecture', async () => {
  // Create old/new aarch64 signed files plus newer unsigned and x64 files.
  // Set deterministic mtimes, publish, then assert the returned absolute paths
  // and target bytes match only the newest aarch64 signed source.
})

test('publishes a single matching artifact and removes its temporary file', async () => {
  // Publish one aarch64 signed file, assert matching bytes and absolute paths,
  // then assert the target directory contains no `.tmp-<pid>` file.
})

test('uses filename order when matching files have the same mtime', async () => {
  // Create LifeSub_0.1.0_aarch64-signed.dmg and
  // LifeSub_0.2.0_aarch64-signed.dmg with equal mtimes.
  // Assert the lexically last filename wins.
})

test('rejects unsupported architectures and missing matching artifacts', async () => {
  // Assert readable Error messages for riscv64 and for an empty source directory.
})

test('rejects a directory containing only another architecture', async () => {
  // Put only an x64 signed DMG in the source and request arm64.
  // Assert no target is created and the error identifies arm64.
})

test('preserves the old installer when source metadata cannot be read', async () => {
  // Inject a filesystem adapter whose stat throws for the matching source.
  // Assert an Error, unchanged old target bytes, and no temporary file.
})

test('preserves the old installer when target directory creation fails', async () => {
  // Inject a filesystem adapter whose mkdir throws.
  // Assert an Error, unchanged old target bytes, and no temporary file.
})

test('preserves the old installer and removes the temporary file when copy fails', async () => {
  // Inject a filesystem adapter whose copyFile creates a partial temp then throws.
  // Assert an Error, unchanged old target bytes, and no temporary file.
})

test('preserves the old installer and removes the temporary file when rename fails', async () => {
  // Inject a filesystem adapter whose rename throws after copyFile succeeds.
  // Assert the old target bytes remain and no `.tmp-<pid>` file remains.
})

test('can replace the stable installer repeatedly', async () => {
  // Publish once, update the source bytes, publish again, and assert the target
  // contains the second complete value.
})
```

- [ ] **Step 2: Write failing CLI integration tests**

Create `tests/specs/publish-latest-installer-cli.test.mjs`. Spawn the CLI with temporary paths supplied only through test-scoped environment variables `LIFESUB_INSTALLER_SOURCE_DIR`, `LIFESUB_INSTALLER_TARGET_PATH`, and `LIFESUB_INSTALLER_ARCH`; production use continues to require no arguments or environment configuration.

```js
test('CLI prints the stable path and source filename on success', async () => {
  // Assert exit 0, empty stderr, and exact stdout containing the absolute
  // temporary target path plus the exact source filename and trailing newlines.
})

test('CLI prints one diagnostic line and exits 1 on failure', async () => {
  // Point at an empty source directory. Assert exit 1, empty stdout, and exact
  // stderr: `Failed to publish latest installer: No signed arm64 installer found in <absolute-source-dir>\n`.
})
```

- [ ] **Step 3: Run tests to verify they fail**

Run:

```bash
node --test tests/specs/publish-latest-installer.test.mjs tests/specs/publish-latest-installer-cli.test.mjs
```

Expected: FAIL because `scripts/publish-latest-installer.mjs` does not exist.

- [ ] **Step 4: Commit the failing acceptance tests**

```bash
git add tests/specs/publish-latest-installer.test.mjs tests/specs/publish-latest-installer-cli.test.mjs
git commit -m "test: define latest installer publishing behavior"
```

### Task 2: Implement The Stable Installer Publisher

**Files:**
- Create: `scripts/publish-latest-installer.mjs`
- Modify: `package.json`
- Modify: `.gitignore`
- Test: `tests/specs/publish-latest-installer.test.mjs`
- Test: `tests/specs/publish-latest-installer-cli.test.mjs`

- [ ] **Step 1: Implement architecture mapping and deterministic selection**

In `scripts/publish-latest-installer.mjs`, use `node:fs/promises`, `node:path`, and `node:url`. Define immutable constants for default paths and architecture suffixes:

```js
const ARCHITECTURE_SUFFIXES = Object.freeze({
  arm64: '_aarch64-signed.dmg',
  x64: '_x64-signed.dmg',
})
```

Resolve every input path to an absolute path. Reject unsupported architectures. Read only regular files whose names end with the exact mapped suffix. Sort candidates by `mtimeMs`, then filename, and select the final entry. Throw a readable `Error` when none match.

- [ ] **Step 2: Implement protected publication and cleanup**

Export `createInstallerPublisher(fileSystem)` for injected failure tests and export its default instance as `publishLatestInstaller`:

```js
export function createInstallerPublisher(fileSystem = fs) {
  return async function publishLatestInstaller({ sourceDir, targetPath, architecture }) {
    // Resolve and select source before touching the existing target.
    // mkdir target parent, copy source to `${targetPath}.tmp-${process.pid}`,
    // rename the complete temporary file over target, and always rm temp in finally.
    // Return { sourcePath, targetPath } with absolute paths.
  }
}

export const publishLatestInstaller = createInstallerPublisher()
```

All failures propagate as `Error` objects. The target is never removed before `rename`, so selection, metadata, directory, copy, and rename failures cannot truncate an existing stable installer.

- [ ] **Step 3: Implement the thin CLI**

Detect direct execution with `pathToFileURL(process.argv[1]).href === import.meta.url`. For normal use, default to the repository Tauri source directory, fixed output path, and `process.arch`. Permit the three `LIFESUB_INSTALLER_*` variables only as test infrastructure so the CLI can be integration-tested in isolation; they are not a supported user configuration surface and are not documented outside the test code.

On success print exactly:

```text
Latest installer: <absolute-path>
Source: <filename>
```

On failure print exactly one line beginning `Failed to publish latest installer: ` to stderr and set `process.exitCode = 1`.

- [ ] **Step 4: Add the npm command and ignore generated DMGs**

Add to `package.json`:

```json
"test:installer": "node --test tests/specs/publish-latest-installer.test.mjs tests/specs/publish-latest-installer-cli.test.mjs",
"installer:latest": "node scripts/publish-latest-installer.mjs"
```

Add to `.gitignore`:

```gitignore
# Locally published installers
output/installers/*.dmg
```

- [ ] **Step 5: Run focused tests**

Run `npm run test:installer`.

Expected: all publisher and CLI tests PASS, including injected rename failure protection.

- [ ] **Step 6: Run repository regression checks**

Run:

```bash
npm test
npm run build
```

Expected: existing Vitest suite and production build PASS. Check modified JavaScript files for `console.log`; the CLI may use `process.stdout.write` and `process.stderr.write` only.

- [ ] **Step 7: Commit the implementation**

```bash
git add scripts/publish-latest-installer.mjs package.json .gitignore
git commit -m "feat: publish installer at a stable local path"
```

### Task 3: Publish And Verify The Current Signed DMG

**Files:**
- Generate locally: `output/installers/LifeSub-latest.dmg`
- Modify: `docs/prd/lifesub-v0.1/.artifacts/process.md`

- [ ] **Step 1: Publish the current signed installer**

Run `npm run installer:latest`.

Expected stdout identifies absolute target `output/installers/LifeSub-latest.dmg` and source `LifeSub_0.1.0_aarch64-signed.dmg`.

- [ ] **Step 2: Verify byte identity and readability**

Run:

```bash
shasum -a 256 src-tauri/target/release/bundle/dmg/LifeSub_0.1.0_aarch64-signed.dmg output/installers/LifeSub-latest.dmg
test -f output/installers/LifeSub-latest.dmg
test -r output/installers/LifeSub-latest.dmg
test ! -L output/installers/LifeSub-latest.dmg
```

Expected: both SHA-256 values are identical; all file checks exit `0`, proving the target is a readable regular copy rather than a symbolic link.

- [ ] **Step 3: Update session progress**

Update `docs/prd/lifesub-v0.1/.artifacts/process.md` frontmatter `last_updated` to `2026-08-16` and record the fixed local installer path, source artifact, publisher command, focused test result, and checksum verification.

- [ ] **Step 4: Commit progress evidence**

```bash
git add docs/prd/lifesub-v0.1/.artifacts/process.md
git commit -m "docs: record stable local installer path"
```

- [ ] **Step 5: Final verification**

Run:

```bash
npm run test:installer
npm test
npm run build
git check-ignore -v output/installers/LifeSub-latest.dmg
git status --short
```

Expected: the focused installer suite reports `12` passing top-level tests, the existing Vitest suite and production build pass, `git check-ignore` reports the `output/installers/*.dmg` rule from `.gitignore`, and pre-existing unrelated worktree changes remain untouched.
