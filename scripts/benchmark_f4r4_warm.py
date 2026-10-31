#!/usr/bin/env python3
"""Offline cost-only experiment. One Rust owner, two syntheses, fixed 3s idle TTL."""
import argparse,json,os,resource,signal,subprocess,sys,time
from pathlib import Path
sys.dont_write_bytecode=True
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles
from f3_runtime import children,metrics
from f4_safety import private_env,require_private


def main():
    p=argparse.ArgumentParser();p.add_argument('--profile',type=Path,required=True);p.add_argument('--resources',type=Path,required=True)
    p.add_argument('--label',default='warm-experiment-final');p.add_argument('--runs',type=int,choices=range(3,11),default=5);a=p.parse_args()
    repo=Path(__file__).resolve().parents[1]
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    root=a.profile.resolve(strict=True);env=private_env(root);require_private(root,env)
    env['KDA_WARM_EXPERIMENT_ROOT']=str(root)
    output=root/(a.label+'.json')
    if output.exists():raise ValueError('Do not overwrite experiment')
    rows=[]
    command=[str(repo/'target/debug/bench_piper_warm'),str(repo/'target/debug/kde-assistant'),str(a.resources/'venv'),str(a.resources/'voice/es_ES-sharvard-medium.onnx')]
    for run in range(a.runs):
        if compilers():raise RuntimeError('Compiler overlaps inference')
        usage=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();samples=[];observed=set();last_check=0
        with (root/f'{a.label}-{run}.out').open('w') as out,(root/f'{a.label}-{run}.err').open('w') as err:
            app=subprocess.Popen(command,cwd=root,env=env,stdout=out,stderr=err)
            try:
                while app.poll() is None:
                    now=time.monotonic()
                    if now-start>65:raise TimeoutError('Prepared experiment limit')
                    if now>=last_check:
                        if compilers():raise RuntimeError('Compiler started during inference')
                        last_check=now+1
                    for pid in children(app.pid):
                        observed.add(pid)
                        try:samples.append(metrics(pid))
                        except (ValueError,OSError):pass
                    time.sleep(.2)
            finally:
                if app.poll() is None:
                    app.send_signal(signal.SIGINT)
                    try:app.wait(timeout=5)
                    except subprocess.TimeoutExpired:app.kill();app.wait()
        assert app.returncode==0,(root/f'{a.label}-{run}.err').read_text()
        result=json.loads((root/f'{a.label}-{run}.out').read_text());after=resource.getrusage(resource.RUSAGE_CHILDREN)
        result.update(run=run,cpu_seconds=after.ru_utime+after.ru_stime-usage.ru_utime-usage.ru_stime,
            rss_peak_kib=max((s['rss_kib'] for s in samples),default=0),pss_peak_kib=max((s['pss_kib'] for s in samples),default=0),
            all_reaped=all(not Path(f'/proc/{pid}').exists() for pid in observed))
        assert result['reaped'] and result['all_reaped']
        rows.append(result);output.write_text(json.dumps(dict(records=rows),indent=2)+'\n')
        print('warm',run,[round(x['first_pcm_ms'],1) for x in result['rows']],flush=True)
    def stats(v):return {k.removesuffix('_ms'):x for k,x in percentiles(v).items()}
    summaries={str(i):{k:stats([r['rows'][i][k] for r in rows]) for k in ['first_pcm_ms','complete_ms']} for i in range(2)}
    output.write_text(json.dumps(dict(category='AUTOMATED_AUDIO_FIXTURE',resident_default=False,records=rows,summaries=summaries,
        cpu=stats([r['cpu_seconds'] for r in rows]),rss=stats([r['rss_peak_kib'] for r in rows]),pss=stats([r['pss_peak_kib'] for r in rows])),indent=2)+'\n')
    print('RESULT',output,flush=True)

if __name__=='__main__':main()
