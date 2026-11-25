// A script because `cp` does not exist in a Windows shell.
import { copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

process.chdir(fileURLToPath(new URL(".", import.meta.url)));

for (const name of ["index.html", "styles.css"]) {
  copyFileSync(`src/${name}`, `dist/${name}`);
}
