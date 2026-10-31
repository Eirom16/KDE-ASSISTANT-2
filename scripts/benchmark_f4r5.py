#!/usr/bin/env python3
"""Alternated, integrated R4-cold/R5-prepared comparison; private null sink only."""
import argparse,json,os,sys,time
from pathlib import Path
sys.dont_write_bytecode=True
from validate_f4_r5 import Scenario,checked
from f3_runtime import metrics


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--profile',type=Path,required=True);p.add_argument('--resources',type=Path,required=True);p.add_argument('--baseline',type=Path,required=True);p.add_argument('--runs',type=int,choices=range(5,11),default=5);a=p.parse_args()
    root=a.profile.resolve(strict=True);resources=a.resources.resolve(strict=True);baseline=a.baseline.resolve(strict=True)
    destination=root/('paired-r5-'+str(time.time_ns())+'.json');rows=[]
    for i in range(a.runs):
        for mode in (['r4-cold','r5-fast'] if i%2==0 else ['r5-fast','r4-cold']):
            checked();s=Scenario(root/(mode+'-'+str(time.time_ns())),resources,baseline if mode=='r4-cold' else None)
            try:
                s.configure(mode=='r5-fast')
                for label in (['cold'] if mode=='r4-cold' else ['first','ready','consecutive']):
                    row=s.render(mode+'-'+label,i);row['mode']=mode
                    row['thermal']={str(path):path.read_text().strip() for path in Path('/sys/class/thermal').glob('thermal_zone*/temp') if path.is_file()}
                    rows.append(row);destination.write_text(json.dumps({'records':rows,'audible':'NOT_TESTED','category':'AUTOMATED_AUDIO_FIXTURE'},indent=2)+'\n')
                    print(mode,label,i,round(row['speaking_ms']),flush=True)
                if mode=='r5-fast':
                    pid=s.status()['fast']['worker_pid'];assert pid
                    before=metrics(pid);backend_before=metrics(s.app.pid);start=time.monotonic();time.sleep(7)
                    after=metrics(pid);backend_after=metrics(s.app.pid)
                    row['quiet_idle']=dict(seconds=time.monotonic()-start,worker_cpu_seconds=(after['cpu_ticks']-before['cpu_ticks'])/os.sysconf('SC_CLK_TCK'),backend_cpu_seconds=(backend_after['cpu_ticks']-backend_before['cpu_ticks'])/os.sysconf('SC_CLK_TCK'),rss_kib=after['rss_kib'],pss_kib=after['pss_kib'])
                    s.configure(False);assert not Path(f'/proc/{pid}').exists()
                    destination.write_text(json.dumps({'records':rows,'audible':'NOT_TESTED','category':'AUTOMATED_AUDIO_FIXTURE'},indent=2)+'\n')
            except BaseException as error:
                destination.with_suffix('.failure.json').write_text(json.dumps(dict(error=str(error),status=s.status(),meminfo=Path('/proc/meminfo').read_text(),psi=Path('/proc/pressure/memory').read_text()),indent=2)+'\n')
                raise
            finally:s.close()
    print('RESULT',destination,flush=True)
if __name__=='__main__':main()
