#!/usr/bin/env python3
"""Offline installed-model benchmark. Does not download, record, play or load user configuration."""
import argparse
import json
import os
from pathlib import Path
import resource
import subprocess
import tempfile
import time
import sys
sys.dont_write_bytecode=True
from f3_runtime import metrics
from voice_corpus import generate
from f4_safety import private_env

def compilers():
    active=[]
    for p in Path('/proc').iterdir():
        if p.name.isdigit():
            try:
                if (p/'comm').read_text().strip() in ('cargo','rustc','cc1','cc1plus','cmake','make','ninja','ld.lld'):
                    active.append(int(p.name))
            except OSError:pass
    return active

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--model',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--runs',type=int,default=5)
    parser.add_argument('--signal',choices=['tone','silence','noise'],default='tone')
    parser.add_argument('--profile',choices=['debug','release'],default='debug')
    parser.add_argument('--threads',type=int,choices=range(1,5),default=3)
    parser.add_argument('--audio-ctx',type=int,default=0)
    parser.add_argument('--temperature-inc',type=float,default=.2)
    parser.add_argument('--cpu-set',help='Optional CPU list for this benchmark child only')
    parser.add_argument('--rate',type=int,choices=[16000,48000],default=16000)
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[1]
    args.output.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='kda-f4-bench-') as directory:
        root=Path(directory)
        env=private_env(root)
        generate(root/'corpus',args.rate)
        (args.output/'corpus-manifest.json').write_text((root/'corpus/manifest.json').read_text())
        if compilers():raise RuntimeError('Compilers active; run benchmark separately after build')
        command=[str(repo/'target'/args.profile/'bench_voice'),str(args.model.resolve(strict=True)),str(root/f'corpus/{args.signal}.wav'),str(args.runs),str(args.output.resolve()/'benchmark.json'),str(args.threads),str(args.audio_ctx),str(args.temperature_inc)]
        if args.cpu_set:command=['/usr/bin/taskset','-c',args.cpu_set,*command]
        start=time.monotonic();samples=[];monitor_start=time.process_time()
        before=resource.getrusage(resource.RUSAGE_CHILDREN)
        observed_load=[]
        with (args.output/'engine.log').open('w') as output:
            process=subprocess.Popen(command,env=env,cwd=root,stdout=output,stderr=subprocess.STDOUT)
            try:
                while process.poll() is None:
                    if time.monotonic()-start>args.runs*180+30:raise TimeoutError('Benchmark deadline')
                    try:samples.append(metrics(process.pid))
                    except (OSError,ValueError):pass
                    if compilers():raise RuntimeError('Compiler started during inference; comparison invalid')
                    observed_load.append(os.getloadavg())
                    time.sleep(.2)
            finally:
                if process.poll() is None:process.kill();process.wait()
        usage=resource.getrusage(resource.RUSAGE_CHILDREN)
        (args.output/'process.json').write_text(json.dumps({'command':command,'exit':process.returncode,'elapsed_seconds':time.monotonic()-start,'monitor_cpu_seconds':time.process_time()-monitor_start,'cpu_user_seconds':usage.ru_utime-before.ru_utime,'cpu_system_seconds':usage.ru_stime-before.ru_stime,'maxrss_kib':usage.ru_maxrss,'sample_peak_rss_kib':max((s['rss_kib'] for s in samples),default=0),'sample_peak_pss_kib':max((s['pss_kib'] for s in samples),default=0),'loadavg_min':list(map(min,zip(*observed_load))) if observed_load else [],'loadavg_max':list(map(max,zip(*observed_load))) if observed_load else [],'affinity_requested':args.cpu_set or 'inherited','compiler_overlap':False,'scope':'synthetic signal; not speech accuracy; not hardware acceptance'},indent=2))
        return process.returncode
if __name__=='__main__':raise SystemExit(main())
