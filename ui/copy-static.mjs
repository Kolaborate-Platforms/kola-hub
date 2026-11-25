// A script because `cp` does not exist in a Windows shell.
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

process.chdir(fileURLToPath(new URL(".", import.meta.url)));

const isPreview = process.argv.includes("--preview");

for (const name of ["index.html", "theme.css", "styles.css"]) {
  copyFileSync(`src/${name}`, `dist/${name}`);
}

const fontSourceDir = "../node_modules/@fontsource/poppins";
const fontOutDir = "dist/fonts";
mkdirSync(fontOutDir, { recursive: true });

for (const fontFile of [
  "poppins-latin-400-normal.woff2",
  "poppins-latin-600-normal.woff2",
]) {
  copyFileSync(`${fontSourceDir}/files/${fontFile}`, `${fontOutDir}/${fontFile}`);
}
copyFileSync(`${fontSourceDir}/LICENSE`, `${fontOutDir}/LICENSE`);

const stringsPath = "../crates/kh-settings/strings/en.json";
if (isPreview) {
  if (existsSync(stringsPath)) {
    copyFileSync(stringsPath, "dist/en.json");
  }
} else {
  rmSync("dist/mock.js", { force: true });
  rmSync("dist/en.json", { force: true });
}
