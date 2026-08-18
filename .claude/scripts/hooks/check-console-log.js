#!/usr/bin/env node

/**
 * PreToolUse Hook: Block console.log in JS/TS files
 *
 * Claude Code passes the full tool call as JSON on stdin.
 * Exit 2 to block the Edit and show the error message to Claude.
 * Exit 0 to allow the Edit through.
 *
 * Wired in settings.json:
 *   hooks.PreToolUse, matcher: "Edit"
 */

const fs = require('fs');

let input;
try {
  input = JSON.parse(fs.readFileSync('/dev/stdin', 'utf8'));
} catch (_) {
  // If we can't parse stdin, don't block anything
  process.exit(0);
}

const filePath = input?.tool_input?.file_path || '';
const newString = input?.tool_input?.new_string || '';

// Only check JS/TS files
if (!/\.(ts|tsx|js|jsx)$/.test(filePath)) {
  process.exit(0);
}

// Check if the content being written contains console.log
if (/console\.log\s*\(/.test(newString)) {
  const lines = newString.split('\n');
  const matches = lines
    .map((line, i) => ({ line, num: i + 1 }))
    .filter(({ line }) => /console\.log\s*\(/.test(line))
    .map(({ line, num }) => `  Line ${num}: ${line.trim()}`)
    .join('\n');

  console.error(
    `[Guardrail] 🚫 检测到 console.log，团队规范禁止提交遗留调试日志！\n` +
    `文件: ${filePath}\n` +
    `位置:\n${matches}\n\n` +
    `请将 console.log 替换为项目规范的日志工具（如 logger.debug/info），或直接删除后重试。`
  );
  process.exit(2); // Block the Edit
}

process.exit(0);
