"""Extract real develop pages after clean standalone shutdown; never use Volmap.

Usage: python3 generate.py --source SOURCE --install INSTALL --database-root NEW_DIR
Requires the delivered producer at the pinned source commit and an empty output
pages directory. Retain the source database outside the repository.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess

SOURCE_COMMIT = "48a3e87e3d56d3b0c998d286f17fc6674bb5e547"
PAGE_SIZE = 16384
# storage_common.h PAGE_TYPE, native develop ordinals (not Volmap output).
KINDS = {1: "file-table", 2: "heap", 3: "volume-header", 4: "volume-bitmap",
         5: "query-result", 6: "extensible-hash", 7: "overflow", 8: "area",
         9: "catalog", 10: "btree", 11: "log", 12: "dropped-files", 13: "vacuum-data"}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--install", type=Path, required=True)
    parser.add_argument("--database-root", type=Path, required=True)
    args = parser.parse_args()
    source, install = args.source.resolve(strict=True), args.install.resolve(strict=True)
    commit = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if commit != SOURCE_COMMIT:
        raise ValueError("generator requires the pinned delivered develop source")
    here = Path(__file__).resolve().parent
    (here / "pages").mkdir(exist_ok=False)
    root = args.database_root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / "databases.txt").touch()
    (root / "cubrid.conf").write_text("[common]\ndata_buffer_size=64M\nlog_buffer_size=4M\n")
    env = {**os.environ, "CUBRID": str(install), "CUBRID_DATABASES": str(root),
           "CUBRID_CONF_FILE": str(root / "cubrid.conf"), "CUBRID_TMP": str(root),
           "PATH": str(install / "bin") + os.pathsep + os.environ["PATH"],
           "LD_LIBRARY_PATH": str(install / "lib") + os.pathsep + str(install / "cci/lib")}
    name = "volmap_develop"
    commands = [
        [str(install / "bin/cubrid"), "createdb", "--db-page-size=16K", "--db-volume-size=32M",
         "--log-volume-size=20M", "-F", str(root), name, "en_US.utf8"],
        [str(install / "bin/csql"), "-S", "-u", "dba", "-i", str(here / "generate.sql"), name],
    ]
    with (here / "generation.log").open("w") as log:
        for command in commands:
            log.write(json.dumps(command) + "\n")
            log.flush()
            result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT,
                                    timeout=120, check=False)
            log.write(f"exit_code={result.returncode}\n")
            result.check_returncode()
    # Each standalone utility has exited; read stable disk images only now.
    pages, volumes, roles_seen, counts = [], [], set(), {}
    for line in (root / (name + "_vinf")).read_text().splitlines():
        volid, filename = line.split(maxsplit=1)
        volid = int(volid)
        if volid < 0:
            continue
        volume = Path(filename).resolve(strict=True)
        if volume.parent != root or volume.stat().st_size % PAGE_SIZE:
            raise ValueError("unexpected fixture volume path or geometry")
        volumes.append({"volid": volid, "path": str(volume), "size": volume.stat().st_size,
                        "sha256": digest(volume)})
        with volume.open("rb") as f:
            for pageid in range(volume.stat().st_size // PAGE_SIZE):
                data = f.read(PAGE_SIZE)
                stored_page, stored_vol, raw = struct.unpack_from("<ihB", data, 8)
                if (stored_page, stored_vol) != (pageid, volid) or raw == 0:
                    continue
                kind = KINDS[raw]
                counts[kind] = counts.get(kind, 0) + 1
                role = kind
                if raw == 2:
                    # Native SPAGE_SLOT: offset:14, length:14, record_type:4.
                    length = (struct.unpack_from("<I", data, PAGE_SIZE - 12)[0] >> 14) & 0x3fff
                    role = "heap-header" if length == 1152 else "heap-chain" if length == 40 else "heap-other"
                if raw == 7:
                    # Keep the complete two/three page chain, not a single sample.
                    role = f"overflow-{pageid}"
                key = (volid, role)
                if key in roles_seen:
                    continue
                roles_seen.add(key)
                filename = f"pages/vol{volid}-page{pageid}.bin"
                (here / filename).write_bytes(data)
                pages.append({"file": filename, "volid": volid, "pageid": pageid, "native_ordinal": raw,
                              "kind": kind, "role": role, "sha256": hashlib.sha256(data).hexdigest()})
    manifest = {"schema": "volmap.develop-disk-corpus", "revision": 1, "engine_commit": commit,
                "engine_base": "8cb558b3b264b5ed045ab24fa50b0ef825925349", "format_profile": "develop",
                "page_size": PAGE_SIZE, "synthetic_content_only": True, "clean_standalone_shutdown": True,
                "commands": commands, "volumes": volumes, "pages": pages, "native_page_counts": counts,
                "input_sha256": {str(p): digest(p) for p in [install / "bin/cubrid", install / "bin/csql",
                                   install / "lib/libcubridsa.so", source / "src/storage/storage_common.h",
                                   source / "src/storage/file_io.h", source / "src/storage/heap_file.c"]},
                "generator_sha256": {p.name: digest(p) for p in [Path(__file__), here / "generate.sql"]}}
    (here / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Extracted {len(pages)} real pages across {len(volumes)} volumes; native counts: {counts}")


if __name__ == "__main__":
    main()
