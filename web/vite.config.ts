import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, resolve } from "node:path";

import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

const webRoot = import.meta.dirname;
const outputDirectory = process.env.VOLMAP_FRONTEND_OUT_DIR
  ? resolve(process.env.VOLMAP_FRONTEND_OUT_DIR)
  : resolve(webRoot, "../src/web/generated");

interface PackageManifest {
  readonly name?: string;
  readonly version?: string;
  readonly license?: string;
  readonly repository?: string | { readonly url?: string };
  readonly dependencies?: Readonly<Record<string, string>>;
  readonly peerDependencies?: Readonly<Record<string, string>>;
}

interface RuntimePackage {
  readonly name: string;
  readonly version: string;
  readonly license: string;
  readonly repository: string;
  readonly dependencies: readonly string[];
}

function repositoryUrl(repository: PackageManifest["repository"]): string {
  const value = typeof repository === "string" ? repository : repository?.url;
  return value?.replace(/^git\+/, "").replace(/\.git$/, "") ?? "unknown";
}

function packageManifest(moduleId: string): PackageManifest | null {
  const sourcePath = moduleId.split("?", 1)[0] ?? moduleId;
  if (sourcePath.includes("\0") || !sourcePath.includes("node_modules") || !existsSync(sourcePath)) {
    return null;
  }
  let directory = dirname(realpathSync(sourcePath));
  while (directory !== dirname(directory)) {
    const candidate = resolve(directory, "package.json");
    if (existsSync(candidate)) {
      const manifest = JSON.parse(readFileSync(candidate, "utf8")) as PackageManifest;
      if (directory.includes("node_modules") && manifest.name && manifest.version) {
        return manifest;
      }
    }
    directory = dirname(directory);
  }
  return null;
}

function runtimePackageManifest(): Plugin {
  return {
    name: "volmap-runtime-package-manifest",
    generateBundle(_options, bundle) {
      const manifests = new Map<string, PackageManifest>();
      for (const output of Object.values(bundle)) {
        if (output.type !== "chunk") continue;
        for (const moduleId of Object.keys(output.modules)) {
          const manifest = packageManifest(moduleId);
          if (manifest?.name && manifest.version) {
            manifests.set(`${manifest.name}@${manifest.version}`, manifest);
          }
        }
      }

      const runtimeNames = new Set(
        [...manifests.values()].flatMap((manifest) => manifest.name ?? []),
      );
      const packages: RuntimePackage[] = [...manifests.values()]
        .map((manifest) => ({
          name: manifest.name ?? "",
          version: manifest.version ?? "",
          license: manifest.license ?? "unknown",
          repository: repositoryUrl(manifest.repository),
          dependencies: [
            ...Object.keys(manifest.dependencies ?? {}),
            ...Object.keys(manifest.peerDependencies ?? {}),
          ]
            .filter((name) => runtimeNames.has(name))
            .sort()
            .filter((name, index, names) => index === 0 || name !== names[index - 1]),
        }))
        .sort((left, right) =>
          left.name.localeCompare(right.name) || left.version.localeCompare(right.version),
        );
      this.emitFile({
        type: "asset",
        fileName: "runtime-packages.json",
        source: `${JSON.stringify(
          { schema: "volmap.frontend-runtime-packages/v1", packages },
          null,
          2,
        )}\n`,
      });
    },
  };
}

export default defineConfig({
  root: webRoot,
  plugins: [react(), runtimePackageManifest()],
  build: {
    assetsDir: "",
    cssCodeSplit: false,
    emptyOutDir: true,
    manifest: "manifest.json",
    modulePreload: false,
    outDir: outputDirectory,
    rollupOptions: {
      input: resolve(webRoot, "src/foundation.tsx"),
      output: {
        assetFileNames: "frontend[extname]",
        chunkFileNames: "chunk-[name].js",
        entryFileNames: "frontend.js",
      },
    },
    sourcemap: false,
    target: ["chrome107", "firefox104", "safari16"],
  },
});
