#!/usr/bin/env python3
"""F2 isolated, offline validation. Never invokes providers, models or a microphone."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time


def main():
    repo = Path(__file__).resolve().parent.parent
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY") or key.upper().endswith("_PROXY"):
            env.pop(key)
    env["CARGO_NET_OFFLINE"] = "true"
    env["QT_QPA_PLATFORM"] = "offscreen"
    env["QML_DISABLE_DISK_CACHE"] = "1"
    with tempfile.TemporaryDirectory(prefix="kda-f2-validation-") as private:
        base = Path(private)
        for key, directory in (("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                               ("XDG_CACHE_HOME", "cache"), ("TMPDIR", "tmp")):
            path = base / directory
            path.mkdir(mode=0o700)
            env[key] = str(path)
        qml = shutil.which("qmltestrunner") or "/usr/lib/qt6/bin/qmltestrunner"
        commands = [
            ("fmt", ["cargo", "fmt", "--check"]),
            ("check", ["cargo", "check", "--locked", "--all-targets"]),
            ("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"]),
            ("clippy-fixtures", ["cargo", "clippy", "--locked", "--features", "legacy-fixtures", "--all-targets", "--", "-D", "warnings"]),
            ("f1a", ["cargo", "test", "--locked", "--features", "legacy-fixtures", "--test", "f1a_security", "--", "--test-threads=1"]),
            ("f1b1", ["cargo", "test", "--locked", "--test", "f1b1_text_startup", "--", "--test-threads=1"]),
            ("f1b2", ["cargo", "test", "--locked", "--test", "f1b2_conversation", "--", "--test-threads=1"]),
            ("f1b3a", ["cargo", "test", "--locked", "--test", "f1b3a_persistence", "--", "--test-threads=1"]),
            ("f1b3b", ["cargo", "test", "--locked", "--features", "legacy-fixtures", "--test", "f1b3b_storage", "--", "--test-threads=1"]),
            ("f2", ["cargo", "test", "--locked", "--test", "f2_permissions", "--", "--test-threads=1"]),
            ("f2-http", ["cargo", "test", "--locked", "--lib", "f2_http_consent", "--", "--test-threads=1"]),
            ("f2-qml", [qml, "-import", str(repo), "-input", str(repo / "tests/qml/tst_f2_consent.qml"), "-o", "-,txt"]),
            ("gate-w1r", ["python3", "scripts/validate_gate_w1r.py"]),
        ]
        results = []
        for name, command in commands:
            start = time.monotonic()
            if name == "f2-qml" and not Path(qml).is_file():
                results.append({"name": name, "status": "NOT_TESTED", "reason": "qmltestrunner unavailable"})
                continue
            result = subprocess.run(command, cwd=repo, env=env, text=True,
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
            log = base / f"{name}.log"
            log.write_text(result.stdout)
            summaries = re.findall(r"test result: .*?(\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", result.stdout)
            summary = summaries[-1] if summaries else None
            qml_summary = re.findall(r"Totals: (\d+) passed, (\d+) failed, (\d+) skipped", result.stdout)
            entry = {"name": name, "status": "PASS" if result.returncode == 0 else "FAIL",
                     "exit": result.returncode, "seconds": round(time.monotonic()-start, 2),
                     "counts": ({"pass": int(summary[0]), "fail": int(summary[1]),
                                 "ignored": int(summary[2]), "filtered": int(summary[4])}
                                if summary else ({"pass":int(qml_summary[-1][0]),"fail":int(qml_summary[-1][1]),"ignored":int(qml_summary[-1][2]),"filtered":0} if qml_summary else None)),
                     "log": str(log)}
            results.append(entry)
            print(f"{name}: {entry['status']} ({entry['seconds']}s)", flush=True)
            if result.returncode:
                print(result.stdout[-6000:], flush=True)
        print(json.dumps({"results": results, "external_services": "NOT_TESTED",
                          "models_microphone": "NOT_TESTED", "plasma_wayland": "NOT_TESTED"}, indent=2), flush=True)
        return 1 if any(item["status"] == "FAIL" for item in results) else 0


if __name__ == "__main__":
    raise SystemExit(main())
