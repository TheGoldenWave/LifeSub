# LifeSub Documentation Realignment Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize LifeSub documentation around the approved Evidence Quality product boundary, produce the formal V0.1 PRD, and align LifeSub contracts with Malow and GoldenWave without changing application code.

**Architecture:** LifeSub becomes the authority for long-duration audio, ASR revisions, speaker evidence, vocabulary, and Evidence access. Malow remains the Project/Matter interpretation and review layer; GoldenWave remains the governed memory and context layer. Documentation is split into product-initiated context, technical context, formal PRD artifacts, and repository entry points.

**Tech Stack:** Markdown, Mermaid, PlantUML, JSON Schema/OpenAPI terminology, repository-provided PRD templates, shell-based documentation validation.

**Primary specification:** `docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md`

---

## File Map

### Product context

- Create `docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md`: product positioning, user problem, value, scope, success gates, reference-project conclusions.
- Create `docs/context/product-initiated/lifesub-evidence-v0.1-202608/research.md`: OpenWhispr and TypeWhisper capability comparison and clean-room constraints.
- Create `docs/context/product-initiated/lifesub-evidence-v0.1-202608/source-disposition.md`: section-level migration, supersession, and archival decision for every flat source document.
- Remove `docs/product-brief.md`, `docs/research.md`, and `docs/roadmap.md` after their approved content is incorporated.

### Technical context

- Create `docs/context/technical/lifesub-evidence-v0.1/architecture.md`: Tauri/React, Rust Core, native capture adapters, storage, reliability, cross-platform boundaries.
- Create `docs/context/technical/lifesub-evidence-v0.1/decisions.md`: confirmed decisions and explicitly unresolved choices.
- Create `docs/context/technical/lifesub-evidence-v0.1/integrations.md`: Evidence Contract, Malow consumer flow, GoldenWave provenance chain, Agent/API surface.
- Create `docs/context/technical/lifesub-evidence-v0.1/privacy-and-sync.md`: recording consent, cloud Provider authorization, voiceprint treatment, retention, encrypted object sync.
- Create `docs/context/technical/lifesub-evidence-v0.1/quality-gates.md`: ASR, correction, vocabulary, timing, speaker, and soak-test metrics.
- Remove the corresponding flat files under `docs/` after migration.

### Formal requirement

- Create `docs/prd/0.1.0-lifesub-evidence-202608/PRD.md`: formal V0.1 requirement.
- Create `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/process.md`: session restore and milestone state.
- Create `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/notes.md`: scoped product/technical risks and decisions.
- Create `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/PRD_dual-pane.html`: generated dual-pane preview.
- Create `docs/prd/0.1.0-lifesub-evidence-202608/预览PRD-macOS.command` and `预览PRD-Windows.bat`: preview launchers.
- Remove `docs/prd/.demo-feature/` after the formal requirement exists.

### Repository entry points

- Modify `README.md`: Evidence Quality positioning, three-project boundary, current V0.1, and new document links.
- Modify `docs/context/INDEX.md` only after presenting `/reflect` candidates and receiving explicit confirmation.
- Modify `docs/context/log.md`: append the documentation operation after execution.

---

## Chunk 1: Product Truth And Source Migration

### Task 0: Record the execution baseline

**Files:**
- Runtime record only: `.git/lifesub-doc-realignment-base.sha`

- [ ] **Step 1: Record the starting commit**

Run:

```bash
git rev-parse HEAD > .git/lifesub-doc-realignment-base.sha
cat .git/lifesub-doc-realignment-base.sha
```

Expected: one full commit SHA. This file remains inside `.git/` and is never committed.

- [ ] **Step 2: Record the initial dirty-worktree paths**

Run:

```bash
git status --short
```

Expected: existing user/scaffolding changes are recorded before execution. Do not stage or remove them unless an exact file is listed in this plan.

### Task 1: Create the product-initiated context

**Files:**
- Create: `docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md`
- Create: `docs/context/product-initiated/lifesub-evidence-v0.1-202608/research.md`
- Create: `docs/context/product-initiated/lifesub-evidence-v0.1-202608/source-disposition.md`
- Source: `docs/product-brief.md`
- Source: `docs/research.md`
- Source: `docs/roadmap.md`
- Reference: `docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md`

