#!/usr/bin/env python3
"""Prepared Piper integration: private profiles, loopback mock, private null sink only."""
import argparse, hashlib, json, os, re, subprocess, sys, threading, time, urllib.request
from pathlib import Path
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private
from f3_runtime import wait_for, children, metrics
from benchmark_f4 import compilers
from benchmark_f4r4 import TEXTS

REPO = Path(__file__).resolve().parents[1]
BASE = '9544a58eba4b6adf6c6343c04db80813b084f76b'

def checked():
    assert subprocess.check_output(['git','branch','--show-current'],cwd=REPO,text=True).strip() == 'recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=REPO,text=True).strip() == BASE
    if compilers(): raise RuntimeError('Compilation overlaps inference')

class Scenario:
    def __init__(self, root, resources, binary=None, qml=False):
        checked(); os.umask(0o077)
        if len(os.fsencode(root/'runtime/f4-unreachable-audio-manager')) >= 108:
            raise ValueError('Private PipeWire socket path exceeds AF_UNIX limit; choose a shorter private root')
        self.root=root; root.mkdir(mode=0o700)
        self.env=private_env(root); require_private(root,self.env)
        if qml:self.env['QT_QPA_PLATFORM']='offscreen'
        self.resources=resources; self.app=self.daemon=None; self.observed=set(); self.calls=[]
        self.delay=0; self.final_at=None; self.first_token_at=None; self.cancel_stream=False
        scenario=self
        class Mock(BaseHTTPRequestHandler):
            def log_message(self,*args): pass
            def do_POST(self):
                body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                assert self.path=='/v1/chat/completions' and 'tools' not in body
                scenario.calls.append({'messages':len(body['messages'])})
                self.send_response(200); self.send_header('Content-Type','text/event-stream'); self.send_header('Connection','close'); self.end_headers()
                try:
                    self.wfile.write(b'data: {"choices":[{"delta":{"content":"Hola. "},"finish_reason":null}]}\n\n'); self.wfile.flush(); scenario.first_token_at=time.monotonic()
                    time.sleep(scenario.delay)
                    scenario.final_at=time.monotonic()
                    self.wfile.write(b'data: {"choices":[{"delta":{"content":"Soy KDE Assistant y esta es una prueba de voz en espa\\u00f1ol."},"finish_reason":null}]}\n\ndata: {"choices":[{"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'); self.wfile.flush()
                except (BrokenPipeError,ConnectionResetError): pass
        self.mock=ThreadingHTTPServer(('127.0.0.1',0),Mock)
        self.thread=threading.Thread(target=self.mock.serve_forever,daemon=True);self.thread.start()
        try:
            with (root/'pipewire.log').open('w') as log:
                self.daemon=subprocess.Popen(['/usr/bin/pipewire','-c',str(REPO/'tests/fixtures/f4/pipewire-tts.conf')],cwd=root,env=self.env,stdout=log,stderr=subprocess.STDOUT)
            wait_for(lambda:(root/'runtime/f4-unreachable-audio').exists(),5)
            require_private(root,self.env)
            with (root/'backend.log').open('w') as log:
                self.app=subprocess.Popen([str(binary or REPO/'target/debug/kde-assistant'),*([] if qml else ['--ui-off']),'--no-shortcuts','--http-port','0'],cwd=root,env=self.env,stdout=log,stderr=subprocess.STDOUT)
            wait_for(lambda:'Núcleo de texto listo en http://' in (root/'backend.log').read_text(),20)
            self.url=re.findall(r'Núcleo de texto listo en (http://127.0.0.1:\d+)',(root/'backend.log').read_text())[-1]
            self.token=(root/'config/kde-assistant/server.token').read_text().strip()
            self.sid=self.api('/api/session',{'title':'R5 synthetic null sink'})['id']
            self.sid2=self.api('/api/session',{'title':'R5 second synthetic session'})['id']
            self.configure(False)
            if qml:
                # Match Main.qml's initial most-recent selection; do not race the UI
                # with a different API-selected conversation during its startup.
                self.sid,self.sid2=self.sid2,self.sid
                scope=self.select();stable=time.monotonic();deadline=stable+20
                while time.monotonic()-stable<3:
                    if time.monotonic()>deadline:raise TimeoutError('QML session did not stabilize')
                    time.sleep(.2);current=self.select()
                    if current!=scope:scope=current;stable=time.monotonic()
        except BaseException:
            self.close(); raise
    def api(self,path,body=None,timeout=20):
        req=urllib.request.Request(self.url+path,data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'Bearer '+self.token,'Content-Type':'application/json'})
        with urllib.request.urlopen(req,timeout=timeout) as response: return json.load(response)
    def status(self):
        self.observed.update(children(self.app.pid))
        return self.api('/api/voice/status')
    def configure(self,fast=True,ttl=30,auto=False,**extra):
        cfg=self.api('/api/config')
        cfg['speech'].update(piper_runtime=str(self.resources/'venv'),piper_model=str(self.resources/'voice/es_ES-sharvard-medium.onnx'),piper_speaker=0,piper_length_scale=1,auto_speak=auto,auto_listen=False,wake_word_enabled=False,chimes_enabled=False,piper_fast=fast,piper_fast_ttl_secs=ttl)
        cfg['speech'].update(extra)
        cfg['memory'].update(enabled=False,auto_summarize=False)
        cfg['ai'].update(provider='custom',base_url=f'http://127.0.0.1:{self.mock.server_port}/v1',model='synthetic',api_key='synthetic',provider_keys={},provider_base_urls={},provider_models={},enable_tool_calling=False)
        self.api('/api/config',cfg)
    def select(self,sid=None): return self.api('/api/voice/session',{'session_id':sid or self.sid})['scope']
    def speak(self,text=None):
        scope=self.select()
        return self.api('/api/speak',{'session_id':self.sid,'scope':scope,'text':text or TEXTS['short']})
    def link(self):
        nodes=json.loads(subprocess.check_output(['/usr/bin/pw-dump'],env=self.env,timeout=3))
        playback=next((n for n in nodes if n.get('info',{}).get('props',{}).get('media.category')=='Playback'),None)
        if not playback:return False
        subprocess.run(['/usr/bin/pw-cli','set-param',str(playback['id']),'PortConfig','{ direction = Output mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 22050 channels = 1 position = [ MONO ] } }'],env=self.env,capture_output=True,check=True,timeout=3)
        ports=subprocess.check_output(['/usr/bin/pw-link','-o'],env=self.env,text=True,timeout=3).splitlines()
        port=next((p.strip() for p in ports if p.strip().endswith(':output_MONO')),None)
        if not port:return False
        subprocess.run(['/usr/bin/pw-link',port,'f4-tts-sink:playback_MONO'],env=self.env,capture_output=True,check=True,timeout=3)
        return True
    def cpu(self):
        fields=Path(f'/proc/{self.app.pid}/stat').read_text().rsplit(') ',1)[1].split()
        total=sum(int(fields[i]) for i in [11,12,13,14])
        for pid in children(self.app.pid):
            try:total+=metrics(pid)['cpu_ticks']
            except (OSError,ValueError):pass
        return total/os.sysconf('SC_CLK_TCK')
    def render(self,label,run=0,chat=False,delay=0):
        checked(); start=time.monotonic(); cpu=self.cpu(); peakrss=peakpss=backendrss=backendpss=workerrss=workerpss=0; speaking=linked=None; timeline=[]; nextcheck=0; persisted_before=None
        result={}; error=[]; self.final_at=None
        def ask():
            try:
                if chat:
                    self.delay=delay; scope=self.select()
                    result['response']=self.api('/api/chat/complete',{'session_id':self.sid,'message':'Mensaje sintético de prueba','voice_scope':scope},timeout=max(20,delay+15))
                else:result['response']=self.speak()
                result['response_at']=time.monotonic()
            except BaseException as e:error.append(e)
        thread=threading.Thread(target=ask);thread.start()
        while True:
            now=time.monotonic()
            if now-start>55:raise TimeoutError('Null playback timeout '+str(self.status()))
            if now>=nextcheck:checked();nextcheck=now+1
            if chat and 'response_at' in result and persisted_before is None:
                persisted_before=hashlib.sha256(json.dumps(self.api('/api/messages?session_id='+self.sid),sort_keys=True).encode()).hexdigest()
            state=self.status(); fast=state.get('fast',{})
            marker=(state['state'],fast.get('state'),fast.get('phase'))
            if not timeline or timeline[-1]['state']!=list(marker):timeline.append({'state':list(marker),'ms':(now-start)*1000})
            if state['state']=='error':raise RuntimeError(state['error'])
            backend=metrics(self.app.pid);backendrss=max(backendrss,backend['rss_kib']);backendpss=max(backendpss,backend['pss_kib'])
            samples=[]
            for pid in children(self.app.pid):
                try:samples.append(metrics(pid))
                except (OSError,ValueError):pass
            if fast.get('worker_pid'):
                try:
                    m=metrics(fast['worker_pid']);workerrss=max(workerrss,m['rss_kib']);workerpss=max(workerpss,m['pss_kib'])
                except (OSError,ValueError):pass
            peakrss=max(peakrss,sum(s['rss_kib'] for s in samples));peakpss=max(peakpss,sum(s['pss_kib'] for s in samples))
            if state['state']=='speaking' and speaking is None:speaking=(time.monotonic()-start)*1000
            if speaking is not None and linked is None and self.link():linked=(time.monotonic()-start)*1000
            if speaking is not None and state['state']=='idle':break
            if error:raise error[0]
            time.sleep(.08)
        thread.join(2); assert not thread.is_alive() and not error and linked is not None
        lines=[line.split('voice_tts_timing ',1)[1] for line in (self.root/'backend.log').read_text().splitlines() if 'voice_tts_timing ' in line]
        trace=json.loads(lines[-1])
        if chat:
            after=hashlib.sha256(json.dumps(self.api('/api/messages?session_id='+self.sid),sort_keys=True).encode()).hexdigest()
            assert persisted_before is not None and persisted_before==after
        return dict(persisted_text_unchanged=True if chat else None,label=label,run=run,speaking_ms=speaking,null_link_ms=linked,complete_ms=(time.monotonic()-start)*1000,cpu_seconds=self.cpu()-cpu,peak_children_rss_kib=peakrss,peak_children_pss_kib=peakpss,backend_peak_rss_kib=backendrss,backend_peak_pss_kib=backendpss,worker_peak_rss_kib=workerrss,worker_peak_pss_kib=workerpss,loadavg=os.getloadavg(),memavailable_kib=int(re.search(r"MemAvailable:\s+(\d+)",Path("/proc/meminfo").read_text())[1]),psi=Path("/proc/pressure/memory").read_text(),timeline=timeline,fast=self.status().get('fast'),trace=trace,final_response_available_ms=(result['response_at']-start)*1000 if chat else None,response_to_speaking_ms=start*1000+speaking-result['response_at']*1000 if chat else None,final_text_ms=None if self.final_at is None else (self.final_at-start)*1000,final_to_speaking_ms=None if self.final_at is None else start*1000+speaking-self.final_at*1000)
    def close(self):
        for child in [self.app,self.daemon]:
            if child:
                self.observed.update(children(child.pid))
                if child.poll() is None:child.terminate()
                child.wait(timeout=15)
        self.mock.shutdown(); self.mock.server_close(); self.thread.join(2)
        wait_for(lambda:all(not Path(f'/proc/{pid}').exists() for pid in self.observed),5)


