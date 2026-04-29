import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];

if (!version) {
  console.error("Usage: node scripts/sync-version.mjs <version>");
  process.exit(1);
}

const cargoToml = "Cargo.toml";
const packageJson = "package.json";

const cargo = readFileSync(cargoToml, "utf8").replace(
  /^version = ".*"$/m,
  `version = "${version}"`,
);
writeFileSync(cargoToml, cargo);

const pkg = JSON.parse(readFileSync(packageJson, "utf8"));
pkg.version = version;
writeFileSync(packageJson, `${JSON.stringify(pkg, null, 2)}\n`);

execFileSync("cargo", ["generate-lockfile"], { stdio: "inherit" });
execFileSync("npm", ["install", "--package-lock-only", "--ignore-scripts"], {
  shell: process.platform === "win32",
  stdio: "inherit",
});
