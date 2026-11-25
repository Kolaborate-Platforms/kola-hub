// A script because `cp` does not exist in a Windows shell.
import { copyFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

process.chdir(fileURLToPath(new URL(".", import.meta.url)));

for (const name of ["index.html", "theme.css", "styles.css"]) {
  copyFileSync(`src/${name}`, `dist/${name}`);
}

const stringsPath = "../crates/kh-settings/strings/en.json";
if (existsSync(stringsPath)) {
  copyFileSync(stringsPath, "dist/en.json");
}