- [ ] **Step 1: Read the approved product sources and specification**

Run:

```bash
sed -n '1,260p' docs/product-brief.md
sed -n '1,240p' docs/research.md
sed -n '1,240p' docs/roadmap.md
sed -n '1,520p' docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md
```

Expected: the old documents describe a memory system, while the approved specification defines an Evidence Quality system.

- [ ] **Step 2: Write the product brief**

Include:

- Product statement: long-duration personal audio and ASR evidence platform.
- Core question: “what was said, by whom, and when?”
- LifeSub/Malow/GoldenWave responsibility table.
- V0.1 user journey and first-value moment.
- Explicit non-goals: memory compression, Project interpretation, Knowledge governance.
- Success gates copied from the approved specification without weakening thresholds.

- [ ] **Step 3: Write the reference-project research note**

Include a capability matrix with columns:

```text
Capability | OpenWhispr | TypeWhisper | LifeSub decision | Target version
```

Record TypeWhisper GPLv3 as clean-room architectural research only. Record OpenWhispr MIT separately; do not claim that any source code has been copied.

- [ ] **Step 4: Write the section-level source disposition**

For every heading in the seven flat source documents, add one row:

```text
Source section | Disposition | Target | Reason
```

The disposition must be exactly one of:

```text
migrate | superseded-by-approved-spec | retain-as-historical-reference
```

For `retain-as-historical-reference`, copy the complete original section into a real archive document under:

```text
docs/context/product-initiated/lifesub-evidence-v0.1-202608/archive/<source-name>.md
```

The disposition target must point to that archive file and heading. Do not treat the disposition row itself as preserved content.

Explicitly account for:

- Market, wearable, and hardware research from `docs/research.md`.
- Codex plugin bundle and DeepSeek Harness adapter notes from `docs/integrations.md`.
- China/GDPR/US recording-law reminders from `docs/privacy-and-sync.md`.
- Every confirmed decision and every unresolved choice from `docs/decisions.md`.

No flat source document may be removed until all of its headings have a disposition row, every `migrate` target exists, and every `retain-as-historical-reference` target contains the complete archived section. A source file may remain in place if archiving it would create less clarity than retaining it.

- [ ] **Step 5: Verify old memory positioning is absent**

Run:

```bash
rg -n "个人 AI 记忆系统|Memory Core|结构化记忆|search_memories|get_memory" \
  docs/context/product-initiated/lifesub-evidence-v0.1-202608
```

Expected: no matches except quoted “not included” explanations.

- [ ] **Step 6: Commit the product context**

```bash
git add \
  docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md \
  docs/context/product-initiated/lifesub-evidence-v0.1-202608/research.md \
  docs/context/product-initiated/lifesub-evidence-v0.1-202608/source-disposition.md
test ! -d docs/context/product-initiated/lifesub-evidence-v0.1-202608/archive || \
  git add docs/context/product-initiated/lifesub-evidence-v0.1-202608/archive
git commit -m "docs: define LifeSub evidence product boundary"
```

### Task 2: Freeze the version roadmap

**Files:**
- Modify: `docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md`
- Source: `docs/roadmap.md`

- [ ] **Step 1: Add the approved version sequence**

Use these milestones:

```text
V0.1 reliable evidence loop
V0.2 quality and management enhancements
V0.3 speaker evidence
V0.4 recording automation
V0.5 desktop platforms and ecosystem
V0.6 encrypted multi-device sync
V1.0 sustainable daily recording
V2+ mobile and wearable capture
```

- [ ] **Step 2: Mark downstream capabilities as integration gates**

State that Malow Organizer/Review and GoldenWave Governance are not LifeSub features. LifeSub versions may require compatibility tests with them, but cannot absorb their domain objects.

- [ ] **Step 3: Validate each future capability has one owner**

Run:

```bash
rg -n "Organizer|Knowledge Patch|Context Pack|Profile|Persona|Project|Matter" \
  docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md
```

