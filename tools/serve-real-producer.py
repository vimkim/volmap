"""Disposable real-CUBRID server for playwright.producer.config.ts.

VOLMAP_PRODUCER_RUN names a JSON file with producer_source, producer_build,
producer_install, consumer_binary, format_profile, listen_port, copy_listen_port,
and output_directory. All paths are explicit.
This helper owns only its temporary database registry and its child processes.
"""
import json
import hashlib
import os
from pathlib import Path
import signal
import shutil
import socket
import subprocess
import tempfile
import time


def main():
    config = json.loads(Path(os.environ["VOLMAP_PRODUCER_RUN"]).read_text())
    source = Path(config["producer_source"]).resolve(strict=True)
    build = Path(config["producer_build"]).resolve(strict=True)
    install = Path(config["producer_install"]).resolve(strict=True)
    consumer = Path(config["consumer_binary"]).resolve(strict=True)
    consumer_source = Path(__file__).resolve().parent.parent
    corpus = source / "docs/pgbuf-inspector/v1"
    vendored = consumer_source / "fixtures/pgbuf-inspector/v1"
    source_files = {p.relative_to(corpus) for p in corpus.rglob("*") if p.is_file()}
    consumer_files = {p.relative_to(vendored) for p in vendored.rglob("*") if p.is_file()}
    if not source_files or source_files != consumer_files or any(
            (corpus / p).read_bytes() != (vendored / p).read_bytes() for p in source_files):
        raise ValueError("producer and consumer corpus trees do not match")
    profile = config["format_profile"]
    if profile not in ("feat-oos", "develop"):
        raise ValueError("an explicit supported format profile is required")
    listen_port, copy_port = config["listen_port"], config["copy_listen_port"]
    if any(type(p) is not int or not 1 <= p <= 65535 for p in (listen_port, copy_port)) or listen_port == copy_port:
        raise ValueError("listen_port and copy_listen_port must be distinct explicit ports from 1 to 65535")
    output = Path(config["output_directory"]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    root = Path(tempfile.mkdtemp(prefix="volmap-real-"))
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    (root / "databases.txt").touch()
    base = (
        f"[common]\ncubrid_port_id={port}\ndata_buffer_size=64M\n"
        "log_buffer_size=4M\nvacuum_log_block_pages=4\n"
    )
    (root / "cubrid.conf").write_text(base)
    env = {**os.environ, "CUBRID": str(install), "CUBRID_DATABASES": str(root),
           "CUBRID_TMP": str(root), "CUBRID_CONF_FILE": str(root / "cubrid.conf"),
           "PATH": str(install / "bin") + os.pathsep + os.environ["PATH"],
           "LD_LIBRARY_PATH": str(install / "lib") + os.pathsep + str(install / "cci/lib")}
    name = "volmap07"
    log = (output / "commands.log").open("w")
    children = []

    def command(*args, check=True, timeout=60):
        log.write(json.dumps(list(args)) + "\n")
        log.flush()
        started = time.monotonic()
        result = subprocess.run(args, env=env, cwd=root, stdout=log, stderr=subprocess.STDOUT,
                                timeout=timeout, check=False, start_new_session=True)
        log.write(json.dumps({"exit_code": result.returncode, "elapsed_seconds": time.monotonic() - started}) + "\n")
        log.flush()
        if check:
            result.check_returncode()
        return result

    def stop(_signal, _frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        command("cubrid", "createdb", "--db-volume-size=20M", "--log-volume-size=20M",
                "-F", str(root), name, "en_US.utf8")
        copy = root / "copy"
        copy.mkdir()
        volumes = []
        copied_identities = []
        for line in (root / f"{name}_vinf").read_text().splitlines():
            volid, filename = line.split(maxsplit=1)
            if int(volid) >= 0:
                original = Path(filename).resolve(strict=True)
                if original.parent != root:
                    raise ValueError("fixture volume escaped its private root")
                destination = copy / original.name
                shutil.copyfile(original, destination)
                copied_identities.append({"volid": int(volid), "original_inode": original.stat().st_ino,
                                          "copied_inode": destination.stat().st_ino,
                                          "device": original.stat().st_dev})
                volumes.append(f"{volid} {destination}\n")
        if not volumes or any(v["original_inode"] == v["copied_inode"] for v in copied_identities):
            raise RuntimeError("fixture must copy nonempty permanent volumes to distinct files")
        (copy / f"{name}_vinf").write_text("".join(volumes))
        # This run actually omits the parameter, unlike start(False) in the
        # producer's older server_attachment.py driver.
        command("cubrid", "server", "start", name)
        command("csql", "-u", "dba", "-c", "select 1;", name)
        if (root / "pgbuf-inspector").exists():
            raise RuntimeError("omitted parameter unexpectedly activated the inspector")
        log.write("PASS omitted-parameter startup serves SQL without inspector directory\n")
        command("cubrid", "server", "stop", name)
        (root / "cubrid.conf").write_text(base + "enable_pgbuf_inspector=yes\n")
        command("cubrid", "server", "start", name)
        command("csql", "-u", "dba", "-c", "select 1;", name)
        sockets = list((root / "pgbuf-inspector").glob("*.sock"))
        if len(sockets) != 1:
            raise RuntimeError("producer did not activate exactly one fixture endpoint")
        metadata = {**config, "database_root": str(root), "socket": str(sockets[0]),
                    "copy_origin": f"http://127.0.0.1:{copy_port}",
                    "producer_commit": subprocess.check_output(
                        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip(),
                    "binary_sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest()
                                      for p in (consumer, install / "bin/cub_server", install / "lib/libcubrid.so")}}
        for label, repository in [("producer", source), ("consumer", consumer_source)]:
            metadata[label + "_commit"] = subprocess.check_output(
                ["git", "-C", str(repository), "rev-parse", "HEAD"], text=True).strip()
            metadata[label + "_worktree_status"] = subprocess.check_output(
                ["git", "-C", str(repository), "status", "--short"], text=True)
        metadata["producer_build_type"] = [line for line in (build / "CMakeCache.txt").read_text().splitlines()
                                           if line.startswith("CMAKE_BUILD_TYPE:")]
        metadata["corpus"] = json.loads((corpus / "manifest.json").read_text())
        metadata["corpus_identical_files"] = len(source_files)
        metadata["corpus_sha256"] = hashlib.sha256((corpus / "corpus/SHA256SUMS").read_bytes()).hexdigest()
        metadata["copied_identities"] = copied_identities
        metadata["omitted_parameter_sql_startup"] = "passed-with-no-inspector-directory"
        metadata["harness_sha256"] = {str(p.relative_to(consumer_source)): hashlib.sha256(p.read_bytes()).hexdigest()
                                      for p in (Path(__file__), consumer_source / "web/e2e/real-producer.spec.ts",
                                                consumer_source / "web/playwright.producer.config.ts")}
        (output / "runtime.json").write_text(json.dumps(metadata, indent=2) + "\n")
        for volume_root, http_port in [(root, listen_port), (copy, copy_port)]:
            args = [str(consumer), "serve", "--vinf", str(volume_root / f"{name}_vinf"),
                    "--volume-root", str(volume_root), "--format-profile", profile,
                    "--listen", f"127.0.0.1:{http_port}", "--progress", "never",
                    "--runtime-page-buffer", "--runtime-socket", str(sockets[0])]
            log.write(json.dumps(args) + "\n")
            log.flush()
            children.append(subprocess.Popen(args, env=env, cwd=root, stdout=log, stderr=subprocess.STDOUT,
                                             start_new_session=True))
        result = children[0].wait()
        if result:
            raise RuntimeError(f"Volmap exited with {result}; see {output / 'commands.log'}")
    except KeyboardInterrupt:
        pass
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        for child in reversed(children):
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
        try:
            try:
                command("cubrid", "server", "stop", name, check=False, timeout=10)
            finally:
                command("cub_commdb", "-A", check=False, timeout=5)
        finally:
            log.close()


if __name__ == "__main__":
    main()
