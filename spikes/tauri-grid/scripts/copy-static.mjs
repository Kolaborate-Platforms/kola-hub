// A script because `cp` does not exist in a Windows shell.
import { copyFileSync } from "node:fs";

for (const name of ["index.html", "styles.css"]) {
  copyFileSync(`src/${name}`, `dist/${name}`);
}
