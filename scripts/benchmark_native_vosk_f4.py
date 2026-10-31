#!/usr/bin/env python3
"""Offline native worker overhead on an explicitly authorised private corpus.

No microphone, downloads, Backend, Python inference or retained model. Separate
preflight and cold-worker processes; results with hypotheses remain private.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
sys.dont_write_bytecode = True
import time
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles
from compare_f4_spanish import REPO, baseline, corpus, profile_path, write_private
from f3_runtime import metrics
from f4_safety import private_env, require_private


def measured(command, root, env, stdout):
    require_private(root, env)
    if compilers():
        raise RuntimeError('Compilation overlaps benchmark')
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    samples, loads = [], []
    start = time.perf_counter()
    with stdout.open('xb') as output:
        stdout.chmod(0o600)
        child = subprocess.Popen(command, cwd=root, env=env, stdout=output,
                                 stderr=subprocess.DEVNULL, umask=0o077)
        try:
            while child.poll() is None:
                if time.perf_counter()-start > 30 or compilers():
                    raise RuntimeError('Worker timeout or compiler overlap')
                try:
                    samples.append(metrics(child.pid))
                except (OSError, ValueError):
                    pass
                loads.append(os.getloadavg())
                time.sleep(.05)
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
    if child.returncode:
        raise RuntimeError('Native worker failed; private output retained')
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return dict(wall_ms=(time.perf_counter()-start)*1000, pid=child.pid,
        reaped=not Path(f'/proc/{child.pid}').exists(),
        cpu_seconds=usage.ru_utime+usage.ru_stime-before.ru_utime-before.ru_stime,
        peak_sample_rss_kib=max((s['rss_kib'] for s in samples), default=0),
        peak_sample_pss_kib=max((s['pss_kib'] for s in samples), default=0),
        loadavg_min=list(map(min, zip(*loads))) if loads else [],
        loadavg_max=list(map(max, zip(*loads))) if loads else [],
        monitor_interval_ms=50, compiler_overlap=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--library', type=Path, required=True)
    parser.add_argument('--model', type=Path, required=True)
    parser.add_argument('--name', required=True, choices=['native-before', 'native-after'])
    parser.add_argument('--runs', type=int, default=3, choices=range(3, 11))
    args = parser.parse_args()
    root = profile_path(args.profile)
    env = private_env(root)
    require_private(root, env)
    revision = baseline(env)
    rows = corpus(root)
    if len(rows) != 1:
        parser.error('Choose one authorised workload per run')
    row = rows[0]
    output = root/args.name
    output.mkdir(mode=0o700)
    commands = {
        'preflight': [str(REPO/'target/debug/kde-assistant'), '--voice-vosk-check',
                      str(args.library), str(args.model)],
        'worker': [str(REPO/'target/debug/bench_voice'), '--vosk', str(args.library),
                   str(args.model), row['path']],
    }
    records = []
    for run in range(args.runs):
        for kind, base in commands.items():
            result_path = output/f'{kind}-{run}.json'
            command = base if kind == 'preflight' else [*base, str(result_path)]
            process = measured(command, root, {**env, 'OMP_NUM_THREADS': '1',
                'OPENBLAS_NUM_THREADS': '1'}, output/f'{kind}-{run}.stdout')
            if kind == 'preflight':
                if json.loads((output/f'{kind}-{run}.stdout').read_text()) is not None:
                    raise RuntimeError('Native preflight rejected artifacts')
                data = {}
            else:
                data = json.loads(result_path.read_text())
                data.pop('text')  # Hypothesis stays only in the private worker JSON.
            records.append(dict(run=run, kind=kind, process=process, **data))
    result = dict(sample=row['id'], wav_sha256=row['sha256'], seconds=row['frames']/16000,
        scope='Authorised human WAV, offline native worker; NOT live PTT',
        binary_sha256={name: hashlib.sha256((REPO/'target/debug'/name).read_bytes()).hexdigest()
                       for name in ['kde-assistant', 'bench_voice']},
        commands=commands, baseline=revision, records=records,
        summaries={kind: percentiles([r['process']['wall_ms'] for r in records if r['kind'] == kind])
                   for kind in commands})
    # Git status may include paths but never a hypothesis, credential or audio.
    write_private(output/'summary.json', result)
    print(json.dumps(result['summaries'], indent=2))


if __name__ == '__main__':
    main()
