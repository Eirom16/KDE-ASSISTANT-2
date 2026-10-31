#!/usr/bin/env python3
"""Private offline corpus comparison; never records, downloads or starts Backend.

All audio, hypotheses and engine logs stay in a private directory outside Git.
--prepare creates references only. Human WAVs require explicit recorded consent
and per-sample reference confirmation in manifest.json before any inference.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import time
import wave
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles
from f3_runtime import metrics
from f4_safety import private_env, require_private
from f4_spanish import CORPUS, alignment, check_phrases, reduced_context

REPO = Path(__file__).resolve().parents[1]
BASE = '9544a58eba4b6adf6c6343c04db80813b084f76b'


def private_file(path, root, maximum):
    path = Path(path)
    if path.is_symlink() or not path.resolve(strict=True).is_relative_to(root):
        raise ValueError('Private file escapes profile or is symlinked')
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise ValueError('Private regular file owned by UID, mode 0600 required')
    if info.st_size > maximum:
        raise ValueError('Private file exceeds bound')
    return path


def profile_path(path):
    path = Path(path)
    if path.is_symlink():
        raise ValueError('Symlinked profile forbidden')
    root = path.resolve(strict=True)
    if root.is_relative_to(REPO) or REPO.is_relative_to(root):
        raise ValueError('Corpus/results must remain outside the repository')
    # private_env performs ownership, permission and XDG/socket checks.
    private_env(root)
    return root


def write_private(path, value):
    with path.open('x', encoding='utf-8') as output:
        path.chmod(0o600)
        json.dump(value, output, ensure_ascii=False, indent=2)
        output.write('\n')


def baseline(env):
    # No application launch until cwd, branch, HEAD and private environment agree.
    for arguments, expected in ((['branch', '--show-current'], 'recovery/f4-voice-coordinator'),
                                (['rev-parse', 'HEAD'], BASE)):
        actual = subprocess.check_output(['git', *arguments], cwd=REPO, env=env,
                                         text=True, timeout=5).strip()
        if actual != expected:
            raise ValueError('Unexpected repository baseline')
    status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO,
                                     env=env, text=True, timeout=5)
    return dict(branch='recovery/f4-voice-coordinator', head=BASE, status=status)


def prepare():
    root = Path(tempfile.mkdtemp(prefix='kda-f4r2-corpus-', dir='/var/tmp'))
    private_env(root)
    write_private(root / 'manifest.json', dict(schema=1, kind='human_speech',
        authorization=None, retention='Temporary offline F4-R2 evaluation; delete at closure',
        reference_license='CC0-1.0 authored text; NOT a license to redistribute recordings',
        samples=[dict(**row, file=row['id']+'.wav', reference_confirmed=False) for row in CORPUS]))
    return root


def corpus(root):
    manifest = json.loads(private_file(root/'manifest.json', root, 65536).read_text())
    if manifest.get('schema') != 1 or manifest.get('kind') != 'human_speech':
        raise ValueError('Explicit human corpus schema required')
    authorization = manifest.get('authorization')
    if not isinstance(authorization, str) or not authorization.strip():
        raise ValueError('Human recording/offline evaluation consent is missing')
    rows = manifest.get('samples')
    if not isinstance(rows, list) or not 1 <= len(rows) <= 12:
        raise ValueError('Corpus must contain 1..12 authorized samples')
    seen = set()
    for row in rows:
        identifier = row.get('id')
        if identifier not in {r['id'] for r in CORPUS} or identifier in seen:
            raise ValueError('Unknown or duplicate corpus ID')
        seen.add(identifier)
        if row.get('reference_confirmed') is not True or not row.get('reference'):
            raise ValueError('Speaker must confirm the actual spoken reference for each sample')
        alignment(row['reference'], '')  # Bounds reference before native inference.
        for alternatives in row.get('checks', []):
            if not isinstance(alternatives, list) or not alternatives:
                raise ValueError('Invalid declared checks')
            for alternative in alternatives:
                alignment(alternative, '')
        if row.get('file') != identifier+'.wav':
            raise ValueError('Only the expected local WAV filename is accepted')
        wav_path = private_file(root/row['file'], root, 960044)
        with wave.open(str(wav_path), 'rb') as wav:
            if (wav.getframerate(), wav.getnchannels(), wav.getsampwidth(), wav.getcomptype()) != (16000, 1, 2, 'NONE'):
                raise ValueError('Supply canonical WAV PCM16 mono 16 kHz; no implicit conversion')
            frames = wav.getnframes()
            reduced_context(frames)
            if len(wav.readframes(frames)) != frames*2:
                raise ValueError('Truncated WAV')
        row.update(path=str(wav_path), frames=frames,
                   sha256=hashlib.sha256(wav_path.read_bytes()).hexdigest())
    return rows


def vosk_worker(args, root):
    # Direct --worker calls also fail closed, before CDLL loads native code.
    require_private(root, os.environ)
    baseline(os.environ)
    rows = corpus(root)
    if str(args.wav.resolve(strict=True)) not in {row['path'] for row in rows}:
        raise ValueError('Worker audio is not part of the authorized corpus')
    if args.result.parent.resolve(strict=True) != (root/'comparison').resolve(strict=True):
        raise ValueError('Worker result must remain in the private comparison directory')
    if args.phase and args.phase.parent.resolve(strict=True) != root:
        raise ValueError('Cancellation marker must remain in the private profile')
    source = private_file(args.wav, root, 960044)
    with wave.open(str(source), 'rb') as wav:
        if (wav.getframerate(), wav.getnchannels(), wav.getsampwidth()) != (16000, 1, 2):
            raise ValueError('Canonical PCM required')
        reduced_context(wav.getnframes())
        start = time.perf_counter()
        pcm = wav.readframes(wav.getnframes())
        conversion_ms = (time.perf_counter()-start)*1000
    start = time.perf_counter()
    lib = ctypes.CDLL(str(args.vosk_lib.resolve(strict=True)))
    for name, types, result in (
        ('vosk_set_log_level', [ctypes.c_int], None),
        ('vosk_model_new', [ctypes.c_char_p], ctypes.c_void_p),
        ('vosk_model_free', [ctypes.c_void_p], None),
        ('vosk_recognizer_new', [ctypes.c_void_p, ctypes.c_float], ctypes.c_void_p),
        ('vosk_recognizer_free', [ctypes.c_void_p], None),
        ('vosk_recognizer_accept_waveform', [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int], ctypes.c_int),
        ('vosk_recognizer_result', [ctypes.c_void_p], ctypes.c_char_p),
        ('vosk_recognizer_final_result', [ctypes.c_void_p], ctypes.c_char_p)):
        function = getattr(lib, name)
        function.argtypes, function.restype = types, result
    lib.vosk_set_log_level(-1)
    library_ms = (time.perf_counter()-start)*1000
    start = time.perf_counter()
    model = lib.vosk_model_new(os.fsencode(args.vosk_model.resolve(strict=True)))
    if not model:
        raise RuntimeError('Vosk load failed')
    load_ms = (time.perf_counter()-start)*1000
    runs = []
    try:
        before = metrics(os.getpid())
        idle_start = time.monotonic()
        time.sleep(1)
        after = metrics(os.getpid())
        idle = dict(seconds=time.monotonic()-idle_start, cpu_ticks=after['cpu_ticks']-before['cpu_ticks'],
                    rss_kib=after['rss_kib'], pss_kib=after['pss_kib'])
        for _ in range(args.runs):
            start = time.perf_counter()
            recognizer = lib.vosk_recognizer_new(model, 16000)
            if not recognizer:
                raise RuntimeError('Vosk recognizer failed')
            state_ms = (time.perf_counter()-start)*1000
            try:
                chunks = []
                cpu = time.process_time()
                start = time.perf_counter()
                for offset in range(0, len(pcm), 6400):
                    piece = pcm[offset:offset+6400]
                    accepted = lib.vosk_recognizer_accept_waveform(recognizer, piece, len(piece))
                    if accepted < 0:
                        raise RuntimeError('Vosk inference failed')
                    if args.phase and not args.phase.exists():
                        pending = args.phase.with_suffix('.partial')
                        write_private(pending, dict(phase='pcm_block_processed', bytes=len(piece)))
                        os.link(pending, args.phase)
                        pending.unlink()
                    if accepted:
                        chunks.append(json.loads(lib.vosk_recognizer_result(recognizer)))
                chunks.append(json.loads(lib.vosk_recognizer_final_result(recognizer)))
                inference_ms = (time.perf_counter()-start)*1000
                cpu_seconds = time.process_time()-cpu
                extraction = time.perf_counter()
                text = ' '.join(c.get('text', '') for c in chunks).strip()
                alignment('', text)  # Bound before storing/scoring.
                text_ms = (time.perf_counter()-extraction)*1000
                runs.append(dict(inference_ms=inference_ms, text_available_ms=state_ms+inference_ms+text_ms,
                                 inference_cpu_seconds=cpu_seconds,
                                 measurement=dict(text=text, state_ms=state_ms, text_ms=text_ms)))
            finally:
                lib.vosk_recognizer_free(recognizer)
        write_private(args.result, dict(library_ms=library_ms, cold_model_load_ms=load_ms,
                                       conversion_ms=conversion_ms, loaded_idle=idle, warm_runs=runs))
    finally:
        lib.vosk_model_free(model)


def run_child(command, env, root, log, deadline):
    require_private(root, env)
    if compilers():
        raise RuntimeError('Wait for compilation before inference')
    samples = []
    loads = []
    start = time.monotonic()
    with log.open('xb') as output:
        log.chmod(0o600)
        child = subprocess.Popen(command, cwd=root, env=env, stdout=output,
                                 stderr=subprocess.STDOUT, umask=0o077)
        try:
            while child.poll() is None:
                if compilers():
                    raise RuntimeError('Compiler overlap invalidates this measurement')
                if time.monotonic()-start > deadline:
                    raise TimeoutError('Benchmark deadline; no automatic retry')
                if log.stat().st_size > 20*1024*1024:
                    raise RuntimeError('Engine diagnostic output exceeded bound')
                try:
                    samples.append(metrics(child.pid))
                except (OSError, ValueError):
                    pass
                loads.append(os.getloadavg())
                time.sleep(.1)
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
    if child.returncode:
        raise RuntimeError('Engine failed; diagnostic remains private')
    return dict(pid=child.pid, command=command, exit=child.returncode,
                reaped=not Path(f'/proc/{child.pid}').exists(),
                elapsed_ms=(time.monotonic()-start)*1000, compiler_overlap=False,
                peak_rss_kib=max((s['rss_kib'] for s in samples), default=0),
                peak_pss_kib=max((s['pss_kib'] for s in samples), default=0),
                sampled_cpu_ticks=max((s['cpu_ticks'] for s in samples), default=0),
                ticks_per_second=os.sysconf('SC_CLK_TCK'), loadavg=loads)


def compare(args, root):
    env = private_env(root)
    require_private(root, env)
    revision = baseline(env)
    rows = corpus(root)  # Missing consent/audio prevents ALL native process launches.
    if bool(args.vosk_lib) != bool(args.vosk_model):
        raise ValueError('Provide both Vosk runtime and model or neither')
    model = args.whisper_model.resolve(strict=True)
    output = root/'comparison'
    output.mkdir(mode=0o700)  # Never overwrite a previous run.
    results = []
    for row in rows:
        contexts = [('whisper_full', 0), ('whisper_reduced', reduced_context(row['frames']))]
        if args.vosk_lib:
            contexts.append(('vosk_small_es', None))
        # Paired same input; alternate order between samples, no concurrent engines.
        if [r['id'] for r in CORPUS].index(row['id']) % 2:
            contexts.reverse()
        for engine, context in contexts:
            prefix = output/(row['id']+'-'+engine)
            result = prefix.with_suffix('.json')
            if context is None:
                command = [sys.executable, str(Path(__file__).resolve()), '--worker', '--profile', str(root),
                           '--wav', row['path'], '--result', str(result), '--runs', str(args.runs),
                           '--vosk-lib', str(args.vosk_lib.resolve(strict=True)),
                           '--vosk-model', str(args.vosk_model.resolve(strict=True))]
            else:
                command = [str(REPO/'target/debug/bench_voice'), str(model), row['path'],
                           str(args.runs), str(result), '3', str(context), '0.2']
            child_env = {**env, 'OPENBLAS_NUM_THREADS': '1', 'OMP_NUM_THREADS': '1'}
            process = run_child(command, child_env, root, prefix.with_suffix('.log'), args.runs*180+30)
            private_file(result, root, 2*1024*1024)
            measured = json.loads(result.read_text())
            # bench_voice historically labels all WAVs as synthetic. This wrapper
            # supplies the source classification; never export that old label for speech.
            measured['evidence'] = 'AUTHORIZED_HUMAN_AUDIO_OFFLINE'
            result.write_text(json.dumps(measured, ensure_ascii=False, indent=2)+'\n')
            scored = []
            for run in measured['warm_runs']:
                hypothesis = run['measurement']['text']
                scored.append(dict(**alignment(row['reference'], hypothesis),
                                   checks=check_phrases(hypothesis, row.get('checks', []))))
            results.append(dict(sample=row['id'], sha256=row['sha256'], engine=engine,
                audio_ctx=context, seconds=row['frames']/16000, process=process, scores=scored,
                inference=percentiles([r['inference_ms'] for r in measured['warm_runs']]),
                text_available_warm=percentiles([r['text_available_ms'] for r in measured['warm_runs']]),
                cold_model_load_ms=measured['cold_model_load_ms'], conversion_ms=measured['conversion_ms']))
    aggregates = {}
    for engine in {r['engine'] for r in results}:
        selected = [r for r in results if r['engine'] == engine]
        primary = [r['scores'][0] for r in selected]
        count = sum(s['reference_words'] for s in primary)
        errors = sum(s['errors'] for s in primary)
        aggregates[engine] = dict(unique_clips=len(selected), reference_words=count,
            primary_run0_errors=errors, primary_run0_wer=errors/count if count else None,
            repeat_score_variation=any(len({s['wer'] for s in r['scores']}) > 1 for r in selected),
            substitutions=sum(s['substitution'] for s in primary),
            deletions=sum(s['deletion'] for s in primary), insertions=sum(s['insertion'] for s in primary),
            unjustified_empty=sum(s['empty_unjustified'] for s in primary))
    write_private(output/'results.json', dict(baseline=revision, category='AUTHORIZED_HUMAN_AUDIO_OFFLINE',
        live_ptt='NOT_TESTED', results=results, corpus_scores=aggregates,
        wer_policy='NFC/case/punctuation only; accents retained; strict numbers; run0 per unique clip for corpus WER',
        warning='Repeated inference is not additional speakers; small corpus cannot represent all accents/environments'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--whisper-model', type=Path)
    parser.add_argument('--vosk-lib', type=Path)
    parser.add_argument('--vosk-model', type=Path)
    parser.add_argument('--runs', type=int, choices=range(1, 11), default=3)
    parser.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    parser.add_argument('--wav', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--result', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--phase', type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.prepare:
        if args.worker or args.profile or args.whisper_model or args.vosk_lib or args.vosk_model:
            parser.error('--prepare must be used alone')
        print(prepare())
        return
    if not args.profile:
        parser.error('Explicit private --profile required')
    if args.worker:
        # Validate inherited environment before profile_path can create defaults.
        require_private(args.profile, os.environ)
        if not all((args.wav, args.result, args.vosk_lib, args.vosk_model)):
            parser.error('Incomplete restricted worker arguments')
        vosk_worker(args, profile_path(args.profile))
    else:
        if not args.whisper_model or args.wav or args.result or args.phase:
            parser.error('Explicit --whisper-model required; internal arguments forbidden')
        compare(args, profile_path(args.profile))


if __name__ == '__main__':
    main()
