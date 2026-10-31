#!/usr/bin/env python3
"""Real Piper through VoiceCoordinator, private null PipeWire sink; never audible."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private
from f3_runtime import wait_for, children, sample
from benchmark_f4 import compilers


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',type=Path,required=True)
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[1];root=args.profile.resolve(strict=True)
    assert subprocess.check_output(['git','branch','--show-current'],cwd=repo,text=True).strip()=='recovery/f4-voice-coordinator'
    assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()=='9544a58eba4b6adf6c6343c04db80813b084f76b'
    if compilers(): raise RuntimeError('Compilation overlaps inference')
    os.umask(0o077)
    # Separate synthetic SQLite/config/profile from package installation and any live trial.
    profile=root/('coordinator-'+str(time.time_ns()));profile.mkdir(mode=0o700)
    env=private_env(profile);require_private(profile,env)
    calls=[];observed=[];app=None;daemon=None;mock=None
    class Mock(BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            assert self.path=='/v1/chat/completions' and 'tools' not in body
            calls.append({'path':self.path,'messages':len(body['messages'])})
            data=b'data: {"choices":[{"delta":{"content":"Hola. La respuesta de prueba termina aqui."},"finish_reason":null}]}\n\ndata: {"choices":[{"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    mock=ThreadingHTTPServer(('127.0.0.1',0),Mock);thread=threading.Thread(target=mock.serve_forever,daemon=True);thread.start()
    try:
        with (profile/'pipewire.log').open('w') as output:
            daemon=subprocess.Popen(['/usr/bin/pipewire','-c',str(repo/'tests/fixtures/f4/pipewire-tts.conf')],cwd=profile,env=env,stdout=output,stderr=subprocess.STDOUT)
        wait_for(lambda:(profile/'runtime/f4-unreachable-audio').exists(),5)
        require_private(profile,env)
        with (profile/'backend.log').open('w') as output:
            app=subprocess.Popen([str(repo/'target/debug/kde-assistant'),'--ui-off','--no-shortcuts','--http-port','0'],cwd=profile,env=env,stdout=output,stderr=subprocess.STDOUT)
        wait_for(lambda:'Núcleo de texto listo en http://' in (profile/'backend.log').read_text(),20)
        url=re.findall(r'Núcleo de texto listo en (http://127.0.0.1:\d+)',(profile/'backend.log').read_text())[-1]
        token=(profile/'config/kde-assistant/server.token').read_text().strip()
        def api(path,body=None):
            req=urllib.request.Request(url+path,data=None if body is None else json.dumps(body).encode(),headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
            with urllib.request.urlopen(req,timeout=15) as response:return json.load(response)
        def status():
            value=api('/api/voice/status');observed.extend(children(app.pid));return value
        def no_children():return not children(app.pid)
        sid=api('/api/session',{'title':'Synthetic Piper offline'})['id']
        sid2=api('/api/session',{'title':'Synthetic Piper second'})['id']
        cfg=api('/api/config')
        cfg['speech'].update(piper_runtime=str(root/'venv'),piper_model=str(root/'voice/es_ES-sharvard-medium.onnx'),piper_speaker=0,auto_speak=False)
        cfg['memory'].update(enabled=False,auto_summarize=False)
        cfg['ai'].update(provider='custom',base_url=f'http://127.0.0.1:{mock.server_port}/v1',model='synthetic-piper',api_key='synthetic-piper',provider_keys={},provider_base_urls={},provider_models={},enable_tool_calling=False)
        api('/api/config',cfg)
        results=[]
        def speak(text='Hola.',session=sid):
            scope=api('/api/voice/session',{'session_id':session})['scope']
            return api('/api/speak',{'session_id':session,'scope':scope,'text':text})
        # Cancel immediate preparation, then a running preparation worker.
        # The Preparing state includes integrity checks, import, load and synthesis;
        # these timings alone do not prove which internal phase was interrupted.
        for name,delay in [('cancel-preparation',0),('cancel-active-preparation',1.5)]:
            snapshot=speak('Esta es una prueba larga. '*200)
            if delay:time.sleep(delay)
            active=children(app.pid);observed.extend(active)
            before=time.monotonic();api('/api/voice/cancel',snapshot['identity'])
            wait_for(no_children,5)
            assert status()['state']=='idle'
            results.append(dict(test=name,status='PASS',cancel_ms=(time.monotonic()-before)*1000,children_reaped=active))
        # Render real PCM then link only to a private null sink.
        def link_playback():
            nodes=json.loads(subprocess.check_output(['/usr/bin/pw-dump'],env=env,timeout=3))
            playback=next((n for n in nodes if n.get('info',{}).get('props',{}).get('media.category')=='Playback'),None)
            if not playback:return False
            subprocess.run(['/usr/bin/pw-cli','set-param',str(playback['id']),'PortConfig','{ direction = Output mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 22050 channels = 1 position = [ MONO ] } }'],env=env,capture_output=True,check=True,timeout=3)
            outputs=subprocess.check_output(['/usr/bin/pw-link','-o'],env=env,text=True,timeout=3).splitlines()
            port=next((p.strip() for p in outputs if p.strip().endswith(':output_MONO')),None)
            if not port:return False
            subprocess.run(['/usr/bin/pw-link',port,'f4-tts-sink:playback_MONO'],env=env,capture_output=True,check=True,timeout=3)
            return True
        snapshot=speak('Hola. Esta respuesta se detendrá antes de terminar. '*4)
        wait_for(lambda:status()['state']=='speaking',40);wait_for(link_playback,5)
        before=time.monotonic();api('/api/voice/cancel',snapshot['identity']);wait_for(no_children,5)
        assert status()['state']=='idle'
        results.append(dict(test='cancel-null-playback',status='PASS',cancel_ms=(time.monotonic()-before)*1000))
        # New speech must complete after interruption, with no overlap.
        speak();wait_for(lambda:status()['state']=='speaking',20);wait_for(link_playback,5)
        wait_for(lambda:status()['state']=='idle',15);wait_for(no_children,5)
        results.append(dict(test='new-playback-after-cancel',status='PASS'))
        snapshot=speak('Este audio es anterior al cambio de conversación. '*3)
        wait_for(lambda:status()['state']=='speaking',35);wait_for(link_playback,5)
        before=time.monotonic();api('/api/voice/session',{'session_id':sid2});wait_for(no_children,5)
        assert status()['state']=='idle'
        results.append(dict(test='session-change-during-playback',status='PASS',cancel_ms=(time.monotonic()-before)*1000))
        assert not calls and api('/api/messages?session_id='+sid)==[]
        # A single completed mock turn, then auto-reading; no additional prompts/tools.
        cfg=api('/api/config');cfg['speech']['auto_speak']=True;api('/api/config',cfg)
        api('/api/voice/session',{'session_id':sid2})
        answer=api('/api/chat/complete',{'session_id':sid2,'message':'Mensaje sintético para lectura'})
        messages_before=api('/api/messages?session_id='+sid2)
        wait_for(lambda:status()['state']=='speaking',30);wait_for(link_playback,5)
        wait_for(lambda:status()['state']=='idle',20);wait_for(no_children,5)
        assert len(calls)==1 and messages_before==api('/api/messages?session_id='+sid2)
        results.append(dict(test='mock-completed-turn-auto-read',status='PASS',provider_calls=len(calls),persisted_text_unchanged=True))
        # Explicit missing model and absent runtime fail without a voice child remaining.
        cfg=api('/api/config');cfg['speech'].update(auto_speak=False,piper_model='/nonexistent/f4-voice.onnx');api('/api/config',cfg)
        speak(session=sid2);wait_for(lambda:status()['state']=='error',15);wait_for(no_children,5)
        results.append(dict(test='missing-model-text-independent',status='PASS',error=status()['error']))
        idle=sample([app.pid],3);assert no_children()
        summary=dict(category='AUTOMATED_AUDIO_FIXTURE',audible='NOT_TESTED',physical_microphone='NOT_TESTED',results=results,idle=idle,provider_calls=calls,observed_child_pids=sorted(set(observed)),profile=str(profile))
        (profile/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        print('RESULT',profile/'summary.json',flush=True)
    finally:
        if app:
            if app.poll() is None:app.terminate()
            app.wait(timeout=10)
        if daemon:
            if daemon.poll() is None:daemon.terminate()
            daemon.wait(timeout=5)
        if mock:mock.shutdown();mock.server_close();thread.join(timeout=2)
        assert all(not Path('/proc/'+str(pid)).exists() for pid in set(observed))


if __name__=='__main__':main()
