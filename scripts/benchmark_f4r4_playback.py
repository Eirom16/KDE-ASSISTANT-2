#!/usr/bin/env python3
"""VoiceCoordinator baseline/stream comparison through a private null sink only."""
import argparse,hashlib,json,os,re,subprocess,sys,time,urllib.request
from pathlib import Path
sys.dont_write_bytecode=True
from benchmark_f4 import compilers
from benchmark_f4r4 import TEXTS
from benchmark_vosk_f4 import percentiles
from f3_runtime import children,metrics,wait_for
from f4_safety import private_env,require_private


def main():
    p=argparse.ArgumentParser();p.add_argument('--profile',type=Path,required=True);p.add_argument('--resources',type=Path,required=True)
    p.add_argument('--paired',action='store_true');p.add_argument('--label',default='playback-final-rerun');p.add_argument('--baseline',type=Path,required=True);p.add_argument('--runs',type=int,choices=range(3,11),default=5);a=p.parse_args()
    repo=Path(__file__).resolve().parents[1]
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    root=a.profile.resolve(strict=True);destination=root/(a.label+'.json')
    if destination.exists():raise ValueError('Do not overwrite playback benchmark')
    records=[]
    modes={'baseline':a.baseline,'buffered':repo/'target/debug/kde-assistant'}
    schedule=[(mode,run) for run in range(a.runs) for mode in (['baseline','buffered'] if run%2==0 else ['buffered','baseline'])] if a.paired else [(mode,None) for mode in modes]
    for mode,iteration in schedule:
        binary=modes[mode]
        profile=root/(a.label+'-'+mode+('-'+str(iteration) if iteration is not None else ''));profile.mkdir(mode=0o700)
        env=private_env(profile);require_private(profile,env);app=daemon=None;observed=set()
        try:
            with (profile/'pipewire.log').open('w') as log:
                daemon=subprocess.Popen(['/usr/bin/pipewire','-c',str(repo/'tests/fixtures/f4/pipewire-tts.conf')],cwd=profile,env=env,stdout=log,stderr=subprocess.STDOUT)
            wait_for(lambda:(profile/'runtime/f4-unreachable-audio').exists(),5)
            if compilers():raise RuntimeError('Compilation overlaps benchmark')
            require_private(profile,env)
            with (profile/'backend.log').open('w') as log:
                app=subprocess.Popen([str(binary),'--ui-off','--no-shortcuts','--http-port','0'],cwd=profile,env=env,stdout=log,stderr=subprocess.STDOUT)
            wait_for(lambda:'Núcleo de texto listo en http://' in (profile/'backend.log').read_text(),20)
            url=re.findall(r'Núcleo de texto listo en (http://127.0.0.1:\d+)',(profile/'backend.log').read_text())[-1]
            token=(profile/'config/kde-assistant/server.token').read_text().strip()
            def api(path,body=None):
                req=urllib.request.Request(url+path,data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
                with urllib.request.urlopen(req,timeout=5) as response:return json.load(response)
            cfg=api('/api/config');cfg['speech'].update(piper_runtime=str(a.resources/'venv'),piper_model=str(a.resources/'voice/es_ES-sharvard-medium.onnx'),piper_speaker=0,piper_length_scale=1,auto_speak=False,auto_listen=False,wake_word_enabled=False,chimes_enabled=False)
            cfg['memory'].update(enabled=False,auto_summarize=False);cfg['ai'].update(api_key='',provider_keys={},base_url='http://127.0.0.1:1/v1',enable_tool_calling=False)
            api('/api/config',cfg);sid=api('/api/session',{'title':'Synthetic offline latency'})['id']
            scope=api('/api/voice/session',{'session_id':sid})['scope']
            def cpu_ticks():
                fields=Path(f'/proc/{app.pid}/stat').read_text().rsplit(') ',1)[1].split()
                return sum(int(fields[i]) for i in [11,12,13,14])
            for run in ([iteration] if iteration is not None else range(a.runs)):
                if compilers():raise RuntimeError('Compilation overlaps inference')
                scope=api('/api/voice/session',{'session_id':sid})['scope']
                before_cpu=cpu_ticks();start=time.monotonic();next_check=0;first=None;linked=None;peak_rss=peak_pss=0;op_pids=set();timeline=[];loads=[]
                api('/api/speak',dict(session_id=sid,scope=scope,text=TEXTS['short']))
                while True:
                    now=time.monotonic();loads.append(os.getloadavg())
                    if now-start>50:raise TimeoutError('Null playback timeout')
                    if now>=next_check:
                        if compilers():raise RuntimeError('Compilation started')
                        next_check=now+1
                    status=api('/api/voice/status');state=status['state']
                    if not timeline or timeline[-1]['state']!=state:timeline.append(dict(state=state,ms=(now-start)*1000))
                    if state=='error':raise RuntimeError(status['error'])
                    pids=children(app.pid);observed.update(pids);op_pids.update(pids);samples=[]
                    for pid in pids:
                        try:samples.append(metrics(pid))
                        except (OSError,ValueError):pass
                    peak_rss=max(peak_rss,sum(s['rss_kib'] for s in samples));peak_pss=max(peak_pss,sum(s['pss_kib'] for s in samples))
                    if state=='speaking' and first is None:first=(time.monotonic()-start)*1000
                    if first is not None and linked is None:
                        nodes=json.loads(subprocess.check_output(['/usr/bin/pw-dump'],env=env,timeout=3))
                        playback=next((n for n in nodes if n.get('info',{}).get('props',{}).get('media.category')=='Playback'),None)
                        if playback:
                            subprocess.run(['/usr/bin/pw-cli','set-param',str(playback['id']),'PortConfig','{ direction = Output mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 22050 channels = 1 position = [ MONO ] } }'],env=env,capture_output=True,check=True,timeout=3)
                            ports=subprocess.check_output(['/usr/bin/pw-link','-o'],env=env,text=True,timeout=3).splitlines()
                            port=next((p.strip() for p in ports if p.strip().endswith(':output_MONO')),None)
                            if port:
                                subprocess.run(['/usr/bin/pw-link',port,'f4-tts-sink:playback_MONO'],env=env,capture_output=True,check=True,timeout=3)
                                linked=(time.monotonic()-start)*1000
                    if state=='idle' and first is not None:break
                    time.sleep(.1)
                wait_for(lambda:not children(app.pid),5)
                assert first is not None and linked is not None and all(not Path(f'/proc/{pid}').exists() for pid in op_pids)
                trace=None
                if mode=='buffered':
                    lines=[line.split('voice_tts_timing ',1)[1] for line in (profile/'backend.log').read_text().splitlines() if 'voice_tts_timing ' in line]
                    assert len(lines)==(1 if iteration is not None else run+1);trace=json.loads(lines[-1])
                row=dict(mode=mode,run=run,speaking_observed_ms=first,null_link_ms=linked,complete_ms=(time.monotonic()-start)*1000,
                    cpu_seconds=(cpu_ticks()-before_cpu)/os.sysconf('SC_CLK_TCK'),peak_children_rss_kib=peak_rss,peak_children_pss_kib=peak_pss,
                    timeline=timeline,trace=trace,all_children_reaped=True,loadavg_min=list(map(min,zip(*loads))),loadavg_max=list(map(max,zip(*loads))))
                records.append(row);destination.write_text(json.dumps(dict(records=records),indent=2)+'\n');print(mode,run,round(first,1),flush=True)
            assert api('/api/messages?session_id='+sid)==[]
        finally:
            for child in [app,daemon]:
                if child:
                    if child.poll() is None:child.terminate()
                    child.wait(timeout=10)
            assert all(not Path(f'/proc/{pid}').exists() for pid in observed)
    def stats(v):return {k.removesuffix('_ms'):x for k,x in percentiles(v).items()}
    summaries={mode:{k:stats([r[k] for r in records if r['mode']==mode]) for k in ['speaking_observed_ms','null_link_ms','complete_ms','cpu_seconds','peak_children_rss_kib','peak_children_pss_kib']} for mode in ['baseline','buffered']}
    destination.write_text(json.dumps(dict(category='AUTOMATED_AUDIO_FIXTURE',audible='NOT_TESTED',records=records,summaries=summaries,
        order='alternating pairs with new isolated backend per operation' if a.paired else 'baseline 5 then buffered 5; temporal batch-order confound'),indent=2)+'\n');print('RESULT',destination,flush=True)

if __name__=='__main__':main()
