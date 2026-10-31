#!/usr/bin/env python3
"""Offline paired Piper worker benchmarks; bounded PCM, no audio server/device."""
import argparse,hashlib,json,os,resource,selectors,struct,subprocess,sys,time
from pathlib import Path
sys.dont_write_bytecode=True
from f4_safety import private_env,require_private
from f3_runtime import metrics
from benchmark_f4 import compilers
from benchmark_vosk_f4 import percentiles

TEXTS={'short':'Hola. Soy KDE Assistant y esta es una prueba de voz en español.',
       'multi':'Hola. Esta es una prueba de voz en español. La respuesta debe conservar el inicio y el final sin cortes. Terminamos la prueba sin cambiar archivos.'}


def process(command,text,root,env,framed=False):
    require_private(root,env)
    if active := compilers():raise RuntimeError(f'Compiler overlaps inference: {active}')
    before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic()
    p=subprocess.Popen(command,env=env,cwd=root,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    select=selectors.DefaultSelector();select.register(p.stdout,selectors.EVENT_READ,'out');select.register(p.stderr,selectors.EVENT_READ,'err')
    output=bytearray();error=bytearray();samples=[];loads=[];first=None;first_pcm=None;next_sample=0;next_compilers=0;monitor_started=time.process_time()
    try:
        p.stdin.write(text.encode());p.stdin.close()
        while select.get_map():
            now=time.monotonic()
            if now-start>90:raise RuntimeError('Timeout')
            if now>=next_compilers:
                if active := compilers():raise RuntimeError(f'Compiler overlap: {active}')
                next_compilers=now+1
            for key,_ in select.select(.025):
                block=os.read(key.fileobj.fileno(),32768)
                if not block:select.unregister(key.fileobj);continue
                if key.data=='out':
                    if first is None:first=(time.monotonic()-start)*1000
                    output.extend(block)
                    if len(output)>17*1024*1024:raise RuntimeError('PCM limit')
                    if framed and first_pcm is None and len(output)>=12:
                        assert output[:8]==b'KDA4PCM1'
                        header_size=struct.unpack_from('<I',output,8)[0]
                        assert 0<header_size<=8192
                        pos=12+header_size
                        if len(output)>=pos+4:
                            size=struct.unpack_from('<I',output,pos)[0]
                            assert 0<size<=32768 and size%2==0
                            if len(output)>=pos+4+size:first_pcm=(time.monotonic()-start)*1000
                else:
                    error.extend(block)
                    if len(error)>16384:raise RuntimeError('Diagnostic limit')
            if now>=next_sample:
                try:samples.append(metrics(p.pid))
                except (OSError,ValueError):pass
                loads.append(os.getloadavg());next_sample=now+.1
        p.wait(timeout=5)
    finally:
        if p.poll() is None:p.kill();p.wait(timeout=5)
        select.close();p.stdout.close();p.stderr.close()
    usage=resource.getrusage(resource.RUSAGE_CHILDREN)
    if p.returncode:raise RuntimeError('Restricted worker failed: '+error.decode(errors='replace')[-1000:])
    detail={}
    for line in error.splitlines():
        try:row=json.loads(line)
        except ValueError:continue
        if 'pcm_bytes' in row:detail=row
    pcm=bytes(output)
    if framed:
        assert pcm[:8]==b'KDA4PCM1'
        header_size=struct.unpack_from('<I',pcm,8)[0];header=json.loads(pcm[12:12+header_size])
        assert header==dict(sample_rate=22050,channels=1,sample_width=2)
        pos=12+header_size;blocks=[]
        while True:
            size=struct.unpack_from('<I',pcm,pos)[0];pos+=4
            if not size:break
            assert 0<size<=32768 and size%2==0
            blocks.append(pcm[pos:pos+size]);pos+=size
        size=struct.unpack_from('<I',pcm,pos)[0];pos+=4
        assert 0<size<=8192 and pos+size==len(pcm)
        detail=json.loads(pcm[pos:pos+size]);pcm=b''.join(blocks)
        assert len(pcm)==detail['pcm_bytes']
    assert len(pcm)==detail['pcm_bytes'] and detail['sample_rate']==22050 and detail['channels']==1 and detail['sample_width']==2
    return dict(first_pcm_received_ms=first_pcm if framed else first,
        monitor_cpu_seconds=time.process_time()-monitor_started,wall_ms=(time.monotonic()-start)*1000,first_output_ms=first,
        cpu_seconds=usage.ru_utime+usage.ru_stime-before.ru_utime-before.ru_stime,
        rss_kib=max((s['rss_kib'] for s in samples),default=0),pss_kib=max((s['pss_kib'] for s in samples),default=0),
        bytes=len(pcm),duration_seconds=len(pcm)/44100,sha256=hashlib.sha256(pcm).hexdigest(),worker=detail,reaped=not Path(f'/proc/{p.pid}').exists(),
        loadavg_min=list(map(min,zip(*loads))),loadavg_max=list(map(max,zip(*loads))))


def main():
    p=argparse.ArgumentParser();p.add_argument('--profile',type=Path,required=True);p.add_argument('--baseline',type=Path,required=True)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--resources',type=Path,required=True)
    p.add_argument('--candidate',choices=['stream','buffered'],default='buffered');p.add_argument('--label',required=True);p.add_argument('--runs',type=int,default=5,choices=range(3,11));a=p.parse_args()
    repo=Path(__file__).resolve().parents[1]
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    root=a.profile.resolve(strict=True);env=private_env(root);require_private(root,env)
    destination=root/(a.label+'.json');records=[]
    if destination.exists():raise ValueError('Do not overwrite a benchmark')
    for run in range(a.runs):
        for sample,text in TEXTS.items():
            for mode in (['baseline',a.candidate] if run%2==0 else [a.candidate,'baseline']):
                binary=a.baseline if mode=='baseline' else a.binary
                common=[str(binary.resolve()),str(a.resources/'venv'),str(a.resources/'voice/es_ES-sharvard-medium.onnx')]
                preflight_ms=0
                if mode=='baseline':
                    if active := compilers():raise RuntimeError(f'Compilation overlap: {active}')
                    start=time.monotonic();check=subprocess.run([common[0],'--voice-piper-check',*common[1:]],env=env,cwd=root,capture_output=True,text=True,timeout=15)
                    preflight_ms=(time.monotonic()-start)*1000
                    assert check.returncode==0 and json.loads(check.stdout) is None
                row=dict(run=run,sample=sample,mode=mode,preflight_ms=preflight_ms,
                    **process([common[0],'--voice-piper-stream' if mode=='stream' else '--voice-piper-worker',*common[1:],'1','0'],text,root,env,mode=='stream'))
                assert row['bytes']>0 and row['bytes']%2==0
                # Original coordinator starts playback only after worker exit, new after first frame.
                row['request_to_playable_ms']=preflight_ms+(row['first_pcm_received_ms'] if mode=='stream' else row['wall_ms'])
                row['complete_request_ms']=preflight_ms+row['wall_ms'];records.append(row)
                destination.write_text(json.dumps(dict(records=records),indent=2)+'\n')
                print(mode,sample,run,round(row['request_to_playable_ms'],1),flush=True)
    keys=['preflight_ms','wall_ms','first_pcm_received_ms','request_to_playable_ms','complete_request_ms','cpu_seconds','monitor_cpu_seconds','rss_kib','pss_kib','duration_seconds']
    def stats(rows):
        return {key:{k.removesuffix('_ms'):v for k,v in percentiles([r[key] for r in rows]).items()} for key in keys}
    result=dict(scope='AUTOMATED_AUDIO_FIXTURE; no player/device; playable is an IPC timestamp, never acoustic onset',
        baseline_sha256=hashlib.sha256(a.baseline.read_bytes()).hexdigest(),binary_sha256=hashlib.sha256(a.binary.read_bytes()).hexdigest(),
        model='es_ES-sharvard-medium',speaker=0,scale=1,records=records,
        summaries={mode:{sample:stats([r for r in records if r['mode']==mode and r['sample']==sample]) for sample in TEXTS} for mode in ['baseline',a.candidate]})
    destination.write_text(json.dumps(result,indent=2)+'\n');print('RESULT',destination,flush=True)

if __name__=='__main__':main()