Expected: each occurrence describes Malow or GoldenWave ownership, or an explicit LifeSub non-goal.

- [ ] **Step 4: Commit the roadmap update**

```bash
git add docs/context/product-initiated/lifesub-evidence-v0.1-202608/product-brief.md
git commit -m "docs: sequence LifeSub evidence roadmap"
```

## Chunk 2: Technical Truth And Cross-Repository Contracts

### Task 3: Rewrite the technical architecture

**Files:**
- Create: `docs/context/technical/lifesub-evidence-v0.1/architecture.md`
- Create: `docs/context/technical/lifesub-evidence-v0.1/decisions.md`
- Source: `docs/architecture.md`
- Source: `docs/decisions.md`
- Reference: `docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md`

- [ ] **Step 1: Write the system architecture**

Document these modules:

```text
Tauri + React desktop shell
Platform Capture Adapter
Rust lifesub-core
lifesubd desktop service
SQLite Evidence Catalog
Audio object store
Markdown and FTS5 derived projections
Evidence API and MCP adapter
```

Include a Mermaid component diagram and a PlantUML sequence for capture, atomic chunk finalization, ASR, correction revision, Markdown rendering, and Malow evidence resolution.

- [ ] **Step 2: Document the capture contract**

Specify monotonic frame time, UTC anchor, sample positions, drift correction, and `Discontinuity` events. Keep macOS ScreenCaptureKit/AVAudioEngine, Windows WASAPI, and Linux PipeWire behind adapters.

- [ ] **Step 3: Write the decision register**

Confirmed decisions must include:

- Evidence-only product boundary.
- Tauri/React desktop and Rust Core.
- Native capture adapters.
- SQLite and object files as authority; Markdown/FTS as derived.
- Original ASR plus immutable revision chain.
- Evidence Contract ownership.
- GitHub not used for continuous audio sync.

Unresolved decisions must include:

- Default local ASR after Phase 0 benchmark.
- Audio encoding and chunk duration after durability tests.
- Correction Provider and model.
- Diarization and speaker embedding model.
- Encrypted sync backend.
- Open-source license.

- [ ] **Step 4: Check for platform lock-in**

Run:

```bash
rg -n "SwiftData|CoreData|UserDefaults|AppKit|Foundation URL" \
  docs/context/technical/lifesub-evidence-v0.1
```

Expected: platform types appear only inside the macOS adapter discussion, never in persisted contracts.

- [ ] **Step 5: Commit the architecture documents**

```bash
git add docs/context/technical/lifesub-evidence-v0.1/architecture.md \
  docs/context/technical/lifesub-evidence-v0.1/decisions.md
git commit -m "docs: define cross-platform evidence architecture"
```

### Task 4: Define integrations, privacy, and quality gates

**Files:**
- Create: `docs/context/technical/lifesub-evidence-v0.1/integrations.md`
- Create: `docs/context/technical/lifesub-evidence-v0.1/privacy-and-sync.md`
- Create: `docs/context/technical/lifesub-evidence-v0.1/quality-gates.md`
- Source: `docs/integrations.md`
- Source: `docs/privacy-and-sync.md`
- Cross-check: `/Users/goldenwave/Documents/MyProject/goldenwave/docs/lifesub-malow-integration.md`
- Cross-check: `/Users/goldenwave/Documents/MyProject/malow/docs/integrations/lifesub-goldenwave-collaboration.zh-CN.md`

- [ ] **Step 1: Write the Evidence Contract surface**

Document:

```text
list_records
search_transcripts
get_transcript_segment
resolve_evidence
request_audio_excerpt
get_evidence_status
```

Include stable `lifesub://` URIs, revisions, hashes, access grants, tombstones, errors, pagination, idempotency, and unknown-major fail-closed behavior.

- [ ] **Step 2: Write the three-system flow**

Use:

```text
LifeSub Evidence -> Malow Organizer/Review -> GoldenWave Inbox/Governance
```

State that Malow stores references and necessary snapshots, not complete audio or the LifeSub database. State that GoldenWave provenance retains both Malow and LifeSub references.

