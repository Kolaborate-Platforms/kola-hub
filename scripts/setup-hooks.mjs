// Turns on the repository's git hooks after `pnpm install`.
import { spawnSync } from "node:child_process";

const result = spawnSync("git", ["config", "core.hooksPath", ".githooks"], { stdio: "inherit" });
if (result.status !== 0) {
  console.warn("Git hooks were not turned on. Read CONTRIBUTING.md before you commit.");
}
