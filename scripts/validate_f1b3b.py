#!/usr/bin/env python3
"""Validación F1B3B con XDG/SQLite privados y mocks loopback; Qt opt-in."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import sys
sys.dont_write_bytecode = True
from validate_f1a import EXTERNAL_TESTS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qml", action="store_true", help="Flujo QML y ejecutable offscreen (puerto 8765 libre); no Plasma.")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    os.umask(0o077)
    evidence = Path(tempfile.mkdtemp(prefix="kda-f1b3b-check-"))
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY") or key.upper().endswith("_PROXY") or key.startswith("KDA_F1"):
            env.pop(key)
    for key, directory in [("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data"), ("TMPDIR", "tmp")]:
        path = evidence / directory
        path.mkdir(mode=0o700)
        env[key] = str(path)
    env["CARGO_NET_OFFLINE"] = "true"
    skips = [argument for name in EXTERNAL_TESTS for argument in ["--skip", name]]
    commands = [
        ("fmt", ["cargo", "fmt", "--check"]),
        ("check", ["cargo", "check", "--locked", "--all-targets"]),
        ("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"]),
        ("f1a-security", ["cargo", "test", "--locked", "--test", "f1a_security"]),
        ("f1b1", ["cargo", "test", "--locked", "--test", "f1b1_text_startup", "--", "--test-threads=1"]),
        ("f1b2-conversation", ["cargo", "test", "--locked", "--test", "f1b2_conversation", "--", "--test-threads=1"]),
        ("f1b3a-persistence", ["cargo", "test", "--locked", "--test", "f1b3a_persistence", "--", "--test-threads=1"]),
        ("f1b3b-storage", ["cargo", "test", "--locked", "--test", "f1b3b_storage", "--", "--test-threads=1"]),
        ("unit-safe", ["cargo", "test", "--locked", "--lib", "--", "--test-threads=1"] + skips),
        ("app-paths", ["cargo", "test", "--locked", "--test", "app_paths"]),
    ]
    if args.qml:
        commands.append(("qml-offscreen", ["cargo", "test", "--locked", "--test", "f1b1_text_startup", "qml_offscreen", "--", "--ignored", "--nocapture"]))
        commands.append(("qml-f1b2-review", ["cargo", "test", "--locked", "--test", "f1b2_conversation", "qml_", "--", "--ignored", "--nocapture", "--test-threads=1"]))
        commands.append(("qml-f1b3b-recovery", ["cargo", "test", "--locked", "--test", "f1b3b_storage", "qml_", "--", "--ignored", "--nocapture", "--test-threads=1"]))
    results = {"base": "d8a6972b5bc3977408c8d0956fce3196623d5458", "commands": [], "excluded": EXTERNAL_TESTS, "qml_offscreen": args.qml, "plasma_wayland": "NOT_TESTED"}
    source_paths = sorted(p for base in ["src", "qml", "tests", "migrations", "scripts"] for p in (repo / base).rglob("*") if p.is_file() and (p.suffix in [".rs", ".qml", ".sql", ".py", ".js"] or p.name == "qmldir"))
    manifest = {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest() for p in source_paths}
    (evidence / "source-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Evidencia privada: {evidence}", flush=True)
    print("Sin modelos ni proveedores remotos. Unitarios excluyen sólo los 2 tests externos F1A. QML es opt-in.", flush=True)
    try:
        for name, command in commands:
            start = time.monotonic()
            log = evidence / f"{name}.log"
            with log.open("w") as output:
                result = subprocess.run(command, cwd=repo, env=env, stdout=output, stderr=subprocess.STDOUT)
            results["commands"].append({"name": name, "command": command, "exit": result.returncode, "seconds": round(time.monotonic()-start, 3)})
            (evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(f"{name}: exit {result.returncode}; log {log}", flush=True)
            if result.returncode:
                print(log.read_text(), flush=True)
                return 1
        changed = [name for name, digest in manifest.items() if hashlib.sha256((repo / name).read_bytes()).hexdigest() != digest]
        if changed:
            results["source_changed_during_validation"] = changed
            (evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print("FAIL: sources changed during validation", flush=True)
            return 1
        print("PASS de la selección F1B3B; Plasma Wayland y voz real NOT_TESTED.", flush=True)
        return 0
    finally:
        for directory in ["tmp", "config", "cache", "data"]:
            shutil.rmtree(evidence / directory)


if __name__ == "__main__":
    raise SystemExit(main())
