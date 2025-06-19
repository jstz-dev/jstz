#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BINARY_NAME = "jstz";

const PLATFORM_TO_PACKAGE = new Map([
  ["darwin_arm64", "@jstz-dev/cli-darwin-arm64"],
  ["linux_arm64", "@jstz-dev/cli-linux-arm64"],
  ["linux_x64", "@jstz-dev/cli-linux-x64"],
]);

function getBinaryPath() {
  const platformKey = `${process.platform}_${process.arch}`;
  const packageName = PLATFORM_TO_PACKAGE.get(platformKey);

  try {
    const packageJsonUrl = import.meta.resolveSync(
      `${packageName}/package.json`,
    );
    const packageJsonPath = fileURLToPath(packageJsonUrl);
    const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf-8"));
    const relativeBinPath = packageJson.bin?.[BINARY_NAME];

    if (!relativeBinPath) {
      throw new Error(`"bin" field missing in ${packageName}/package.json`);
    }

    const packageDir = path.dirname(packageJsonPath);
    return path.join(packageDir, relativeBinPath);
  } catch (e) {
    return path.join(__dirname, "bin", BINARY_NAME);
  }
}

try {
  const binaryPath = getBinaryPath();
  const args = process.argv.slice(2);
  execFileSync(binaryPath, args, { stdio: "inherit" });
} catch (error) {
  process.exit(error.status ?? 1);
}