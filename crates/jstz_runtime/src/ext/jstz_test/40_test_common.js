// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";

const { StringPrototypeReplaceAll } = primordials;

const ESCAPE_ASCII_CHARS = [
  ["\b", "\\b"],
  ["\f", "\\f"],
  ["\t", "\\t"],
  ["\n", "\\n"],
  ["\r", "\\r"],
  ["\v", "\\v"],
];

/**
 * @param {string} name
 * @returns {string}
 */
export function escapeName(name) {
  // Check if we need to escape a character
  for (let i = 0; i < name.length; i++) {
    const ch = name.charCodeAt(i);
    if (ch <= 13 && ch >= 8) {
      // Slow path: We do need to escape it
      for (const [escape, replaceWith] of ESCAPE_ASCII_CHARS) {
        name = StringPrototypeReplaceAll(name, escape, replaceWith);
      }
      return name;
    }
  }

  // We didn't need to escape anything, return original string
  return name;
}