- [ ] **Step 3: Write privacy and sync rules**

Cover:

- Always-visible recording and privacy pause.
- Separate cloud ASR, cloud correction, and cloud voiceprint authorization.
- Voiceprint consent, encryption, retention, unlinking, and deletion.
- Encrypted object synchronization instead of Git for audio.
- Markdown as explicit export, not canonical sync.
- Evidence revocation propagation.

- [ ] **Step 4: Write executable quality gates**

Copy the complete approved `16.1 V0.1 Gate` and its supporting benchmark thresholds from the primary specification. This includes recording durability, startup reconciliation, queue backpressure, disk protection, ASR/correction metrics, dual-source drift, `Discontinuity`, independent cloud authorizations, Markdown reconstruction, Malow Evidence consumption, GoldenWave provenance, revocation propagation, and domain-object exclusion. Separate the V0.3 speaker gates into their own section.

- [ ] **Step 5: Validate cross-repository terminology**

Run:

```bash
rg -n "LifeSub|Evidence Contract|Knowledge Patch|GoldenWave Inbox|Organizer" \
  docs/context/technical/lifesub-evidence-v0.1 \
  /Users/goldenwave/Documents/MyProject/goldenwave/docs/lifesub-malow-integration.md \
  /Users/goldenwave/Documents/MyProject/malow/docs/integrations/lifesub-goldenwave-collaboration.zh-CN.md
```

Expected: authority direction remains LifeSub -> Malow -> GoldenWave; no document instructs a consumer to read another repository's database.

- [ ] **Step 6: Commit the contract and quality documents**

```bash
git add docs/context/technical/lifesub-evidence-v0.1/integrations.md \
  docs/context/technical/lifesub-evidence-v0.1/privacy-and-sync.md \
  docs/context/technical/lifesub-evidence-v0.1/quality-gates.md
git commit -m "docs: specify evidence contracts and quality gates"
```

## Chunk 3: Formal V0.1 Requirement

### Task 5: Produce the V0.1 PRD and session artifacts

**Files:**
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/PRD.md`
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/process.md`
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/notes.md`
- Reference: `.claude/templates/PRD.md`
- Reference: `.claude/templates/PRD-writing-guide.md`
- Reference: `.claude/templates/process_template.md`

- [ ] **Step 1: Write the PRD header and goal table**

Set:

```yaml
feature_id: 0.1.0-lifesub-evidence-202608
source: product-initiated
stage: prd
```

The core success moment is: a user records a long session, receives a trustworthy revisioned transcript, and Malow resolves an authorized Evidence Ref back to the exact text and audio range.

- [ ] **Step 2: Add the User Journey Map**

Cover onboarding and permissions, start/pause/resume, long-running status, safe stop/finalization, ASR and correction status, transcript review, Markdown export, and Malow evidence consumption.

- [ ] **Step 3: Add process and sequence diagrams**

Use Mermaid for the user flow and PlantUML for:

```text
macOS Capture Adapter -> Rust Core -> Object Store -> Job Queue -> ASR -> Correction -> Evidence Catalog -> Malow
```

- [ ] **Step 4: Specify V0.1 functional modules**

Each function uses the required six-part PRD structure:

1. Long-duration dual-source recording.
2. Atomic physical chunks and crash recovery.
3. Local ASR and revision management.
4. Basic Vocabulary and constrained LLM correction.
5. Logical transcript segmentation and Markdown projection.
6. FTS5 transcript search and evidence resolution.
7. Storage protection, diagnostics, access grants, and audit.
8. Malow minimum consumer integration.

- [ ] **Step 5: Add boundaries and acceptance criteria**

Copy the complete V0.1 gates from `quality-gates.md`, not only the model-quality subset. State that diarization, SpeakerProfile, calendar automation, Windows/Linux, encrypted sync, memory compression, and GoldenWave governance are not V0.1 features.

- [ ] **Step 6: Write process.md and notes.md**

Set `stage: prd-approved` only if the PRD content has passed review. Before review, use `stage: prd-draft`.

Record:

- Approved Evidence-only boundary.
- Reference-project research completed.
- Open choices requiring Phase 0 benchmarks.
- Next milestone: technical spike and acceptance-test design.

- [ ] **Step 7: Validate PRD structure**

Run:

```bash
rg -n "^## [0-9]+\.|User Journey|mermaid|plantuml|验收标准|功能边界|非功能" \
  docs/prd/0.1.0-lifesub-evidence-202608/PRD.md
