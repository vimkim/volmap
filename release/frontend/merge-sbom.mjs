#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const cargoPath = resolve(process.argv[2] ?? "");
const frontendPath = resolve(process.argv[3] ?? "");
const outputPath = resolve(process.argv[4] ?? "");
if (!process.argv[2] || !process.argv[3] || !process.argv[4]) {
  throw new Error("usage: merge-sbom.mjs CARGO_SBOM FRONTEND_SBOM OUTPUT_SBOM");
}

const cargo = JSON.parse(readFileSync(cargoPath, "utf8"));
const frontend = JSON.parse(readFileSync(frontendPath, "utf8"));
if (cargo.bomFormat !== "CycloneDX" || frontend.bomFormat !== "CycloneDX") {
  throw new Error("both inputs must be CycloneDX documents");
}
if (cargo.specVersion !== frontend.specVersion) {
  throw new Error("Cargo and frontend SBOM spec versions differ");
}

const frontendRoot = frontend.metadata?.component;
const cargoRoot = cargo.metadata?.component;
if (!frontendRoot?.["bom-ref"] || !cargoRoot?.["bom-ref"]) {
  throw new Error("both SBOMs must identify their root component");
}

cargo.metadata.tools = [
  ...(cargo.metadata.tools ?? []),
  { vendor: "Volmap", name: "frontend-sbom-merge", version: "1" },
];

cargoRoot.components = [...(cargoRoot.components ?? []), frontendRoot].sort((left, right) =>
  left["bom-ref"].localeCompare(right["bom-ref"]),
);
cargo.components = [...(cargo.components ?? []), ...(frontend.components ?? [])];

const dependencies = new Map(
  (cargo.dependencies ?? []).map((dependency) => [dependency.ref, dependency]),
);
const cargoRootDependency = dependencies.get(cargoRoot["bom-ref"]);
if (!cargoRootDependency) throw new Error("Cargo SBOM has no root dependency entry");
cargoRootDependency.dependsOn = [
  ...new Set([...(cargoRootDependency.dependsOn ?? []), frontendRoot["bom-ref"]]),
].sort();
for (const dependency of frontend.dependencies ?? []) {
  if (dependencies.has(dependency.ref)) {
    throw new Error(`duplicate SBOM dependency reference ${dependency.ref}`);
  }
  dependencies.set(dependency.ref, dependency);
}
cargo.dependencies = [...dependencies.values()];

writeFileSync(outputPath, `${JSON.stringify(cargo, null, 2)}\n`);
