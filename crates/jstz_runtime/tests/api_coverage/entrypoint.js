import baseline from "ext:api_coverage_test/baseline.js";
import { visit } from "ext:api_coverage_test/utils.js";

async function runTest() {
  // This test function is basically taken from
  // https://raw.githubusercontent.com/jstz-dev/nodejs-compat-matrix/426ca553141d5ac41764beb9078bd27efd980756/deno/dump.js
  const globals = {};
  const importedModules = {};
  for (const name of Object.keys(baseline)) {
    if (name === "*globals*") {
      for (const globalProp of Object.keys(baseline["*globals*"])) {
        if (globalProp in globalThis) {
          globals[globalProp] = globalThis[globalProp];
        }
      }
      continue;
    }

    try {
      const module = await import(`node:${name}`);
      importedModules[name] = module;
    } catch {
      continue;
    }
  }

  const result = visit(baseline, {
    "*globals*": globals,
    ...importedModules,
  });
  return JSON.stringify(result);
}

Object.defineProperty(globalThis, "runTest", {
  value: runTest,
  enumerable: true,
  configurable: true,
  writable: true,
});
