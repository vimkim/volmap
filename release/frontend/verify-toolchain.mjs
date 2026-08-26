#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const expected = JSON.parse(readFileSync(resolve(process.argv[2] ?? ""), "utf8"));
const manifest = JSON.parse(readFileSync(join(repositoryRoot, "web/package.json"), "utf8"));

if (expected.schema !== "volmap.frontend-toolchain/v1") {
  throw new Error(`unsupported frontend toolchain schema ${expected.schema}`);
}
if (process.version !== `v${expected.node}` || manifest.engines.node !== `=${expected.node}`) {
  throw new Error(`Node must be exactly ${expected.node}; found ${process.version}`);
}
if (!manifest.packageManager.startsWith(`pnpm@${expected.pnpm}+sha512-`)) {
  throw new Error(`packageManager must pin pnpm ${expected.pnpm} with integrity`);
}
if (manifest.devDependencies["@playwright/test"] !== expected.playwright) {
  throw new Error("Playwright package and browser pin use different versions");
}

const browserMetadata = JSON.parse(
  readFileSync(
    join(
      repositoryRoot,
      `web/node_modules/.pnpm/playwright-core@${expected.playwright}/node_modules/playwright-core/browsers.json`,
    ),
    "utf8",
  ),
);
for (const name of ["chromium", "firefox"]) {
  const actual = browserMetadata.browsers.find((browser) => browser.name === name);
  const pinned = expected.browsers[name];
  if (
    !actual ||
    actual.revision !== pinned.revision ||
    actual.browserVersion !== pinned.version ||
    !actual.installByDefault
  ) {
    throw new Error(`${name} does not match the reviewed Playwright browser pin`);
  }
}
