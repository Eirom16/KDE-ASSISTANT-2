"""Build a local QApplication Qt Quick Test runner; no system installation."""
from pathlib import Path
import shlex
import subprocess


def build_runner(repo: Path, output: Path, env: dict) -> Path:
    flags = subprocess.check_output(
        ["pkg-config", "--cflags", "--libs", "Qt6Widgets", "Qt6QuickTest"],
        env=env, text=True,
    )
    binary = output / "qmltest-qapplication"
    subprocess.run(["c++", "-std=c++17", "-fPIC", str(repo / "tests/qt/quicktest_main.cpp"),
                    "-o", str(binary), *shlex.split(flags)], env=env, check=True)
    return binary
