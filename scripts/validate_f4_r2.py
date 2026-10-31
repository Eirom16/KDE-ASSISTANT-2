#!/usr/bin/env python3
"""Sequential R2 regression gates. No recording, inference or downloads.

Plasma gates use the existing session bus/display only for their own fixture
windows. HOME, TMP and all XDG roots remain private; audio stays unreachable.
No application profile, DB, bearer or endpoint from the normal profile is used.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import time
from benchmark_f4 import compilers
from compare_f4_spanish import baseline, REPO
from f4_safety import private_env, require_private


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    # Refuse to overwrite an earlier regression record.
    if (out/'results.json').exists():
        parser.error('Use a new output directory')
    host_bus = os.environ.get('DBUS_SESSION_BUS_ADDRESS')
    display = os.environ.get('WAYLAND_DISPLAY')
    host_runtime = os.environ.get('XDG_RUNTIME_DIR')
    if not host_bus or not display or not host_runtime:
        parser.error('Official --plasma gates need an existing Wayland session')
    if not Path(display).is_absolute():
        display = str(Path(host_runtime)/display)
    if compilers():
        parser.error('Wait for other compilation before the serial gates')
    commands = [
        ('fmt', ['cargo', 'fmt', '--check'], False),
        ('clippy', ['cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings'], False),
        ('clippy-legacy', ['cargo', 'clippy', '--locked', '--features', 'legacy-fixtures', '--all-targets', '--', '-D', 'warnings'], False),
        ('build', ['cargo', 'build', '--locked', '--bin', 'kde-assistant', '--bin', 'bench_voice'], False),
        ('f4', ['python3', '-B', 'scripts/validate_f4.py', '--output', str(out/'f4')], False),
        ('f35', ['python3', '-B', 'scripts/validate_f35.py', '--plasma'], True),
        ('f2', ['python3', '-B', 'scripts/validate_f2.py'], False),
        ('f3', ['python3', '-B', 'scripts/validate_f3.py', '--runtime-only', '--plasma'], True),
        ('f1b3b', ['python3', '-B', 'scripts/validate_f1b3b.py', '--qml'], False),
    ]
    results = []
    with tempfile.TemporaryDirectory(prefix='kda-f4r2-gates-', dir='/var/tmp') as directory:
        root = Path(directory)
        env = private_env(root)
        require_private(root, env)
        revision = baseline(env)
        env.update(CARGO_BUILD_JOBS='2', QT_QPA_PLATFORM='offscreen', PYTHONDONTWRITEBYTECODE='1')
        for name, command, plasma in commands:
            require_private(root, env)
            launched = dict(env)
            if plasma:
                # Explicit desktop-infrastructure exception for these official gates;
                # never change the XDG roots or grant an audio socket.
                launched.update(DBUS_SESSION_BUS_ADDRESS=host_bus, WAYLAND_DISPLAY=display)
            started = time.monotonic()
            with (out/(name+'.log')).open('x') as log:
                done = subprocess.run(command, cwd=REPO, env=launched, stdout=log,
                                      stderr=subprocess.STDOUT, timeout=1200)
            text = (out/(name+'.log')).read_text()
            for candidate in re.findall(r'^Evidence:\s*(.+)$', text, re.M):
                evidence = Path(candidate)/'results.json'
                if evidence.is_file() and evidence.resolve().is_relative_to(root):
                    shutil.copyfile(evidence, out/(name+'-results.json'))
            results.append(dict(name=name, command=command, exit=done.returncode,
                                status='PASS' if done.returncode == 0 else 'FAIL',
                                seconds=time.monotonic()-started, plasma_own_windows=plasma))
            (out/'results.json').write_text(json.dumps(dict(baseline=revision, results=results,
                microphone='NOT_USED_BY_GATES', audio_remote='f4-unreachable-audio',
                home_tmp_xdg='PRIVATE', provider='SYNTHETIC_LOOPBACK_ONLY'), indent=2)+'\n')
            print(name, results[-1]['status'], flush=True)
            if done.returncode:
                print(text[-5000:])
                return done.returncode
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
