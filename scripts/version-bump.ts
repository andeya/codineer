#!/usr/bin/env bun
/**
 * Unified version bump for both package.json and Cargo workspace.
 *
 * Usage:
 *   bun run version:bump <patch|minor|major|x.y.z>
 */
import { readFileSync, writeFileSync } from "node:fs";

const arg = process.argv[2];
if (!arg) {
  console.error("Usage: bun run version:bump <patch|minor|major|x.y.z>");
  process.exit(1);
}

const pkgPath = "package.json";
const cargoPath = "Cargo.toml";
const tauriPath = "app/tauri.conf.json";

// Read current version from package.json
const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
const current = pkg.version as string;
const [major, minor, patch] = current.split(".").map(Number);

let next: string;
if (arg === "patch") next = `${major}.${minor}.${patch + 1}`;
else if (arg === "minor") next = `${major}.${minor + 1}.0`;
else if (arg === "major") next = `${major + 1}.0.0`;
else if (/^\d+\.\d+\.\d+$/.test(arg)) next = arg;
else {
  console.error(`Invalid version argument: ${arg}`);
  process.exit(1);
}

// 1. package.json
pkg.version = next;
writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

// 2. Cargo.toml workspace version
const cargo = readFileSync(cargoPath, "utf-8");
writeFileSync(cargoPath, cargo.replace(/^version = ".*"$/m, `version = "${next}"`));

// 3. tauri.conf.json
const tauri = JSON.parse(readFileSync(tauriPath, "utf-8"));
tauri.version = next;
writeFileSync(tauriPath, `${JSON.stringify(tauri, null, 2)}\n`);

console.log(`${current} → ${next}  (package.json, Cargo.toml, tauri.conf.json)`);
