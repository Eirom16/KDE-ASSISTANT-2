#!/usr/bin/env python3
"""One consent-scoped corpus take. Never starts KDE Assistant or a model.

This helper has NO automatic consent. An operator must record the explicit chat
authorization AND a separate ready confirmation in a private one-use receipt.
It is an operational guard, not authentication against other processes of this UID.
"""
import argparse
import json
import os
from pathlib import Path
import selectors
import stat
import subprocess
import sys
sys.dont_write_bytecode = True
import time
import wave
from compare_f4_spanish import baseline, private_file, profile_path, write_private
from f4_safety import private_env, require_private
from f4_spanish import CORPUS


def admit(root, receipt):
    env = private_env(root)
    require_private(root, env)
    consent = json.loads(private_file(receipt, root, 16384).read_text())
    if not all(isinstance(consent.get(k), str) and consent[k].strip()
               for k in ('authorization_reference', 'ready_reference')):
        raise ValueError('Separate explicit authorization and ready confirmation required')
    samples = {row['id']: row for row in CORPUS}
    if consent.get('sample') not in samples or consent.get('purpose') != 'offline_f4r2_corpus':
        raise ValueError('Consent scope mismatch')
    seconds = consent.get('max_seconds')
    if type(seconds) is not int or not 1 <= seconds <= 15:
        raise ValueError('Consent duration must be 1..15 seconds')
    if consent.get('sample') == 'short' and seconds > 8:
        raise ValueError('Initial short-take consent allows at most eight seconds')
    marker = receipt.with_suffix('.used')
    if marker.exists():
        raise ValueError('One-use consent already consumed')
    destination = root/(consent['sample']+'.wav')
    if destination.exists() or destination.is_symlink():
        raise ValueError('Never overwrite a recording')
    return env, consent, marker, destination


def live_remote():
    remote = Path('/run/user')/str(os.getuid())/'pipewire-0'
    info = remote.lstat()
    if not stat.S_ISSOCK(info.st_mode) or info.st_uid != os.getuid():
        raise ValueError('Expected UID-owned PipeWire socket is unavailable')
    return remote


def record_command():
    # Omission means default device. Literal --target=auto becomes target.object
    # "auto" on this host and dont-fallback makes WirePlumber reject that name.
    return ['/usr/bin/pw-record', '--raw', '--format=s16', '--rate=16000',
               '--channels=1', '--quality=4',
               '--properties={"application.name":"KDA F4-R2 authorized corpus",'
               '"node.dont-reconnect":true,"node.dont-fallback":true,"node.dont-move":true}', '-']


def capture(root, receipt):
    env, consent, marker, destination = admit(root, receipt)
    baseline(env)
    # Only this dedicated recorder can reach this explicitly identified live socket.
    # The general F4 safety guard remains unchanged and still rejects live sockets.
    remote = live_remote()
    require_private(root, env)
    command = record_command()
    # Keep every HOME/XDG path private; grant only this audio client the live remote.
    live_env = {**env, 'PIPEWIRE_REMOTE': str(remote)}
    write_private(marker, dict(consumed=True, sample=consent['sample']))
    limit = consent['max_seconds']*16000*2
    pcm = bytearray()
    errors = bytearray()
    started = time.monotonic()
    first_data = None
    child = None
    completed = False
    failure = None
    try:
        child = subprocess.Popen(command, cwd=root, env=live_env, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, umask=0o077)
        with selectors.DefaultSelector() as selector:
            selector.register(child.stdout, selectors.EVENT_READ, 'pcm')
            selector.register(child.stderr, selectors.EVENT_READ, 'diagnostic')
            while len(pcm) < limit:
                now = time.monotonic()
                if now-started > consent['max_seconds']+3:
                    raise TimeoutError('Recording deadline; take discarded')
                if first_data is None and now-started > 3:
                    raise TimeoutError('No microphone PCM; take discarded')
                events = selector.select(.1)
                if not events:
                    if child.poll() is not None:
                        raise RuntimeError('Recorder stopped before completing take')
                    continue
                for key, _ in events:
                    data = os.read(key.fd, min(3200, limit-len(pcm)) if key.data == 'pcm' else 4096)
                    if key.data == 'diagnostic':
                        if not data:
                            selector.unregister(key.fileobj)
                        errors.extend(data[:max(0, 8192-len(errors))])
                        continue
                    if not data:
                        raise RuntimeError('Microphone disconnected/stopped; take discarded')
                    if first_data is None:
                        first_data = time.monotonic()
                    pcm.extend(data)
        completed = True
    except BaseException as error:
        failure = type(error).__name__
        raise
    finally:
        if child is not None:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
            child.stdout.close()
            errors.extend(child.stderr.read(max(0, 8192-len(errors))))
            child.stderr.close()
            write_private(root/(consent['sample']+'-attempt.json'), dict(sample=consent['sample'],
                result='COMPLETE' if completed else 'FAILED', error_type=failure,
                pid=child.pid, exit=child.returncode, reaped=not Path(f'/proc/{child.pid}').exists(),
                pcm_bytes=len(pcm), stderr_private=errors.decode(errors='replace')))
        if not completed:
            pcm.clear()
    # No audio file before the exact owned recorder has been killed and reaped.
    try:
        with destination.open('xb') as output:
            destination.chmod(0o600)
            with wave.open(output, 'wb') as wav:
                wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
                wav.writeframes(pcm)
        write_private(root/(consent['sample']+'-capture.json'), dict(sample=consent['sample'],
            category='PIPEWIRE_CORPUS_CAPTURE', live_microphone='PENDING_HUMAN_CONFIRMATION',
            product_ptt='NOT_TESTED', bytes=len(pcm),
            seconds=len(pcm)/32000, first_pcm_ms=(first_data-started)*1000,
            wall_ms=(time.monotonic()-started)*1000, pid=child.pid,
            reaped=not Path(f'/proc/{child.pid}').exists(), remote=str(remote),
            reference_confirmation='PENDING; do not score until speaker confirms actual words'))
    except BaseException:
        destination.unlink(missing_ok=True)
        raise
    finally:
        pcm.clear()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    capture(profile_path(args.profile), args.receipt)


if __name__ == '__main__':
    main()
