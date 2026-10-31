#!/usr/bin/env python3
"""Verify the new persistent protocol and process residency using PCM bytes only.

No PipeWire, microphone, playback, Backend, provider, download, or repeated corpus benchmark.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import struct
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
from f4_safety import private_env


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--models', type=Path, required=True)
    p.add_argument('--onnx-runtime', type=Path, required=True)
    a = p.parse_args()
    binary = a.binary.resolve(strict=True)
    models = a.models.resolve(strict=True)
    runtime = a.onnx_runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='kda-background-protocol-') as temporary:
        root = Path(temporary)
        env = private_env(root)
        child = subprocess.Popen([str(binary), '--voice-wake-stream', str(models), str(runtime), '0.8'],
                                 cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL)
        def exact(n, timeout=3):
            result = bytearray()
            deadline = time.monotonic() + timeout
            while len(result) < n:
                if not select.select([child.stdout], [], [], max(0, deadline - time.monotonic()))[0]:
                    raise TimeoutError('Persistent wake worker timed out')
                block = os.read(child.stdout.fileno(), n-len(result))
                if not block:
                    raise EOFError('Persistent wake worker stopped')
                result.extend(block)
            return bytes(result)
        def metrics():
            tail = Path(f'/proc/{child.pid}/stat').read_text().rsplit(')', 1)[1].split()
            rollup = Path(f'/proc/{child.pid}/smaps_rollup').read_text().splitlines()
            values = {line.split(':')[0]: int(line.split()[1]) for line in rollup if line.startswith(('Rss:', 'Pss:'))}
            return {'cpu_ticks': int(tail[11])+int(tail[12]), 'rss_kib': values['Rss'], 'pss_kib': values['Pss']}
        try:
            assert exact(1, 15) == b'R'
            before = metrics(); started = time.monotonic(); maximum = 0.0
            for i in range(75):
                child.stdin.write(b'F'+bytes(2560)); child.stdin.flush()
                score = struct.unpack('<f', exact(4))[0]
                assert 0 <= score <= 1 and score < .8
                maximum = max(maximum, score)
                time.sleep(max(0, started+(i+1)*.08-time.monotonic()))
            elapsed = time.monotonic()-started; after = metrics()
            active_cpu = (after['cpu_ticks']-before['cpu_ticks'])/os.sysconf('SC_CLK_TCK')
            child.stdin.write(b'X'); child.stdin.flush(); assert exact(1) == b'R'
            idle_before = metrics(); time.sleep(2); idle_after = metrics()
            child.stdin.write(b'F'+bytes(2560)); child.stdin.flush(); exact(4)
            child.stdin.close(); assert child.wait(timeout=5) == 0
            assert not Path(f'/proc/{child.pid}').exists()
            assert not (root/'config/kde-assistant').exists()
            print(json.dumps({'input':'offline_PCM16_silence_16k','frames':75,'stream_seconds':elapsed,
                'cpu_seconds':active_cpu,'cpu_percent_one_core':100*active_cpu/elapsed,
                'resident':after,'idle_2s_cpu_ticks':idle_after['cpu_ticks']-idle_before['cpu_ticks'],
                'max_score':maximum,'reset_same_process':True,'worker_reaped':True,'microphone_opened':False,
                'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()},indent=2))
        finally:
            if child.poll() is None:
                child.kill(); child.wait(timeout=5)
            child.stdout.close()
            if not child.stdin.closed:
                child.stdin.close()


if __name__ == '__main__':
    main()
