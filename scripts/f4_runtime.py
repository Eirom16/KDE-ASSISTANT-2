"""Genuine F4 HTTP lifecycle, zero hardware/network audio. Only synthetic chat loopback."""
import json
import os
from pathlib import Path
import re
import subprocess
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from f3_runtime import wait_for, children, sample
from f4_safety import require_private

def runtime(repo, profile, env, out):
    require_private(profile, env)
    calls=[]
    class Mock(BaseHTTPRequestHandler):
        def log_message(self,*args): pass
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            calls.append({'path':self.path,'message_count':len(body.get('messages',[]))})
            assert 'audio' not in self.path and self.path.endswith('/chat/completions')
            data=b'data: {"choices":[{"delta":{"content":"Texto operativo sin voz"},"finish_reason":null}]}\n\ndata: {"choices":[{"delta":{},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
    mock=ThreadingHTTPServer(('127.0.0.1',0),Mock)
    worker=threading.Thread(target=mock.serve_forever,daemon=True);worker.start()
    log=out/'runtime.log'
    app=None
    def launch():
        nonlocal app
        output=log.open('w')
        app=subprocess.Popen([str(repo/'target/debug/kde-assistant'),'--ui-off','--no-shortcuts','--http-port','0'],env=env,cwd=profile,stdout=output,stderr=subprocess.STDOUT)
        output.close()
        wait_for(lambda: 'Núcleo de texto listo en http://' in log.read_text(),20)
        return re.findall(r'Núcleo de texto listo en (http://127.0.0.1:\d+)',log.read_text())[-1]
    try:
        # Internal modes must be dispatched before Backend/config/Qt startup.
        check=subprocess.run([str(repo/'target/debug/kde-assistant'),'--voice-model-check','/nonexistent/f4-model'],env=env,cwd=profile,capture_output=True,text=True,timeout=5)
        assert check.returncode==0 and 'no instalado' in json.loads(check.stdout)
        invalid=subprocess.run([str(repo/'target/debug/kde-assistant'),'--voice-invalid'],env=env,cwd=profile,capture_output=True,text=True,timeout=5)
        assert invalid.returncode!=0
        assert not (profile/'config/kde-assistant/config.json').exists()
        url=launch()
        token=(profile/'config/kde-assistant/server.token').read_text().strip()
        def api(path,body=None,authenticated=True):
            request=urllib.request.Request(url+path,data=json.dumps(body).encode() if body is not None else None,headers={'Authorization':'Bearer '+(token if authenticated else 'bad'),'Content-Type':'application/json'})
            try:
                with urllib.request.urlopen(request,timeout=8) as reply:return reply.status,json.load(reply)
            except urllib.error.HTTPError as error:
                try:value=json.load(error)
                except (ValueError,json.JSONDecodeError):value={}
                return error.code,value
        assert api('/api/voice/status',authenticated=False)[0]==401
        assert api('/api/voice/status')[1]['state']=='idle'
        assert children(app.pid)==[], 'Voice children at idle startup'
        idle=sample([app.pid],3)
        _,session=api('/api/session',{'title':'F4 synthetic dictation'})
        sid=session['id']
        assert api('/api/voice/start',{'session_id':'foreign','scope':'fake'})[0]==404
        _,selected=api('/api/voice/session',{'session_id':sid})
        assert api('/api/voice/start',{'session_id':sid,'scope':selected['scope'],'transcript':'injected'})[0]==422
        status,snapshot=api('/api/voice/start',{'session_id':sid,'scope':selected['scope']})
        assert status==202
        wait_for(lambda:api('/api/voice/status')[1]['state']=='error',10)
        error=api('/api/voice/status')[1]
        (out/'missing-model.json').write_text(json.dumps(error,indent=2))
        assert 'no instalado' in error['error']
        assert not calls
        assert api('/api/messages?session_id='+sid)[1]==[]
        assert api('/api/voice/cancel',error['identity'])[0]==200
        assert api('/api/voice/cancel',error['identity'])[0]==409
        assert api('/api/voice/start',{'session_id':sid,'scope':selected['scope']})[0]==409
        # Retired mutation routes are unavailable even with the local bearer.
        for route in ['/api/voice/log','/api/voice/sidecar-event','/api/voice/calibration','/api/dictate/start']:
            assert api(route,{})[0] in (404,503)
        _,cfg=api('/api/config')
        cfg['ai'].update(provider='custom',base_url=f'http://127.0.0.1:{mock.server_port}/v1',model='synthetic-f4',api_key='synthetic-f4',provider_keys={},provider_base_urls={},provider_models={},enable_tool_calling=False)
        cfg['speech'].update(stt_backend='api',auto_speak=False)
        assert api('/api/config',cfg)[0]==200
        assert api('/api/chat/complete',{'session_id':sid,'message':'Mensaje escrito sintético'})[0]==200
        assert len(calls)==1 and calls[0]['path']=='/v1/chat/completions'
        wait_for(lambda:children(app.pid)==[])
        result={'name':'native-runtime','category':'AUTOMATED_AUDIO_FIXTURE','status':'PASS','backend_pid':app.pid,'idle':idle,'provider_calls':calls,'microphone':'NOT_TESTED','tts':'NOT_TESTED','wake':'NOT_TESTED','missing_model_error':error['error']}
        return result
    finally:
        if app:
            app.terminate();app.wait(timeout=10)
            assert app.returncode==0
            assert not children(app.pid)
        mock.shutdown();mock.server_close();worker.join(timeout=2)
