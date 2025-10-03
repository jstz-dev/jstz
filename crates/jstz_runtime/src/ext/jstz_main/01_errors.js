import { core } from "ext:core/mod.js";

// Builtin v8 / JS errors
// https://github.com/denoland/deno_core/blob/0694f1763f4cecc84dd32e2052aa6d639b1b83ed/core/00_infra.js#L81
const BUILT_IN_V8_ERRORS = [
  "Error",
  "RangeError",
  "ReferenceError",
  "SyntaxError",
  "TypeError",
  "URIError",
];

/**
 * Define and register multiple custom error classes by name.
 * Ensures class names are unique in the given array and are not built-in v8 errors.
 *
 * @param {string[]} names - Array of error class names to define.
 * @returns {Record<string, typeof Error>} An object mapping error names to their constructors.
 */
export const registerErrorClasses = (names) => {
  const uniqueNames = [...new Set(names)];
  const entries = uniqueNames.map((name) => {
    if (typeof name !== "string" || name.length === 0) {
      throw new TypeError("Error class name must be a non-empty string");
    }

    if (BUILT_IN_V8_ERRORS.includes(name)) {
      throw new TypeError(`Error class name ${name} is a built-in v8 error`);
    }

    const ErrorClass = class extends Error {
      constructor(message) {
        super(message);
        this.name = name;
      }
    };
    core.registerErrorClass(name, ErrorClass);
    return [name, ErrorClass];
  });

  return Object.fromEntries(entries);
};
