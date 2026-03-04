#!/usr/bin/env node
////////////////////////////////////////////////////
// NAME
//  format-json.mjs
//
// Description
//  Reformat JSON files into the repository's preferred stable inline and
//  indented layout.
//
// Synopsis
//  ./scripts/format-json.mjs path...
//
// Requirements
//  * Node.js must be installed.
//
// Portability
//  This script should work with modern Node.js.
//
// Author
//  Masaki Waga
//
// License
//  Apache 2.0 License
////////////////////////////////////////////////////

import fs from "node:fs";

function formatInline(value) {
  if (Array.isArray(value)) {
    return `[${value.map(formatInline).join(", ")}]`;
  }

  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value);
    return `{${entries.map(([key, entry]) => `${JSON.stringify(key)}: ${formatInline(entry)}`).join(", ")}}`;
  }

  return JSON.stringify(value);
}

function formatValue(value, indentLevel = 0) {
  if (Array.isArray(value)) {
    return formatInline(value);
  }

  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      return "{}";
    }

    const currentIndent = "  ".repeat(indentLevel);
    const nextIndent = "  ".repeat(indentLevel + 1);
    const body = entries
      .map(([key, entry]) => `${nextIndent}${JSON.stringify(key)}: ${formatValue(entry, indentLevel + 1)}`)
      .join(",\n");
    return `{\n${body}\n${currentIndent}}`;
  }

  return JSON.stringify(value);
}

for (const path of process.argv.slice(2)) {
  const source = fs.readFileSync(path, "utf8");
  const formatted = `${formatValue(JSON.parse(source))}\n`;
  fs.writeFileSync(path, formatted);
}
