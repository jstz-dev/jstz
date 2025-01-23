#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const https = require("https");

const BINARY_NAME = "jstz";
const FALLBACK_BINARY_PATH = path.join(__dirname, BINARY_NAME);
const BINARY_DISTRIBUTION = {
  darwin_arm64: "jstz_darwin_arm64",
  linux_x64: "jstz_linux_x64",
  linux_arm64: "jstz_linux_arm64",
};
const BINARY_DISTRIBUTION_PACKAGE = {
  darwin_arm64: "cli-darwin-arm64",
  linux_arm64: "cli-linux-arm64",
  linux_x64: "cli-linux-x64",
};
const PLATFORM_ARCH_KEY = `${process.platform}_${process.arch}`;

function makeRequest(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (response) => {
        if (response.statusCode >= 200 && response.statusCode < 300) {
          const chunks = [];
          response.on("data", (chunk) => chunks.push(chunk));
          response.on("end", () => {
            resolve(Buffer.concat(chunks));
          });
        } else if (
          response.statusCode >= 300 &&
          response.statusCode < 400 &&
          response.headers.location
        ) {
          // Follow redirects
          makeRequest(response.headers.location).then(resolve, reject);
        } else {
          reject(
            new Error(
              `npm responded with status code ${response.statusCode} when downloading the package!`,
            ),
          );
        }
      })
      .on("error", (error) => {
        reject(error);
      });
  });
}

async function downloadBinaryFromGithubRelease(binName) {
  const downloadBuffer = await makeRequest(
    `https://github.com/jstz-dev/jstz/releases/download/${process.env.npm_package_version}/${binName}`,
  );

  fs.writeFileSync(FALLBACK_BINARY_PATH, downloadBuffer);

  fs.chmodSync(FALLBACK_BINARY_PATH, "755");
}

function isPlatformSpecificPackageInstalled() {
  try {
    // Resolving will fail if the optionalDependency was not installed
    require.resolve(
      `${BINARY_DISTRIBUTION_PACKAGE[PLATFORM_ARCH_KEY]}/bin/${BINARY_NAME}`,
    );
    return true;
  } catch {
    return false;
  }
}

if (!isPlatformSpecificPackageInstalled()) {
  let binName = BINARY_DISTRIBUTION[PLATFORM_ARCH_KEY];
  if (binName === undefined) {
    throw new Error(`Unsupported platform ${PLATFORM_ARCH_KEY}`);
  }
  downloadBinaryFromGithubRelease(binName);
}
