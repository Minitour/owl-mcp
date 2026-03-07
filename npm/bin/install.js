#!/usr/bin/env node
/**
 * Postinstall script: verifies that a binary was resolved successfully.
 * If not (e.g. unsupported platform), it prints a warning but does not fail
 * the install.
 */

const fs = require("fs");
const path = require("path");

const PLATFORM_PACKAGES = {
  "linux-x64": "@owl-mcp/owl-mcp-linux-x64",
  "linux-arm64": "@owl-mcp/owl-mcp-linux-arm64",
  "darwin-x64": "@owl-mcp/owl-mcp-darwin-x64",
  "darwin-arm64": "@owl-mcp/owl-mcp-darwin-arm64",
  "win32-x64": "@owl-mcp/owl-mcp-win32-x64",
};

const BINARY_NAME = process.platform === "win32" ? "owl-mcp.exe" : "owl-mcp";
const platformKey = `${process.platform}-${process.arch}`;
const packageName = PLATFORM_PACKAGES[platformKey];

if (!packageName) {
  console.warn(
    `owl-mcp: Platform '${platformKey}' is not officially supported.\n` +
      "You can build from source: cargo build --release"
  );
  process.exit(0);
}

try {
  const pkgDir = path.dirname(require.resolve(`${packageName}/package.json`));
  const candidate = path.join(pkgDir, "bin", BINARY_NAME);
  if (fs.existsSync(candidate)) {
    console.log(`owl-mcp: Binary resolved at ${candidate}`);
  } else {
    console.warn(`owl-mcp: Optional package found but binary missing at ${candidate}`);
  }
} catch {
  console.warn(
    `owl-mcp: Platform package '${packageName}' was not installed (optional).\n` +
      "You may need to install it manually, or build from source."
  );
}
