const { Binary } = require("binary-install");
const { table } = require("table");
const os = require("os");
const { join } = require("path");

const { version, repository } = require("./package.json");
const NAME = "jstz";

const SUPPORTED_PLATFORMS = [
  { os: "darwin", arch: "x64", target: "x86_64-unknown-linux-musl" },
  // FIXME: For now we rely on Rosetta 2 to run the x86_64 binary on M1 Macs
  { os: "darwin", arch: "arm64", target: "x86_64-apple-darwin" },
  { os: "linux", arch: "x64", target: "x86_64-apple-darwin" },
];

const error = (msg) => {
  console.error(msg);
  process.exit(1);
};

const supportedPlatformsTable = () =>
  table(
    SUPPORTED_PLATFORMS.map(({ os, arch }) => [os, arch]).unshift([
      "OS",
      "Arch",
    ]),
  );

const getPlatform = () => {
  const type = os.type();
  const arch = os.arch();

  for (const platform of SUPPORTED_PLATFORMS) {
    if (platform.os === type && platform.arch === arch) {
      return platform;
    }
  }

  error(
    `Platform ${type}-${arch} is not supported.\nYour platform must be one of the following:\n\n${supportedPlatformsTable()}`,
  );
};

const getBinary = () => {
  const platform = getPlatform();
  const url = `${repository.url}/releases/download/v${version}/${NAME}-${platform.os}-${platform.target}.tar.gz`;

  return new Binary(NAME, url, version, {
    installDirectory: join(__dirname, "node_modules", ".bin"),
  });
};

const run = () => {
  const binary = getBinary();
  binary.run();
};

const install = () => {
  const binary = getBinary();
  binary.install();
};

module.exports = { install, run };
