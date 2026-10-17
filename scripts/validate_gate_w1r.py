#!/usr/bin/env python3
"""Regresiones QML aisladas de W1R; no certifican KWin Wayland."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from qml_test_harness import build_runner


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    runner = shutil.which("qmltestrunner") or "/usr/lib/qt6/bin/qmltestrunner"
    if not Path(runner).is_file():
        print("qmltestrunner no disponible")
        return 2
    with tempfile.TemporaryDirectory(prefix="kda-gate-w1r-qml-") as private:
        env = os.environ.copy()
        for key in list(env):
            if key.endswith("_API_KEY") or key.upper().endswith("_PROXY"):
                env.pop(key)
        for key, name in (
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_CACHE_HOME", "cache"),
            ("TMPDIR", "tmp"),
        ):
            path = Path(private) / name
            path.mkdir(mode=0o700)
            env[key] = str(path)
        env["QT_QPA_PLATFORM"] = "offscreen"
        env["QML_DISABLE_DISK_CACHE"] = "1"
        runner = env.get("KDA_QML_TEST_RUNNER") or str(build_runner(repo, Path(private), env))
        command = [
            runner,
            "-import", str(repo),
            "-input", str(repo / "tests/qml/tst_gate_w1r.qml"),
            "-o", "-,txt",
        ]
        print("QML offscreen aislado; la prueba real de KWin se registra por separado.", flush=True)
        return subprocess.run(command, cwd=repo, env=env, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
