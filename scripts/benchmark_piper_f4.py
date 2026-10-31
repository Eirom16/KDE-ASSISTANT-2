#!/usr/bin/env python3
"""Authorized private Piper evaluation; synthetic text to bounded PCM, never playback."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess
import sys
import time
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles
from f3_runtime import metrics

TEXT = 'Hola. Esta es una prueba de voz en español de KDE Assistant.'


def measured(command, root, env, name):
    require_private(root, env)
    if compilers():
        raise RuntimeError('Compiler overlap')
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    begin = time.perf_counter()
    rows, loads = [], []
    first_pcm = None
    pcm = root / (name + '.pcm')
    err = root / (name + '.stderr')
    with pcm.open('xb') as output, err.open('xb') as error:
        child = subprocess.Popen(command, cwd=root, env=env, stdin=subprocess.PIPE,
                                 stdout=output, stderr=error, umask=0o077)
        try:
            child.stdin.write(TEXT.encode()); child.stdin.close()
            while child.poll() is None:
                elapsed = time.perf_counter() - begin
                if elapsed > 90 or pcm.stat().st_size > 16*1024*1024 or compilers():
                    raise RuntimeError('Timeout, output limit, or compiler overlap')
                if first_pcm is None and pcm.stat().st_size:
                    first_pcm = elapsed*1000
                try:
                    rows.append(metrics(child.pid))
                except (OSError, ValueError):
                    pass
                loads.append(os.getloadavg())
                time.sleep(.025)
        finally:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    duration = time.perf_counter()-begin
    if child.returncode:
        raise RuntimeError('Piper failed: ' + err.read_text()[-2000:])
    data = pcm.read_bytes()
    if not data or len(data)%2 or not any(data):
        raise RuntimeError('Empty, silent or malformed PCM')
    detail = {}
    if err.read_text().strip():
        try:
            candidate = json.loads(err.read_text().splitlines()[-1])
            if isinstance(candidate, dict) and 'pcm_bytes' in candidate:
                detail = candidate
        except ValueError:
            pass
    pcm.unlink(); err.unlink()
    return dict(wall_ms=duration*1000, cpu_seconds=usage.ru_utime+usage.ru_stime-before.ru_utime-before.ru_stime,
                peak_sample_rss_kib=max((s['rss_kib'] for s in rows),default=0),
                peak_sample_pss_kib=max((s['pss_kib'] for s in rows),default=0),
                first_pcm_observed_ms=first_pcm, observation_interval_ms=25,
                pcm_bytes=len(data), generated_seconds=len(data)/44100,
                pcm_sha256=hashlib.sha256(data).hexdigest(),
                pid=child.pid,reaped=not Path(f'/proc/{child.pid}').exists(),
                loadavg_min=list(map(min,zip(*loads))),loadavg_max=list(map(max,zip(*loads))),worker_metrics=detail)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',type=Path,required=True)
    parser.add_argument('--runs',type=int,choices=range(3,11),default=3)
    parser.add_argument('--optimization-comparison',action='store_true')
    parser.add_argument('--prepack-comparison',action='store_true')
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[1];root=args.profile.resolve(strict=True)
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    env=private_env(root);require_private(root,env)
    env.update(OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1')
    os.umask(0o077)
    output=root/('bench-'+str(time.time_ns()));output.mkdir(mode=0o700)
    py=str(root/'venv/bin/python');model=str(root/'voice/es_ES-sharvard-medium.onnx')
    site=str(root/'venv/lib/python3.14/site-packages')
    source=(repo/'src/backend/piper_worker.py').read_text()
    commands={'official-cli':[py,'-I','-B','-m','piper','-m',model,'--output-raw','--speaker','0','--length-scale','1.0']}
    for threads in [1,2,3]:
        commands['worker-'+str(threads)]=[py,'-I','-B','-S','-c',source,site,model,'1.0','0',str(threads),'all']
    if args.optimization_comparison:
        commands={level: [py,'-I','-B','-S','-X','pycache_prefix=/dev/null','-c',source,site,model,'1.0','0','3',level]
                  for level in ['all','basic','disabled']}
    if args.prepack_comparison:
        commands={level: [py,'-I','-B','-S','-X','pycache_prefix=/dev/null','-c',source,site,model,'1.0','0','3',level]
                  for level in ['all','all-nopack','all-parallelpack']}
    records=[]
    for run in range(args.runs):
        for name,command in commands.items():
            row=dict(run=run,name=name,**measured(command,root,env,output.name+'/'+name+'-'+str(run)))
            records.append(row)
            (output/'records.json').write_text(json.dumps(records,indent=2)+'\n')
            print(name,run,round(row['wall_ms'],1),round(row['cpu_seconds'],3),flush=True)
    result=dict(scope='AUTOMATED_AUDIO_FIXTURE, synthetic Spanish text; NOT audible or human verified',
                text=TEXT,model_sha256=hashlib.sha256(Path(model).read_bytes()).hexdigest(),
                worker_sha256=hashlib.sha256(source.encode()).hexdigest(),records=records,
                summaries={name:percentiles([r['wall_ms'] for r in records if r['name']==name]) for name in commands},
                audio_files_deleted=True)
    (output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print('RESULT', str(output/'summary.json'), flush=True)
    print(json.dumps(result['summaries'],indent=2))


if __name__=='__main__':
    main()