```

Expected: all required PRD sections and both diagram formats are present.

- [ ] **Step 8: Run PRD review and request approval**

Dispatch a PRD/spec reviewer against the approved Evidence Platform specification. Fix blocking findings and repeat until approved. Then present the written PRD and review result to the user and pause for explicit approval.

Expected: reviewer approval plus an explicit user confirmation before `stage: prd-approved` is used.

- [ ] **Step 9: Commit the formal requirement**

```bash
git add docs/prd/0.1.0-lifesub-evidence-202608/PRD.md \
  docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/process.md \
  docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/notes.md
git commit -m "docs: add LifeSub V0.1 evidence PRD"
```

### Task 6: Generate the dual-pane preview

**Files:**
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/PRD_dual-pane.html`
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/预览PRD-macOS.command`
- Create: `docs/prd/0.1.0-lifesub-evidence-202608/预览PRD-Windows.bat`
- Reference: `docs/prd/.demo-feature/`

- [ ] **Step 1: Reuse the project preview templates**

Copy the template structure, replace demo paths and titles, and keep generated HTML in `.artifacts/`.

- [ ] **Step 2: Verify launcher paths**

Run:

```bash
rg -n "demo-feature|用户登录|process.txt" \
  docs/prd/0.1.0-lifesub-evidence-202608
```

Expected: no matches.

- [ ] **Step 3: Open the HTML locally and verify both panes**

Expected: the left pane renders `PRD.md`; the right pane renders `.artifacts/process.md`; local links resolve from the formal PRD directory.

- [ ] **Step 4: Commit the preview artifacts**

```bash
git add docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/PRD_dual-pane.html \
  docs/prd/0.1.0-lifesub-evidence-202608/预览PRD-macOS.command \
  docs/prd/0.1.0-lifesub-evidence-202608/预览PRD-Windows.bat
git commit -m "docs: add LifeSub PRD preview"
```

## Chunk 4: Repository Cleanup And Validation

### Task 7: Switch repository entry points and remove superseded files

**Files:**
- Modify: `README.md`
- Remove: `docs/product-brief.md`
- Remove: `docs/research.md`
- Remove: `docs/roadmap.md`
- Remove: `docs/architecture.md`
- Remove: `docs/decisions.md`
- Remove: `docs/integrations.md`
- Remove: `docs/privacy-and-sync.md`
- Remove: `docs/prd/.demo-feature/`

- [ ] **Step 1: Update README positioning**

Use the approved statement and link to:

- Formal V0.1 PRD.
- Product brief and reference research.
- Architecture, decisions, integrations, privacy/sync, and quality gates.
- Current process file.
- Malow and GoldenWave repositories.

- [ ] **Step 2: Remove superseded flat documents and demo PRD**

Only remove a source file after every heading is present in `source-disposition.md`, each `migrate` target exists, and the migration can be checked from the final diff.

- [ ] **Step 3: Check for stale links and terminology**

Run:

```bash
rg -n "docs/(product-brief|research|roadmap|architecture|decisions|integrations|privacy-and-sync)\.md|\.demo-feature" \
  README.md docs
```

Expected: no stale links except historical references inside committed design/plan documents.

Run:

```bash
rg -n "search_memories|get_memory|Memory Core|结构化记忆|GitHub 记忆库" \
  README.md docs \
  -g '!docs/superpowers/specs/2026-08-15-lifesub-evidence-platform-design.md' \
  -g '!docs/superpowers/plans/2026-08-15-lifesub-documentation-realignment.md'
