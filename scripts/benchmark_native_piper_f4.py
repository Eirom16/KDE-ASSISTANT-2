#!/usr/bin/env python3
"""Final native Piper worker: validation, first PCM, cancellation and corruption offline."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode = True
from f4_safety import private_env,require_private
from f3_runtime import metrics
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles
from benchmark_piper_f4 import measured


def cancel_after_loaded(command,root,env):
    require_private(root,env)
    if compilers():raise RuntimeError('Compiler overlaps inference')
    child=subprocess.Popen(command,cwd=root,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    selector=selectors.DefaultSelector();loaded=None;buffer=b''
    try:
        child.stdin.write(('Esta es una frase larga para cancelar la síntesis '+ 'sin ejecutar acciones '*80+'.').encode());child.stdin.close()
        selector.register(child.stderr,selectors.EVENT_READ)
        start=time.monotonic()
        while loaded is None and time.monotonic()-start<20:
            for key,_ in selector.select(.05):
                block=os.read(key.fileobj.fileno(),4096)
                if not block:raise RuntimeError('Worker ended before loading')
                buffer+=block
                if len(buffer)>8192:raise RuntimeError('Unexpected stderr length')
                for line in buffer.splitlines():
                    try:event=json.loads(line)
                    except ValueError:continue
                    if event.get('event')=='loaded':loaded=event
            if child.poll() is not None:raise RuntimeError('Worker failed before loading')
        if loaded is None:raise RuntimeError('No load marker')
        before=metrics(child.pid)
        time.sleep(.25)  # Long single sentence: cancel processing after the actual load marker.
        after=metrics(child.pid)
        begin=time.monotonic();child.kill();child.wait(timeout=5)
        return dict(loaded_event=loaded,post_load_delay_ms=250,cpu_ticks_since_loaded=after['cpu_ticks']-before['cpu_ticks'],
                    cancel_and_reap_ms=(time.monotonic()-begin)*1000,reaped=not Path(f'/proc/{child.pid}').exists(),
                    scope='Direct owned native worker, post-load processing; HTTP token routing is tested separately')
    finally:
        selector.close()
        if child.poll() is None:child.kill();child.wait(timeout=5)
        child.stdout.close();child.stderr.close()


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--profile',type=Path,required=True);p.add_argument('--runs',type=int,choices=range(3,11),default=3);a=p.parse_args()
    repo=Path(__file__).resolve().parents[1];root=a.profile.resolve(strict=True)
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    env=private_env(root);require_private(root,env);env.update(OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1')
    if compilers():raise RuntimeError('Compiler overlaps benchmark')
    output=root/('native-'+str(time.time_ns()));output.mkdir(mode=0o700);os.umask(0o077)
    binary=repo/'target/debug/kde-assistant';runtime=root/'venv';model=root/'voice/es_ES-sharvard-medium.onnx'
    check=[str(binary),'--voice-piper-check',str(runtime),str(model)]
    worker=[str(binary),'--voice-piper-worker',str(runtime),str(model),'1','0']
    rows=[]
    for run in range(a.runs):
        start=time.perf_counter();r=subprocess.run(check,env=env,cwd=root,capture_output=True,text=True,timeout=15)
        assert r.returncode==0 and json.loads(r.stdout) is None
        check_ms=(time.perf_counter()-start)*1000
        result=measured(worker,root,env,output.name+'/cold-'+str(run))
        row=dict(run=run,preflight_ms=check_ms,**result);rows.append(row)
        (output/'records.json').write_text(json.dumps(rows,indent=2)+'\n');print('native',run,round(check_ms,1),round(row['wall_ms'],1),flush=True)
    cancels=[cancel_after_loaded(worker,root,env) for _ in range(a.runs)]
    with tempfile.TemporaryDirectory(dir=root/'tmp',prefix='corrupt-voice-') as raw:
        corrupt=Path(raw)/model.name;shutil.copyfile(model,corrupt)
        with corrupt.open('r+b') as file:file.write(b'BAD!')
        result=subprocess.run([*check[:3],str(corrupt)],env=env,cwd=root,capture_output=True,text=True,timeout=15)
        assert result.returncode==0 and 'Integridad' in json.loads(result.stdout)
        corruption=dict(status='PASS',error=json.loads(result.stdout),fixture_deleted=True)
    summary=dict(category='AUTOMATED_AUDIO_FIXTURE',audio_output='NONE',model_sha256=hashlib.sha256(model.read_bytes()).hexdigest(),
                 binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),records=rows,cancellation=cancels,corrupt_model=corruption,
                 summaries={key:percentiles([r[key] for r in rows]) for key in ['preflight_ms','wall_ms','first_pcm_observed_ms']},
                 cancellation_summary=percentiles([r['cancel_and_reap_ms'] for r in cancels]),pcm_files_deleted=True,
                 cold_definition='Fresh process and ONNX session; filesystem pages may be warm. No retained model.',
                 latency_note='PCM generation only; preflight + worker.wall approximates coordinator synthesis, excludes HTTP/playback')
    (output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print('RESULT',output/'summary.json',flush=True)
    print(json.dumps(summary['summaries'],indent=2))


if __name__=='__main__':main()
