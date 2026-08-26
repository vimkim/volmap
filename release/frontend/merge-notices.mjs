#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const cargoPath = resolve(process.argv[2] ?? "");
const frontendPath = resolve(process.argv[3] ?? "");
const outputPath = resolve(process.argv[4] ?? "");
if (!process.argv[2] || !process.argv[3] || !process.argv[4]) {
  throw new Error("usage: merge-notices.mjs CARGO_NOTICES FRONTEND_NOTICES OUTPUT_NOTICES");
}

const cargo = readFileSync(cargoPath, "utf8").trimEnd();
const frontend = readFileSync(frontendPath, "utf8").trim();
writeFileSync(outputPath, `${cargo}\n\n${frontend}\n`);
