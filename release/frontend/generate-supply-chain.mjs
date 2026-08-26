#!/usr/bin/env node

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../..");
const webRoot = join(repositoryRoot, "web");
const runtimeManifestPath = resolve(process.argv[2] ?? "");
const outputDirectory = resolve(process.argv[3] ?? "");

if (!process.argv[2] || !process.argv[3]) {
  throw new Error(
    "usage: generate-supply-chain.mjs RUNTIME_PACKAGE_MANIFEST OUTPUT_DIRECTORY",
  );
}

const acceptedLicenses = new Set([
  "Apache-2.0",
  "BSD-3-Clause",
  "ISC",
  "MIT",
  "MPL-2.0",
]);
const lifecycleNames = new Set(["preinstall", "install", "postinstall", "prepare"]);

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function repositoryUrl(manifest) {
  const source =
    typeof manifest.repository === "string"
      ? manifest.repository
      : manifest.repository?.url ?? manifest.homepage;
  if (!source) throw new Error(`${manifest.name}@${manifest.version} has no source URL`);
  return source.replace(/^git\+/, "").replace(/\.git$/, "");
}

function packageRoots() {
  const virtualStore = join(webRoot, "node_modules/.pnpm");
  const roots = [];
  for (const virtualPackage of readdirSync(virtualStore).sort()) {
    const nestedModules = join(virtualStore, virtualPackage, "node_modules");
    if (!existsSync(nestedModules)) continue;
    for (const entry of readdirSync(nestedModules).sort()) {
      if (entry.startsWith("@")) {
        const scope = join(nestedModules, entry);
        for (const child of readdirSync(scope).sort()) roots.push(join(scope, child));
      } else {
        roots.push(join(nestedModules, entry));
      }
    }
  }
  return roots;
}

function installedPackages() {
  const packages = new Map();
  for (const root of packageRoots()) {
    const manifestPath = join(root, "package.json");
    if (!existsSync(manifestPath)) continue;
    const manifest = readJson(manifestPath);
    if (!manifest.name || !manifest.version || !manifest.license) continue;
    const identity = `${manifest.name}@${manifest.version}`;
    const current = packages.get(identity);
    const canonicalRoot = realpathSync(root);
    if (!current || canonicalRoot < current.root) {
      packages.set(identity, { root: canonicalRoot, manifest });
    }
  }
  return packages;
}

