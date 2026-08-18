#!/usr/bin/env node

/**
 * SessionStart Hook — Memory Injection (with Context Budget)
 *
 * Fires when a new Claude Code session starts (SessionStart event).
 * Scans docs/prd/ for primary .artifacts/process.md files plus legacy
 * process.txt files and injects their contents into the session context
 * so Claude can restore memory.
 *
 * V3: Budget-aware injection — limits total injected content to
 * MAX_CONTEXT_CHARS (~2000 tokens) to avoid consuming excessive
 * context window space. Files are prioritized by recency (newest first).
 * Over-budget files get frontmatter summary only.
 *
 * Output protocol (compatible with Claude Code & ECC):
 *   - context injection → stdout JSON { hookSpecificOutput: { hookEventName, additionalContext } }
 *   - diagnostic logs   → stderr (never injected into Claude's context)
 *
 * Wired in settings.json:
 *   hooks.SessionStart
 */

const fs = require('fs');
const path = require('path');

const cwd = process.cwd();
const prdDir = path.join(cwd, 'docs', 'prd');

/**
 * Context budget in characters. ~4 chars ≈ 1 token, so 8000 chars ≈ 2000 tokens.
 * Adjust this value if your project needs more or less session-start context.
 */
const MAX_CONTEXT_CHARS = 8000;

/**
 * Recursively find files matching a target name under a directory.
 */
function findFiles(dir, targetName) {
  if (!fs.existsSync(dir)) return [];

  const results = [];
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (_) {
    return results;
  }

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...findFiles(fullPath, targetName));
    } else if (entry.name === targetName) {
      results.push(fullPath);
    }
  }
  return results;
}

/**
 * Extract YAML frontmatter from markdown content (zero dependencies).
 * Returns an object of key-value pairs, or null if no frontmatter found.
 */
function extractFrontmatter(content) {
  const match = content.match(/^---\n([\s\S]*?)\n---/);
  if (!match) return null;

  const data = {};
  match[1].split('\n').forEach(line => {
    const colonIdx = line.indexOf(':');
    if (colonIdx === -1) return;
    const key = line.slice(0, colonIdx).trim();
    const value = line.slice(colonIdx + 1).trim();
    if (key && value) data[key] = value;
  });
  return Object.keys(data).length > 0 ? data : null;
}

/**
 * Build a one-line stage summary from frontmatter.
 */
function buildStageSummary(frontmatter, filePath) {
  const parts = [`stage=${frontmatter.stage}`];
  if (frontmatter.source) parts.push(`source=${frontmatter.source}`);
  if (frontmatter.owner) parts.push(`owner=${frontmatter.owner}`);
  if (frontmatter.last_updated) parts.push(`updated=${frontmatter.last_updated}`);

  const featureId = frontmatter.feature_id || path.basename(path.dirname(path.dirname(filePath)));
  return `🧠 [项目状态] ${featureId}: ${parts.join(', ')}`;
}

function main() {
  // --- Collect all process files into a unified list ---
  const allFiles = [];

  for (const name of ['process.txt', 'process.md']) {
    for (const filePath of findFiles(prdDir, name)) {
      let stat;
      try { stat = fs.statSync(filePath); } catch (_) { continue; }
      const priority = filePath.includes(`${path.sep}.artifacts${path.sep}process.md`) ? 0 : 1;
      allFiles.push({ path: filePath, type: name.endsWith('.md') ? 'md' : 'txt', mtimeMs: stat.mtimeMs, priority });
    }
  }

  if (allFiles.length === 0) {
    process.stderr.write('[SessionStart] 🆕 No process files found. Fresh session.\n');
    process.exit(0);
  }

  // Sort by mtime, newest first
  allFiles.sort((a, b) => a.priority - b.priority || b.mtimeMs - a.mtimeMs);

  // --- Single-pass budget-aware injection ---
  const stageSummaries = [];  // Always injected (outside budget)
  const sections = [];        // Full content sections (budget-tracked)
  const fullyInjected = [];   // Paths of fully injected files
  const summarizedOnly = [];  // Paths of budget-truncated files
  let usedChars = 0;

  for (const file of allFiles) {
    let content;
    try { content = fs.readFileSync(file.path, 'utf8').trim(); } catch (_) { continue; }
    if (!content) continue;

    const relPath = path.relative(cwd, file.path);

    // For .md files, always extract frontmatter stage summary (small, outside budget)
    let frontmatter = null;
    if (file.type === 'md') {
      frontmatter = extractFrontmatter(content);
      if (frontmatter && frontmatter.stage) {
        stageSummaries.push(buildStageSummary(frontmatter, file.path));
      }
    }

    // Budget check: inject full content or summary only
    const sectionText = `=== ${relPath} ===\n${content}`;

    if (usedChars + sectionText.length <= MAX_CONTEXT_CHARS) {
      sections.push(sectionText);
      usedChars += sectionText.length;
      fullyInjected.push(relPath);
    } else {
      // Over budget — inject one-line summary only
      if (frontmatter && frontmatter.stage) {
        // .md file: stage summary already in stageSummaries[], no need to duplicate
      } else if (file.type === 'txt') {
        // .txt file: try to extract a useful one-liner
        const phaseLine = content.split('\n').find(l => /current_phase|当前阶段/i.test(l));
        if (phaseLine) {
          stageSummaries.push(`📋 [摘要] ${relPath}: ${phaseLine.trim()}`);
        } else {
          stageSummaries.push(`📋 [摘要] ${relPath}: (内容已省略，需要时请 Read 该文件)`);
        }
      }
      summarizedOnly.push(relPath);
    }
  }

  if (sections.length === 0 && stageSummaries.length === 0) {
    process.exit(0);
  }

  // --- Build the injected context ---
  let additionalContext = '';

  // Stage summaries at the top for quick scanning (always present, outside budget)
  if (stageSummaries.length > 0) {
    additionalContext += stageSummaries.join('\n') + '\n\n';
  }

  additionalContext +=
    '🧠 [会话恢复] 以下是上次会话的状态记录，请在开始任务前阅读并恢复上下文：\n\n' +
    sections.join('\n\n');

  // Truncation notice
  if (summarizedOnly.length > 0) {
    additionalContext +=
      '\n\n⚠️ [上下文预算] 以下文件因体积限制仅注入摘要，如需详情请使用 Read 工具查看：\n' +
      summarizedOnly.map(p => `  - ${p}`).join('\n');
  }

  additionalContext += '\n\n[SessionStart] ✅ 上下文恢复完成。';

  // Emit context via stdout JSON
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: 'SessionStart',
        additionalContext,
      },
    })
  );

  // Diagnostic log to stderr
  process.stderr.write(
    `[SessionStart] 🧠 Injected ${fullyInjected.length} file(s) in full, ${summarizedOnly.length} summarized. ` +
    `Budget: ${usedChars}/${MAX_CONTEXT_CHARS} chars used.\n`
  );
}

try {
  main();
} catch (err) {
  // Never crash the session — just log and continue
  process.stderr.write(`[SessionStart] ⚠️ Hook error (non-fatal): ${err.message}\n`);
  process.exit(0);
}
