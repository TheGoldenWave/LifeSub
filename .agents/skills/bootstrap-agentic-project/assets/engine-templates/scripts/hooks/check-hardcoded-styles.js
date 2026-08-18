#!/usr/bin/env node

/**
 * PreToolUse Hook: Block hardcoded style values in source files
 *
 * Enforces the project convention: all colors, spacing, and typography
 * must reference design tokens from docs/design/tokens/base.json.
 * Hardcoded hex colors (#xxx), rgb()/rgba()/hsl()/hsla() literals are blocked.
 *
 * Exit 2 = block the Edit (with error message on stderr)
 * Exit 0 = allow
 *
 * Wired in settings.json:
 *   hooks.PreToolUse, matcher: "Edit"
 */

const fs = require('fs');

// --- Configuration ---

// File extensions to check
const STYLE_EXTENSIONS = /\.(css|scss|less|vue|svelte|tsx|jsx)$/;

// Files/paths to always skip (token definitions, tests, configs)
const SKIP_PATTERNS = [
  /design\/tokens\//,           // Token definition files themselves
  /\.test\.[jt]sx?$/,           // Test files
  /\.spec\.[jt]sx?$/,           // Spec files
  /\.stories\.[jt]sx?$/,        // Storybook stories
  /node_modules\//,             // Dependencies
  /\.config\.[jt]s$/,           // Config files (tailwind.config, etc.)
  /\.d\.ts$/,                   // Type declarations
];

// --- Patterns ---

// Hex color: #RGB, #RRGGBB, #RRGGBBAA (3, 4, 6, or 8 hex digits)
// Negative lookbehind avoids matching anchor refs like href="#section"
const HEX_COLOR = /#(?:[0-9a-fA-F]{3}){1,2}(?:[0-9a-fA-F]{2})?\b/g;

// rgb()/rgba()/hsl()/hsla() function calls
const COLOR_FUNC = /(?:rgba?|hsla?)\s*\(\s*\d+/g;

// Lines that are comments or contain known safe patterns
const SAFE_LINE_PATTERNS = [
  /^\s*\/\//,                   // Single-line comment
  /^\s*\*/,                     // Block comment continuation
  /^\s*\/\*/,                   // Block comment start
  /^\s*<!--/,                   // HTML comment
  /^\s*\{\/\*/,                 // JSX comment
  /eslint-disable/,             // ESLint override
  /stylelint-disable/,          // Stylelint override
  /noinspection/,               // JetBrains suppression
  /prettier-ignore/,            // Prettier override
  /token-ok/,                   // Explicit opt-out annotation
  /token/i,                     // Token-related lines (definitions, imports)
  /--[\w-]+-color/,             // CSS custom property definitions (token vars)
  /var\(--/,                    // CSS var() usage (using tokens correctly)
];

// Safe hex patterns that aren't colors
const SAFE_HEX_PATTERNS = [
  /#region\b/,                  // Code folding
  /#endregion\b/,               // Code folding
  /#pragma\b/,                  // Compiler directives
  /#if\b/,                      // Preprocessor
  /#else\b/,                    // Preprocessor
  /#endif\b/,                   // Preprocessor
  /#include\b/,                 // C includes
  /href\s*=\s*["']#/,          // Anchor links
  /url\s*\(\s*["']?#/,         // SVG URL references
  /id\s*=\s*["']#/,            // ID references
];

// --- Main ---

let input;
try {
  input = JSON.parse(fs.readFileSync('/dev/stdin', 'utf8'));
} catch (_) {
  process.exit(0);
}

const filePath = input?.tool_input?.file_path || '';
const newString = input?.tool_input?.new_string || '';

// Only check style-relevant file types
if (!STYLE_EXTENSIONS.test(filePath)) {
  process.exit(0);
}

// Skip token files, tests, configs
if (SKIP_PATTERNS.some(p => p.test(filePath))) {
  process.exit(0);
}

// Scan each line for violations
const lines = newString.split('\n');
const violations = [];

for (let i = 0; i < lines.length; i++) {
  const line = lines[i];

  // Skip safe lines (comments, token usage, etc.)
  if (SAFE_LINE_PATTERNS.some(p => p.test(line))) {
    continue;
  }

  // Skip lines with safe hex patterns
  if (SAFE_HEX_PATTERNS.some(p => p.test(line))) {
    continue;
  }

  // Check for hex colors
  const hexMatches = line.match(HEX_COLOR);
  if (hexMatches) {
    // Filter out very short matches that might be false positives
    const realColors = hexMatches.filter(m => {
      const hex = m.slice(1).toLowerCase();
      // Skip common non-color hex patterns
      if (['000', 'fff'].includes(hex)) return false; // Allow #000 and #fff (too common in resets)
      return true;
    });
    if (realColors.length > 0) {
      violations.push({
        lineNum: i + 1,
        line: line.trim(),
        matches: realColors,
        type: 'hex-color',
      });
    }
  }

  // Check for rgb()/rgba()/hsl()/hsla()
  const funcMatches = line.match(COLOR_FUNC);
  if (funcMatches) {
    violations.push({
      lineNum: i + 1,
      line: line.trim(),
      matches: funcMatches.map(m => m + '...)'),
      type: 'color-function',
    });
  }
}

if (violations.length > 0) {
  const details = violations
    .map(v => `  Line ${v.lineNum}: ${v.line}\n    → 检测到: ${v.matches.join(', ')}`)
    .join('\n');

  console.error(
    `[Guardrail] 🎨 检测到硬编码样式值，团队规范要求使用 Design Token！\n` +
    `文件: ${filePath}\n` +
    `位置:\n${details}\n\n` +
    `📋 修复方式：\n` +
    `  1. 查阅 docs/design/tokens/base.json 找到对应 Token\n` +
    `  2. 使用 CSS 变量 var(--token-name) 或 JS Token 常量替代硬编码值\n` +
    `  3. 如需新增 Token，请先更新 base.json 再引用\n` +
    `  4. 如确实需要硬编码（如 SVG 内联样式），在该行添加注释 // token-ok 或 /* token-ok */`
  );
  process.exit(2);
}

process.exit(0);
