// Builds the AI server and copies it where the installer picks it up. The file name must end in the
// target name, which is how the bundler finds the right one.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const run = (command, args) => execFileSync(command, args, { cwd: root, encoding: "utf8" });

const target = run("rustc", ["-vV"]).match(/^host: (.+)$/m)[1].trim();
const suffix = target.includes("windows") ? ".exe" : "";
execFileSync("cargo", ["build", "--release", "--locked", "-p", "kh-mcp"], { cwd: root, stdio: "inherit" });
const folder = join(root, "crates", "kh-app", "binaries");
mkdirSync(folder, { recursive: true });
copyFileSync(join(root, "target", "release", `kola-hub-mcp${suffix}`), join(folder, `kola-hub-mcp-${target}${suffix}`));
console.log(`Prepared the AI server for ${target}`);
