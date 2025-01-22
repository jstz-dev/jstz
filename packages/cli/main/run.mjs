#!/usr/bin/env node

import { promises as fs } from "fs";
import path from "path";
import childProcess from "child_process";
import { exit } from "process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));

const BINARY_NAME = "jstz";
const BINARY_DISTRIBUTION_PACKAGE = {
  darwin_arm64: "cli-darwin-arm64",
  linux_arm64: "cli-linux-arm64",
  linux_x64: "cli-linux-x64",
};
const PLATFORM_ARCH_KEY = `${process.platform}_${process.arch}`;

function getBinaryPath() {
  try {
    // Resolving will fail if the optionalDependency was not installed
    return require.resolve(
      `${BINARY_DISTRIBUTION_PACKAGE[PLATFORM_ARCH_KEY]}/bin/${BINARY_NAME}`,
    );
  } catch {
    return path.join(__dirname, BINARY_NAME);
  }
}

try {
  await fs.access(getBinaryPath());
} catch (e) {
  console.error(`Fail to load jstz: ${e}`);
  exit(1);
}

try {
  childProcess.execFileSync(getBinaryPath(), process.argv.slice(2), {
    stdio: "inherit",
  });
} catch (e) {
  if (e.code) {
    exit(1);
  } else {
    exit(e.status);
  }
}
