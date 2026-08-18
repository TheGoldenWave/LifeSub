#!/usr/bin/env node

/**
 * Stop Hook — Progress Checkpoint Reminder
 *
 * Fires when Claude is about to end its response (Stop event).
 * On first invocation: exit 2 to inject a reminder and keep Claude active.
 * On second invocation (stop_hook_active: true): exit 0 to allow stop.
 *
 * V3: Detects primary .artifacts/process.md plus legacy process.txt
 * files and suggests running /reflect if notes.md files were recently modified.
 *
 * Wired in settings.json:
 *   hooks.Stop
 */

const fs = require('fs');
const path = require('path');

let input;
try {
  input = JSON.parse(fs.readFileSync('/dev/stdin', 'utf8'));
} catch (_) {
  process.exit(0);
}

// stop_hook_active = true means we already fired once this cycle.
// Let Claude stop to avoid an infinite loop.
if (input?.stop_hook_active) {
  process.exit(0);
}

const cwd = process.cwd();
const prdDir = path.join(cwd, 'docs', 'prd');

// Only prompt if the project has the prd structure set up
if (!fs.existsSync(prdDir)) {
  process.exit(0);
}

// Check if there are any process files (.artifacts/process.md or legacy process.txt) in the project
function hasProcessFiles(dir) {
  if (!fs.existsSync(dir)) return false;
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (_) {
    return false;
  }
  for (const entry of entries) {
    if (entry.isDirectory() && hasProcessFiles(path.join(dir, entry.name))) return true;
    if (entry.name === 'process.txt' || entry.name === 'process.md') return true;
  }
  return false;
}

// Check if any notes.md files were modified recently (within thresholdMs)
function hasRecentNotes(dir, thresholdMs) {
  if (!fs.existsSync(dir)) return false;
  const now = Date.now();
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch (_) {
    return false;
  }
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (hasRecentNotes(fullPath, thresholdMs)) return true;
    } else if (entry.name === 'notes.md') {
      try {
        const stat = fs.statSync(fullPath);
        if (now - stat.mtimeMs < thresholdMs) return true;
      } catch (_) { /* skip */ }
    }
  }
  return false;
}

if (!hasProcessFiles(prdDir)) {
  process.exit(0);
}

// Build checkpoint message
let message =
  '[Checkpoint] 📌 即将结束会话。在结束前，请确认以下两项：\n' +
  '1. 已将本次会话的进度（当前阶段、下一步计划）更新到对应功能目录的 .artifacts/process.md（如项目仍在迁移，也可兼容保留 process.txt）\n' +
  '2. 遇到的问题或踩坑已记录到同目录的 notes.md\n';

// Suggest /reflect if notes.md was recently modified (within 2 hours)
const TWO_HOURS_MS = 2 * 60 * 60 * 1000;
if (hasRecentNotes(prdDir, TWO_HOURS_MS)) {
  message += '\n💡 检测到 notes.md 有更新，建议运行 /reflect 将新知识同步到团队知识库索引 (docs/context/INDEX.md)。\n';
}

message += '\n如已完成，回复"已保存"即可。如尚未完成，请立即执行写入操作。';

console.error(message);
process.exit(2); // Force Claude to respond before session ends
