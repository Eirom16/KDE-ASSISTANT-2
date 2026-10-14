#!/usr/bin/env python3
"""Validación F1B1 con XDG/SQLite privados y mocks loopback; Qt opt-in."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import sys
sys.dont_write_bytecode = True
from validate_f1a import PIPER_TESTS, EXTERNAL_TESTS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qml", action="store_true", help="Flujo QML y ejecutable offscreen (puerto 8765 libre); no Plasma.")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    os.umask(0o077)
    evidence = Path(tempfile.mkdtemp(prefix="kda-f1b1-check-"))
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY") or key == "KDA_F1B1_CHILD":
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
        ("f1a-security", ["cargo", "test", "--locked", "--features", "legacy-fixtures", "--test", "f1a_security"]),
        ("f1b1", ["cargo", "test", "--locked", "--test", "f1b1_text_startup", "--", "--test-threads=1"]),
        ("unit-safe", ["cargo", "test", "--locked", "--lib", "--", "--test-threads=1"] + skips),
        ("piper-inherited-now-lazy", ["cargo", "test", "--locked", "--lib", "--", "--exact", "--test-threads=1"] + PIPER_TESTS),
        ("app-paths", ["cargo", "test", "--locked", "--test", "app_paths"]),
    ]
    if args.qml:
        commands.append(("qml-offscreen", ["cargo", "test", "--locked", "--test", "f1b1_text_startup", "qml_offscreen", "--", "--ignored", "--nocapture"]))
    results = {"base": "ca82e722935e8076f526a484655f2f95f87b5d4c", "commands": [], "excluded": EXTERNAL_TESTS, "qml_offscreen": args.qml, "plasma_wayland": "NOT_TESTED"}
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
        print("PASS de la selección F1B1; Plasma Wayland y voz real NOT_TESTED.", flush=True)
        return 0
    finally:
        for directory in ["tmp", "config", "cache", "data"]:
            shutil.rmtree(evidence / directory)


if __name__ == "__main__":
    raise SystemExit(main())