function purl(name, version) {
  if (name.startsWith("@")) {
    const [scope, packageName] = name.slice(1).split("/", 2);
    if (!scope || !packageName) throw new Error(`invalid scoped package name ${name}`);
    return `pkg:npm/%40${encodeURIComponent(scope)}/${encodeURIComponent(packageName)}@${version}`;
  }
  return `pkg:npm/${encodeURIComponent(name)}@${version}`;
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function lifecycleScripts(manifest) {
  return Object.fromEntries(
    Object.entries(manifest.scripts ?? {})
      .filter(([name]) => lifecycleNames.has(name))
      .sort(([left], [right]) => left.localeCompare(right)),
  );
}

function licenseFiles(root) {
  return readdirSync(root)
    .filter((name) => /^(license|copying|notice)(\..*)?$/i.test(name))
    .sort()
    .map((name) => ({ name, text: readFileSync(join(root, name), "utf8").trimEnd() }));
}

const rootManifest = readJson(join(webRoot, "package.json"));
const toolchainPin = readJson(join(webRoot, "toolchain.json"));
const runtimeManifest = readJson(runtimeManifestPath);
if (runtimeManifest.schema !== "volmap.frontend-runtime-packages/v1") {
  throw new Error(`unsupported runtime package manifest ${runtimeManifest.schema}`);
}

const installed = installedPackages();
const runtimeIdentities = new Set(
  runtimeManifest.packages.map((item) => `${item.name}@${item.version}`),
);
const directRuntime = new Set(Object.keys(rootManifest.dependencies ?? {}));
const directBuild = new Set(Object.keys(rootManifest.devDependencies ?? {}));

for (const name of directRuntime) {
  if (!runtimeManifest.packages.some((item) => item.name === name)) {
    throw new Error(`declared runtime package ${name} was not found in the production bundle`);
  }
}

const provenancePackages = [...installed.values()]
  .map(({ manifest }) => {
    if (!acceptedLicenses.has(manifest.license)) {
      throw new Error(
        `frontend package ${manifest.name}@${manifest.version} has unreviewed license ${manifest.license}`,
      );
    }
    const identity = `${manifest.name}@${manifest.version}`;
    const role = runtimeIdentities.has(identity)
      ? "bundled-runtime"
      : directBuild.has(manifest.name)
        ? "direct-build"
        : directRuntime.has(manifest.name)
          ? "direct-runtime-unbundled"
          : "transitive-build";
    return {
      name: manifest.name,
      version: manifest.version,
      role,
      license: manifest.license,
      source: repositoryUrl(manifest),
      lifecycle_scripts: lifecycleScripts(manifest),
      lifecycle_scripts_executed: false,
    };
  })
  .sort((left, right) =>
    left.name.localeCompare(right.name) || left.version.localeCompare(right.version),
  );

const runtimePackages = runtimeManifest.packages.map((item) => {
  const identity = `${item.name}@${item.version}`;
  const installedPackage = installed.get(identity);
  if (!installedPackage) throw new Error(`bundled package ${identity} is not installed`);
  if (installedPackage.manifest.license !== item.license) {
    throw new Error(`bundled package ${identity} changed license metadata`);
  }
  const licenses = licenseFiles(installedPackage.root);
  if (licenses.length === 0) throw new Error(`bundled package ${identity} has no license text`);
  return { ...item, identity, installedPackage, licenses };
});

const notices = [
  "Volmap live-viewer runtime notices",
  "",
  "Only packages whose code is present in the committed production bundle are listed here.",
  "Build-only packages are recorded in BUILD_PROVENANCE.json.",
  "",
];
for (const item of runtimePackages) {
  notices.push(`${item.name} ${item.version}`, `License: ${item.license}`, `Source: ${item.repository}`);
  for (const license of item.licenses) {
    notices.push(`File: ${basename(license.name)}`, "", license.text);
  }
  notices.push("", "-------------------------------------------------------------------------------", "");
}

const rootPurl = "pkg:npm/volmap-live-viewer@0.0.0";
const components = runtimePackages.map((item) => ({
  type: "library",
  "bom-ref": purl(item.name, item.version),
  name: item.name,
  version: item.version,
  licenses: [{ license: { id: item.license } }],
  purl: purl(item.name, item.version),
  externalReferences: [{ type: "vcs", url: item.repository }],
}));
const runtimeByName = new Map(runtimePackages.map((item) => [item.name, item]));
const dependencies = [
  {
    ref: rootPurl,
    dependsOn: runtimePackages.map((item) => purl(item.name, item.version)).sort(),
  },
  ...runtimePackages.map((item) => ({
    ref: purl(item.name, item.version),
    dependsOn: item.dependencies
      .map((name) => runtimeByName.get(name))
      .filter((dependency) => dependency !== undefined)
      .map((dependency) => purl(dependency.name, dependency.version))
      .sort(),
  })),
];
const sbom = {
  bomFormat: "CycloneDX",
  specVersion: "1.5",
  version: 1,
  metadata: {
    tools: [{ vendor: "Volmap", name: "frontend-supply-chain", version: "1" }],
    component: {
      type: "application",
      "bom-ref": rootPurl,
      name: "volmap-live-viewer",
      version: "0.0.0",
      purl: rootPurl,
    },
  },
  components,
  dependencies,
};
const provenance = {
  schema: "volmap.frontend-build-provenance/v1",
  target: "linux-x64",
  toolchain: {
    node: rootManifest.engines.node.replace(/^=/, ""),
    package_manager: rootManifest.packageManager,
    typescript: rootManifest.devDependencies.typescript,
    vite: rootManifest.devDependencies.vite,
    vitest: rootManifest.devDependencies.vitest,
    playwright: rootManifest.devDependencies["@playwright/test"],
    browsers: toolchainPin.browsers,
  },
  install_policy: {
    registry: "https://registry.npmjs.org/",
    frozen_lockfile: true,
    lifecycle_scripts: "ignored",
    strict_peer_dependencies: true,
  },
  advisory_policy: {
    command: "pnpm audit --audit-level high",
    graph: "all installed runtime and build packages",
    source: "https://registry.npmjs.org/-/npm/v1/security/advisories/bulk",
    required_result: "no high or critical advisories",
    result_is_live_gate_not_committed_snapshot: true,
  },
  lockfile: {
    path: "web/pnpm-lock.yaml",
    sha256: sha256(join(webRoot, "pnpm-lock.yaml")),
  },
  packages: provenancePackages,
};

mkdirSync(outputDirectory, { recursive: true });
writeFileSync(join(outputDirectory, "THIRD_PARTY_NOTICES.txt"), `${notices.join("\n").trimEnd()}\n`);
writeFileSync(join(outputDirectory, "SBOM.cdx.json"), `${JSON.stringify(sbom, null, 2)}\n`);
writeFileSync(
  join(outputDirectory, "BUILD_PROVENANCE.json"),
  `${JSON.stringify(provenance, null, 2)}\n`,
);
