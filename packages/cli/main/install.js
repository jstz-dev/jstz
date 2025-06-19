#!/usr/bin/env node

import { createWriteStream } from "fs";
import { promises as fsPromises } from "fs";
import path from "path";
import { pipeline } from "stream/promises";
import { fileURLToPath } from "url";
import packageJson from "./package.json" with { type: "json" };

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const BINARY_NAME = "jstz";
const BIN_DIR = path.join(__dirname, "bin");
const DEST_PATH = path.join(BIN_DIR, BINARY_NAME);

const PLATFORM_TO_BINARY_NAME = new Map([
  ["darwin_arm64", "jstz_darwin_arm64"],
  ["linux_x64", "jstz_linux_x64"],
  ["linux_arm64", "jstz_linux_arm64"],
]);

const PLATFORM_TO_PACKAGE = new Map([
  ["darwin_arm64", "@jstz-dev/cli-darwin-arm64"],
  ["linux_arm64", "@jstz-dev/cli-linux-arm64"],
  ["linux_x64", "@jstz-dev/cli-linux-x64"],
]);

function isOptionalDependencyInstalled(platformKey) {
  const packageName = PLATFORM_TO_PACKAGE.get(platformKey);
  if (!packageName) {
    return false;
  }
  try {
    import.meta.resolveSync(`${packageName}/package.json`);
    console.log(
      `Found optional dependency: ${packageName}. Skipping download.`,
    );
    return true;
  } catch (e) {
    return false;
  }
}

try {
  const platformKey = `${process.platform}_${process.arch}`;

  if (isOptionalDependencyInstalled(platformKey)) {
    console.log("Installation complete.");
    process.exit(0);
  }

  console.log(
    "Optional dependency not found. Attempting to download binary as a fallback.",
  );

  const binaryName = PLATFORM_TO_BINARY_NAME.get(platformKey);
  if (!binaryName) {
    throw new Error(
      `Unsupported platform: ${platformKey}. Pre-compiled jstz binaries are not available.`,
    );
  }

  await fsPromises.mkdir(BIN_DIR, { recursive: true });

  if (!packageJson.version) {
    throw new Error("The 'version' field was not found in package.json");
  }

  const url = `https://github.com/jstz-dev/jstz/releases/download/${packageJson.version}/${binaryName}`;

  console.log(`Downloading ${binaryName} from ${url}`);

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error(
      `Failed to download binary: Server responded with ${response.status} ${response.statusText}`,
    );
  }

  if (!response.body) {
    throw new Error("The download response had no body.");
  }

  const tempPath = `${DEST_PATH}.tmp`;

  try {
    await pipeline(response.body, createWriteStream(tempPath));
    await fsPromises.rename(tempPath, DEST_PATH);
    await fsPromises.chmod(DEST_PATH, 0o755);

    console.log(
      `Successfully downloaded and installed ${BINARY_NAME} to ${DEST_PATH}`,
    );
  } catch (downloadError) {
    try {
      await fsPromises.unlink(tempPath);
    } catch (cleanupError) {}
    throw downloadError;
  }
} catch (error) {
  console.error("\n--- jstz binary installation failed ---");
  console.error(error.message);
  process.exit(1);
}