```

Expected: no active-document matches unless explicitly marked superseded or excluded.

- [ ] **Step 4: Commit repository cleanup**

```bash
git add README.md \
  docs/product-brief.md \
  docs/research.md \
  docs/roadmap.md \
  docs/architecture.md \
  docs/decisions.md \
  docs/integrations.md \
  docs/privacy-and-sync.md \
  docs/prd/.demo-feature
git commit -m "docs: make evidence architecture the repository baseline"
```

Do not stage `docs/context/INDEX.md`, `docs/design/`, unrelated PRD directories, or the entire `docs/` tree.

### Task 8: Validate documentation and prepare knowledge-index candidates

**Files:**
- Modify: `docs/context/log.md`
- Conditional modify after confirmation: `docs/context/INDEX.md`
- Modify: `docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/process.md`

- [ ] **Step 1: Check Markdown links**

Run this fixed read-only checker against all active Markdown documents:

```bash
node <<'NODE'
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const files = execFileSync('/usr/bin/find', ['docs', '-type', 'f', '-name', '*.md'], { encoding: 'utf8' })
  .trim().split('\n').filter(Boolean).concat(['README.md']);
const missing = [];
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const raw = match[1].trim();
    if (!raw || /^(https?:|mailto:|#)/.test(raw)) continue;
    const target = raw.split('#')[0].replace(/^<|>$/g, '');
    if (!target) continue;
    const resolved = path.resolve(path.dirname(file), decodeURIComponent(target));
    if (!fs.existsSync(resolved)) missing.push(`${file} -> ${raw}`);
  }
}
if (missing.length) {
  process.stderr.write(missing.join('\n') + '\n');
  process.exit(1);
}
process.stdout.write(`Checked ${files.length} Markdown files: OK\n`);
NODE
```

Expected: zero missing local targets.

- [ ] **Step 2: Run repository consistency checks**

Run:

```bash
git diff --check
git status --short
rg -n "console\.log" . -g '*.ts' -g '*.tsx' -g '*.js' -g '*.jsx'
```

Expected: no whitespace errors; no application code changed; no newly introduced `console.log`.

Verify the complete commit range is documentation-only:

```bash
BASE_SHA=$(cat .git/lifesub-doc-realignment-base.sha)
if git diff --name-only "$BASE_SHA"..HEAD | rg -v '^(README\.md|docs/)'; then
  echo "Non-document path changed during documentation realignment" >&2
  exit 1
fi
```

Expected: no output and exit code 0.

- [ ] **Step 3: Present `/reflect` candidates**

Scan:

```text
docs/context/product-initiated/lifesub-evidence-v0.1-202608/
docs/context/technical/lifesub-evidence-v0.1/
docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/notes.md
```

Present candidate INDEX rows to the user. Do not modify `docs/context/INDEX.md` until explicit confirmation is received.

If the user confirms entries, append only the approved rows and commit them separately:

```bash
git add docs/context/INDEX.md
git commit -m "docs: index LifeSub evidence architecture knowledge"
```

If the user does not confirm, leave `docs/context/INDEX.md` unchanged and report the pending candidates.

- [ ] **Step 4: Append the operation log**

Add a dated `reflect` or documentation-realignment entry to `docs/context/log.md`, following its existing format.

- [ ] **Step 5: Mark the process milestone**

Only after the explicit PRD approval in Task 5, update `.artifacts/process.md` with:

```yaml
stage: prd-approved
last_updated: 2026-08-15
```

Record documentation migration, PRD approval, remaining Phase 0 choices, and the next technical spike.

- [ ] **Step 6: Commit final documentation state**

```bash
git add docs/context/log.md \
  docs/prd/0.1.0-lifesub-evidence-202608/.artifacts/process.md
git commit -m "docs: finalize LifeSub evidence documentation baseline"
```

If the user has not approved the PRD, keep `stage: prd-draft`, do not execute this commit, and report the pending approval.

- [ ] **Step 7: Report completion evidence**

Report:

- New document paths.
- Removed superseded paths.
- Link-check result.
- Stale-term scan result.
- PRD preview verification.
- INDEX candidates awaiting or receiving confirmation.
- Exact commits created during execution.
