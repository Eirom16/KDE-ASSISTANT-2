#!/usr/bin/env python3
"""Explicit installed artifacts only. Offline inference experiment, never a server/agent.

No pip, downloads, audio devices, application configuration or global installation.
The ctypes adapter lives only in this benchmark, not in the product voice pipeline.
"""
import argparse
import ctypes
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import time
import wave
from benchmark_f4 import compilers
from f3_runtime import metrics
from f4_safety import private_env
from voice_corpus import generate


def percentiles(values):
    values = sorted(values)
    def p(q):
        import math
        return values[max(0, math.ceil(len(values)*q)-1)]
    median = statistics.median(values)
    return dict(n=len(values), p50_ms=p(.5), p95_ms=p(.95), min_ms=values[0], max_ms=values[-1],
                mean_ms=statistics.mean(values), stddev_ms=statistics.pstdev(values),
                mad_ms=statistics.median(abs(x-median) for x in values))


def worker(args):
    started = time.perf_counter()
    lib = ctypes.CDLL(str(args.lib.resolve(strict=True)))
    lib.vosk_set_log_level.argtypes = [ctypes.c_int]
    lib.vosk_set_log_level(-1)
    lib.vosk_model_new.argtypes = [ctypes.c_char_p]
    lib.vosk_model_new.restype = ctypes.c_void_p
    lib.vosk_model_free.argtypes = [ctypes.c_void_p]
    lib.vosk_recognizer_new.argtypes = [ctypes.c_void_p, ctypes.c_float]
    lib.vosk_recognizer_new.restype = ctypes.c_void_p
    lib.vosk_recognizer_free.argtypes = [ctypes.c_void_p]
    lib.vosk_recognizer_accept_waveform.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
    lib.vosk_recognizer_accept_waveform.restype = ctypes.c_int
    for name in ('vosk_recognizer_result', 'vosk_recognizer_final_result'):
        fn = getattr(lib,name); fn.argtypes = [ctypes.c_void_p]; fn.restype = ctypes.c_char_p
    library_ms = (time.perf_counter()-started)*1000
    start = time.perf_counter()
    model = lib.vosk_model_new(os.fsencode(args.model.resolve(strict=True)))
    if not model: raise RuntimeError('Vosk model load failed')
    load_ms = (time.perf_counter()-start)*1000
    rows = []
    try:
        # Loaded idle is separate from library/model load and inference.
        before = metrics(os.getpid()); idle_start = time.monotonic(); time.sleep(2)
        after = metrics(os.getpid())
        idle = dict(seconds=time.monotonic()-idle_start, cpu_ticks=after['cpu_ticks']-before['cpu_ticks'],
                    rss_kib=after['rss_kib'], pss_kib=after['pss_kib'])
        for signal in ('tone','silence','noise'):
            convert = time.perf_counter()
            with wave.open(str(args.corpus/(signal+'.wav')),'rb') as wav:
                assert (wav.getframerate(),wav.getnchannels(),wav.getsampwidth()) == (16000,1,2)
                assert wav.getnframes() <= 120*16000
                pcm = wav.readframes(wav.getnframes())
            conversion_ms = (time.perf_counter()-convert)*1000
            for i in range(args.runs):
                start = time.perf_counter()
                recognizer = lib.vosk_recognizer_new(model,16000)
                if not recognizer: raise RuntimeError('Vosk recognizer creation failed')
                state_ms = (time.perf_counter()-start)*1000
                cpu = metrics(os.getpid())['cpu_ticks']; start = time.perf_counter(); chunks=[]
                try:
                    for offset in range(0,len(pcm),6400):
                        piece = pcm[offset:offset+6400]
                        accepted = lib.vosk_recognizer_accept_waveform(recognizer,piece,len(piece))
                        if accepted < 0: raise RuntimeError('Vosk PCM failed')
                        if args.phase and not args.phase.exists():
                            marker = args.phase.with_suffix('.tmp'); marker.write_text('inference'); marker.replace(args.phase)
                        if accepted: chunks.append(json.loads(lib.vosk_recognizer_result(recognizer)))
                    chunks.append(json.loads(lib.vosk_recognizer_final_result(recognizer)))
                    inference_ms = (time.perf_counter()-start)*1000
                    text_start = time.perf_counter(); text=' '.join(c.get('text','') for c in chunks).strip()
                    row = dict(signal=signal, run=i, conversion_ms=conversion_ms, state_ms=state_ms,
                               inference_ms=inference_ms,text_ms=(time.perf_counter()-text_start)*1000,
                               text=text,cpu_ticks=metrics(os.getpid())['cpu_ticks']-cpu)
                    rows.append(row)
                finally: lib.vosk_recognizer_free(recognizer)
        args.output.write_text(json.dumps(dict(category='AUTOMATED_AUDIO_FIXTURE', library_ms=library_ms,
            cold_model_load_ms=load_ms, loaded_idle=idle, runs=rows,
            summary={s:percentiles([r['inference_ms'] for r in rows if r['signal']==s]) for s in ('tone','silence','noise')},
            wer='NOT_TESTED', live_microphone='NOT_TESTED'),indent=2)+'\n')
    finally: lib.vosk_model_free(model)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lib',type=Path,required=True);parser.add_argument('--model',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True);parser.add_argument('--runs',type=int,default=5)
    parser.add_argument('--worker',action='store_true');parser.add_argument('--corpus',type=Path)
    parser.add_argument('--phase',type=Path)
    args=parser.parse_args()
    if not 1<=args.runs<=100:parser.error('runs 1..100')
    if args.worker:
        if not args.corpus:parser.error('worker requires explicit corpus')
        worker(args);return 0
    if compilers():raise RuntimeError('Wait for compilers before benchmark')
    args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='kda-f4r-vosk-bench-') as directory:
        root=Path(directory);env=private_env(root)
        env.update(OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1')
        generate(root/'corpus',16000)
        (args.output/'corpus-manifest.json').write_text((root/'corpus/manifest.json').read_text())
        command=[sys.executable,str(Path(__file__).resolve()),'--worker','--lib',str(args.lib.resolve(strict=True)),
                 '--model',str(args.model.resolve(strict=True)),'--runs',str(args.runs),'--corpus',str(root/'corpus'),
                 '--output',str(args.output.resolve()/'benchmark.json')]
        samples=[];load=[];start=time.monotonic();own_before=time.process_time()
        with (args.output/'engine.log').open('w') as log:
            child=subprocess.Popen(command,env=env,cwd=root,stdout=log,stderr=subprocess.STDOUT)
            try:
                while child.poll() is None:
                    if compilers():raise RuntimeError('Compiler overlap invalidates comparison')
                    if time.monotonic()-start>180:raise TimeoutError('Vosk benchmark deadline')
                    try:samples.append(metrics(child.pid))
                    except (OSError,ValueError):pass
                    load.append(os.getloadavg());time.sleep(.2)
            finally:
                if child.poll() is None:child.kill();child.wait()
        assert not Path(f'/proc/{child.pid}').exists()
        (args.output/'process.json').write_text(json.dumps(dict(command=command,exit=child.returncode,
            elapsed_seconds=time.monotonic()-start,monitor_cpu_seconds=time.process_time()-own_before,
            peak_rss_kib=max((s['rss_kib'] for s in samples),default=0),peak_pss_kib=max((s['pss_kib'] for s in samples),default=0),
            cpu_ticks=max((s['cpu_ticks'] for s in samples),default=0),loadavg_min=list(map(min,zip(*load))),
            loadavg_max=list(map(max,zip(*load))),compiler_overlap=False,reaped=True),indent=2)+'\n')
        if child.returncode: return child.returncode
        # Terminate only a known benchmark child after its explicit inference marker.
        phase=root/'phase'
        cancel_command=[*command,'--phase',str(phase),'--runs','100']
        with (args.output/'cancel.log').open('w') as log:
            child=subprocess.Popen(cancel_command,env=env,cwd=root,stdout=log,stderr=subprocess.STDOUT)
            try:
                limit=time.monotonic()+15
                while not phase.exists() and child.poll() is None and time.monotonic()<limit:time.sleep(.002)
                assert phase.exists() and child.poll() is None,'No cancellable inference observed'
                start=time.monotonic();child.kill();child.wait(timeout=3)
                assert not Path(f'/proc/{child.pid}').exists()
                (args.output/'cancel.json').write_text(json.dumps(dict(category='AUTOMATED_AUDIO_FIXTURE',
                    inference_marker_observed=True,pid=child.pid,exit=child.returncode,reaped=True,
                    kill_wait_ms=(time.monotonic()-start)*1000,product_integration=False),indent=2)+'\n')
            finally:
                if child.poll() is None:child.kill();child.wait()
    return 0


if __name__=='__main__':raise SystemExit(main())