def lifecycle(s, destination):
    import signal, shutil
    rows=[]
    def record(name,start=None,**extra):
        rows.append(dict(test=name,status='PASS',ms=None if start is None else (time.monotonic()-start)*1000,**extra))
        destination.write_text(json.dumps({'category':'AUTOMATED_AUDIO_FIXTURE','audible':'NOT_TESTED','results':rows},indent=2)+'\n')
        print('PASS',name,flush=True)
    def empty(): return not children(s.app.pid)
    for name,delay in [('disable_preparing',.5),('disable_ready',None)]:
        s.configure(True); snap=s.speak()
        if delay is None: wait_for(lambda:s.status()['fast']['state']=='ready',30)
        else: time.sleep(delay)
        pids=children(s.app.pid); start=time.monotonic();s.configure(False);wait_for(empty,5)
        assert s.status()['state']=='idle' and s.status()['fast']['state']=='disabled'
        assert all(not Path(f'/proc/{pid}').exists() for pid in pids)
        record(name,start)
    s.configure(True)
    snap=s.speak('Esta frase sintética contiene muchas palabras para comprobar que una síntesis larga se interrumpe durante la inferencia sin producir audio obsoleto y sin bloquear la conversación escrita. '*5)
    wait_for(lambda:s.status()['fast'].get('phase')=='inference',30)
    pid=s.status()['fast']['worker_pid'];start=time.monotonic()
    s.api('/api/voice/cancel',snap['identity']);wait_for(empty,5)
    assert not Path(f'/proc/{pid}').exists(); record('cancel_actual_inference',start)
    # Recover and exercise completed buffer playback, then revoke by session change.
    s.render('recover_after_cancel');pid=s.status()['fast']['worker_pid']
    snap=s.speak();wait_for(lambda:s.status()['state']=='speaking',15);wait_for(s.link,5)
    start=time.monotonic();s.select(s.sid2);wait_for(empty,5)
    assert not Path(f'/proc/{pid}').exists() and s.status()['state']=='idle'
    record('session_change_during_playback',start)
    s.select();s.render('worker_fault_setup');pid=s.status()['fast']['worker_pid']
    os.kill(pid,signal.SIGKILL);wait_for(lambda:s.status()['fast']['worker_pid'] is None,5);wait_for(empty,5)
    record('unexpected_worker_death_reaped')
    s.render('after_worker_death');pid=s.status()['fast']['worker_pid'];os.kill(pid,signal.SIGSTOP)
    snap=s.speak();time.sleep(.2);start=time.monotonic();s.api('/api/voice/cancel',snap['identity']);wait_for(empty,5)
    assert not Path(f'/proc/{pid}').exists();record('stalled_worker_cancel_kill_wait',start)
    # A streaming LLM may prepare ONNX, but cannot synthesize partial tokens.
    s.configure(True,auto=True);s.delay=10;s.select();answer={}
    def ask():
        try:answer['value']=s.api('/api/chat/complete',{'session_id':s.sid,'message':'Mensaje sintético cancelable'})
        except urllib.error.HTTPError as error:
            answer['http_status']=error.code;answer['terminal']=json.loads(error.read())
    t=threading.Thread(target=ask);t.start()
    wait_for(lambda:s.status()['fast']['state']=='preparing',5)
    assert s.status()['state']=='idle' and s.status()['fast']['state'] in ['preparing','ready']
    turn=s.api('/api/chat/turn?session_id='+s.sid)
    start=time.monotonic();s.api('/api/chat/cancel',{'session_id':s.sid,'turn_id':turn['turn_id']});wait_for(empty,5);t.join(15)
    assert not t.is_alive() and s.status()['state']=='idle' and answer.get('http_status')==409
    record('llm_cancel_during_preparation_no_partial_audio',start)
    s.configure(True);s.render('voice_change_setup');pid=s.status()['fast']['worker_pid']
    start=time.monotonic();s.configure(True,piper_speaker=1);wait_for(empty,5)
    assert not Path(f'/proc/{pid}').exists();record('speaker_change_revokes',start)
    # Only a PRIVATE COPY is mutated; the installed authorized voice is untouched.
    voice=s.root/'voice-copy';voice.mkdir(mode=0o700)
    model=voice/'es_ES-sharvard-medium.onnx'
    shutil.copyfile(s.resources/'voice/es_ES-sharvard-medium.onnx',model)
    shutil.copyfile(s.resources/'voice/es_ES-sharvard-medium.onnx.json',str(model)+'.json')
    os.chmod(model,0o600);os.chmod(str(model)+'.json',0o600)
    s.configure(True,piper_model=str(model));s.render('copy_model')
    pid=s.status()['fast']['worker_pid'];start=time.monotonic()
    with open(str(model)+'.json','ab') as f:f.write(b' ')
    wait_for(empty,5);assert not Path(f'/proc/{pid}').exists();record('resource_change_inotify_revokes_idle',start)
    Path(str(model)+'.json').write_bytes((s.resources/'voice/es_ES-sharvard-medium.onnx.json').read_bytes())
    s.render('resource_recovered_first_request')
    record('first_new_request_after_health_revocation_recovers')
    s.configure(True,piper_model='/nonexistent/model.onnx')
    s.speak();wait_for(lambda:s.status()['state']=='error',10);wait_for(empty,5)
    record('missing_model_no_orphan')
    for i in range(4):s.configure(True);s.configure(False);assert empty()
    record('repeated_toggle_no_preload')
    s.configure(False,auto=False);s.delay=0;s.select()
    before=len(s.calls);s.api('/api/chat/complete',{'session_id':s.sid,'message':'Texto independiente de voz'})
    assert len(s.calls)==before+1 and empty();record('text_works_with_voice_disabled')
    s.configure(True);s.speak();time.sleep(1.5);pids=children(s.app.pid);start=time.monotonic()
    s.app.terminate();s.app.wait(timeout=10)
    wait_for(lambda:all(not Path(f'/proc/{pid}').exists() for pid in pids),5)
    record('application_shutdown_during_load',start,children_reaped=len(pids))
    return rows


