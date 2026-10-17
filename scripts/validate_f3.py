#!/usr/bin/env python3
"""F3 isolated validation. No portal registration or personal KDE settings changes."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import sys
sys.dont_write_bytecode = True
from qml_test_harness import build_runner
from f3_runtime import runtime_tests


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--specific-only", action="store_true", help="Omit inherited regressions already run separately")
    parser.add_argument("--plasma", action="store_true", help="Own windows on the current real Plasma Wayland session")
    parser.add_argument("--runtime-only", action="store_true", help="Reuse a built binary and repeat only own-process/window tests")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    os.umask(0o077)
    evidence = Path(tempfile.mkdtemp(prefix="kda-f3-validation-"))
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY") or key.upper().endswith("_PROXY") or key.startswith("KDA_F1"):
            env.pop(key)
    for key, folder in [("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                        ("XDG_CACHE_HOME", "cache"), ("TMPDIR", "tmp")]:
        directory = evidence / folder
        directory.mkdir(mode=0o700)
        env[key] = str(directory)
    env.update(CARGO_NET_OFFLINE="true", QT_QPA_PLATFORM="offscreen", QML_DISABLE_DISK_CACHE="1",
        QT_FORCE_STDERR_LOGGING="1", QT_ASSUME_STDERR_HAS_CONSOLE="1")
    runner = evidence / "qmltest-qapplication"
    if not args.runtime_only:
        runner = build_runner(repo, evidence, env)
        env["KDA_QML_TEST_RUNNER"] = str(runner)
    commands = [
        ("fmt", ["cargo", "fmt", "--check"], "UNIT"),
        ("f3-unit", ["cargo", "test", "--locked", "--test", "f3_desktop", "--", "--test-threads=1"], "UNIT"),
        ("owner-thread", ["cargo", "test", "--locked", "--test", "f3_process_owner"], "UNIT"),
        ("desktop-http", ["cargo", "test", "--locked", "--lib", "f3_http_desktop", "--", "--test-threads=1"], "UNIT"),
        ("paths", ["cargo", "test", "--locked", "--test", "app_paths"], "UNIT"),
        ("theme-unit", ["cargo", "test", "--locked", "--lib", "backend::kde_integration::tests", "--", "--test-threads=1"], "UNIT"),
        ("f3-qml", [str(runner), "-import", str(repo), "-input", str(repo / "tests/qml/tst_f3_desktop.qml"), "-o", "-,txt"], "AUTOMATED_QML"),
    ]
    if not args.specific_only:
        commands += [
            ("regression-f2", ["python3", "scripts/validate_f2.py"], "UNIT/AUTOMATED_QML"),
            ("regression-f1b3b", ["python3", "scripts/validate_f1b3b.py", "--qml"], "UNIT/AUTOMATED_QML"),
            ("regression-gate-w1r", ["python3", "scripts/validate_gate_w1r.py"], "AUTOMATED_QML"),
        ]
    # Legacy regression features may rebuild target/debug/kde-assistant. Restore
    # the default production feature set before any own-process runtime test.
    commands.append(("build", ["cargo", "build", "--locked", "--bin", "kde-assistant"], "UNIT"))
    if args.runtime_only:
        commands = []
    results = []
    print(f"Evidence: {evidence}", flush=True)
    def save():
        (evidence / "results.json").write_text(json.dumps({"results": results,
            "human_verified": "NOT_TESTED", "real_shortcut_registration": "NOT_TESTED",
            "plasma_wayland": "REQUESTED" if args.plasma else "NOT_TESTED"}, indent=2))
    for name, command, category in commands:
        started = time.monotonic()
        log = evidence / (name+".log")
        with log.open("w") as output:
            completed = subprocess.run(command, env=env, cwd=repo, stdout=output, stderr=subprocess.STDOUT)
        text = log.read_text()
        rust = re.findall(r"test result:.*?(\d+) passed; (\d+) failed; (\d+) ignored;.*?(\d+) filtered out", text)
        qml = re.findall(r"Totals: (\d+) passed, (\d+) failed, (\d+) skipped", text)
        results.append({"name": name, "command": command, "category": category,
            "status": "PASS" if completed.returncode == 0 else "FAIL", "exit": completed.returncode,
            "seconds": round(time.monotonic()-started, 3), "rust_counts": rust, "qml_counts": qml, "log": str(log)})
        save()
        print(f"{name}: {results[-1]['status']}", flush=True)
        if completed.returncode:
            print(text[-7000:])
            return 1
    try:
        results.extend(runtime_tests(repo, evidence, env, plasma=args.plasma))
    except Exception as error:
        results.append({"name": "runtime", "status": "FAIL", "reason": str(error)})
        raise
    finally:
        save()
    blocked = [result["name"] for result in results if result["status"] == "BLOCKED"]
    print("F3 automated selection: "+("BLOCKED: "+", ".join(blocked) if blocked else "PASS")+
        "; human input and real shortcut consent remain separate.", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
