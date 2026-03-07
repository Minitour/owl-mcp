#!/usr/bin/env node
/**
 * Entry point for `npx owl-mcp` and `owl-mcp` CLI invocations.
 *
 * Resolves the platform-specific native binary from optionalDependencies,
 * falling back to a locally built binary if available, then exec-replaces
 * the current process with it.
 */

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

// Map Node.js platform/arch to the npm package suffix
const PLATFORM_PACKAGES: Record<string, string> = {
  "linux-x64": "@owl-mcp/owl-mcp-linux-x64",
  "linux-arm64": "@owl-mcp/owl-mcp-linux-arm64",
  "darwin-x64": "@owl-mcp/owl-mcp-darwin-x64",
  "darwin-arm64": "@owl-mcp/owl-mcp-darwin-arm64",
  "win32-x64": "@owl-mcp/owl-mcp-win32-x64",
};

const BINARY_NAME = process.platform === "win32" ? "owl-mcp.exe" : "owl-mcp";
const platformKey = `${process.platform}-${process.arch}`;
const packageName = PLATFORM_PACKAGES[platformKey];

function tryResolveBinary(): string | null {
  // 1. Try the platform-specific optional package
  if (packageName) {
    try {
      const pkg = require(packageName) as { binaryPath?: string } | undefined;
      if (pkg?.binaryPath) {
        return pkg.binaryPath;
      }
      const pkgDir = path.dirname(require.resolve(`${packageName}/package.json`));
      const candidate = path.join(pkgDir, "bin", BINARY_NAME);
      if (fs.existsSync(candidate)) return candidate;
    } catch {
      // optional package not installed
    }
  }

  // 2. Try a locally built binary (for development / cargo build)
  const localCandidates = [
    path.join(__dirname, "..", "..", "target", "release", BINARY_NAME),
    path.join(__dirname, "..", "..", "target", "debug", BINARY_NAME),
  ];
  for (const candidate of localCandidates) {
    if (fs.existsSync(candidate)) return candidate;
  }

  return null;
}

const binaryPath = tryResolveBinary();

if (!binaryPath) {
  console.error(
    `owl-mcp: No pre-built binary found for platform '${platformKey}'.\n` +
      "Please build from source: cargo build --release\n" +
      "Or install a supported platform package via npm."
  );
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});

if (result.error) {
  console.error("Failed to start owl-mcp:", result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 0);