def ttl_trial(s, ttl, destination):
    s.configure(True,ttl=ttl);s.render('ttl_setup_'+str(ttl))
    pid=s.status()['fast']['worker_pid'];before=metrics(pid);start=time.monotonic();samples=[];warm_fds=len(list(Path(f'/proc/{s.app.pid}/fd').iterdir()))
    # Continuous status polling must NOT renew the lease.
    while Path(f'/proc/{pid}').exists():
        if time.monotonic()-start>ttl+5:raise TimeoutError('TTL renewed by status')
        s.status()
        try:samples.append(metrics(pid))
        except (OSError,ValueError):pass
        time.sleep(.25)
    after=s.status();assert after['fast']['state']=='cold' and not children(s.app.pid)
    row=dict(ttl_secs=ttl,remaining_after_playback_s=time.monotonic()-start,ready_rss_kib=before['rss_kib'],ready_pss_kib=before['pss_kib'],idle_cpu_ticks=(samples[-1]['cpu_ticks']-samples[0]['cpu_ticks']) if len(samples)>1 else None,samples=len(samples),all_reaped=True,warm_backend_fds=warm_fds,cold_backend_fds=len(list(Path(f'/proc/{s.app.pid}/fd').iterdir())),backend=metrics(s.app.pid),meminfo=Path('/proc/meminfo').read_text(),psi=Path('/proc/pressure/memory').read_text())
    existing=json.loads(destination.read_text()) if destination.exists() else []
    existing.append(row);destination.write_text(json.dumps(existing,indent=2)+'\n');print('TTL',ttl,row['remaining_after_playback_s'],flush=True)
    row['after_expiry']=s.render('after_expiry_'+str(ttl))
    destination.write_text(json.dumps(existing,indent=2)+'\n')
    s.configure(False)
    return row


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--profile',type=Path,required=True);parser.add_argument('--resources',type=Path,required=True);parser.add_argument('--smoke',action='store_true');parser.add_argument('--lifecycle',action='store_true');parser.add_argument('--ttl',type=int,choices=[15,30,60]);parser.add_argument('--mock',action='store_true');parser.add_argument('--cold',action='store_true');parser.add_argument('--binary',type=Path);parser.add_argument('--qml-coexist',action='store_true');parser.add_argument('--runs',type=int,default=5)
    a=parser.parse_args();root=a.profile.resolve(strict=True);resources=a.resources.resolve(strict=True)
    s=Scenario(root/('r5-'+str(time.time_ns())),resources,a.binary,a.qml_coexist);records=[]
    label='lifecycle' if a.lifecycle else 'ttl-'+str(a.ttl) if a.ttl else 'mock-cold' if a.mock and a.cold else 'mock' if a.mock else 'cold' if a.cold else 'smoke' if a.smoke else 'integration'
    destination=root/('r5-'+label+'-'+str(time.time_ns())+'.json')
    try:
        if a.lifecycle:
            lifecycle(s,destination);return
        if a.ttl:
            ttl_trial(s,a.ttl,root/'r5-ttl.json');return
        if a.mock:
            s.configure(not a.cold,auto=True)
            for i in range(a.runs):
                s.configure(not a.cold,auto=True)
                row=s.render('mock_stream',i,chat=True,delay=9);records.append(row)
                print('MOCK',i,row['final_to_speaking_ms'],flush=True)
                destination.write_text(json.dumps({'records':records,'calls':s.calls},indent=2)+'\n')
            assert len(s.calls)==a.runs;return
        if a.cold:
            for i in range(a.runs):
                row=s.render('cold',i);records.append(row)
                destination.write_text(json.dumps({'records':records},indent=2)+'\n');print('COLD',i,row['speaking_ms'],flush=True)
            return
        assert s.status()['fast']['state']=='disabled' and (a.qml_coexist or not children(s.app.pid))
        s.configure(True); assert s.status()['fast']['state']=='cold' and (a.qml_coexist or not children(s.app.pid))
        for i in range(1 if a.smoke else a.runs):
            s.configure(True)
            for label in ['first','ready','consecutive']:
                row=s.render(label,i);records.append(row);print(label,i,round(row['speaking_ms'],1),flush=True)
                destination.write_text(json.dumps({'records':records},indent=2)+'\n')
                assert s.status()['fast']['state']=='ready', s.status()['fast']
        pid=s.status()['fast']['worker_pid'];s.configure(False)
        assert not Path(f'/proc/{pid}').exists() and (a.qml_coexist or not children(s.app.pid))
        assert not s.calls and s.api('/api/messages?session_id='+s.sid)==[]
        destination.write_text(json.dumps({'category':'AUTOMATED_AUDIO_FIXTURE','audible':'NOT_TESTED','records':records,'disable_reaped':True,'profile':str(s.root)},indent=2)+'\n')
    except BaseException as error:
        (root/('failure-'+str(time.time_ns())+'.json')).write_text(json.dumps(dict(error=str(error),status=s.status() if s.app.poll() is None else {'backend_exit':s.app.returncode},meminfo=Path('/proc/meminfo').read_text(),psi=Path('/proc/pressure/memory').read_text()),indent=2)+'\n')
        raise
    finally:s.close()
    print('RESULT',destination,flush=True)
if __name__=='__main__': main()
