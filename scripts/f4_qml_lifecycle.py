"""Production Main.qml with delayed synthetic HTTP. Never starts a Rust/audio backend."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from f4_safety import require_private

def validate(repo, profile, env, out):
    require_private(profile, env)
    observations=[]
    for scenario in ['normal','cancel','hide']:
        root=profile/('main-'+scenario);root.mkdir(mode=0o700)
        resources=root/'resources';resources.mkdir(mode=0o700)
        shutil.copytree(repo/'qml',resources/'qml')
        (resources/'assets').symlink_to(repo/'assets',target_is_directory=True)
        calls=[]
        sessions=[]
        identity={'session_id':'synthetic-voice-session','generation':'synthetic-scope','turn_id':None}
        class Api(BaseHTTPRequestHandler):
            def log_message(self,*args):pass
            def reply(self,value):
                data=json.dumps(value).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers()
                try:self.wfile.write(data)
                except (BrokenPipeError,ConnectionResetError):pass
            def do_GET(self):
                if self.path=='/api/health':self.reply({'status':'ok'})
                elif self.path=='/api/sessions':self.reply(sessions)
                elif self.path=='/api/config':self.reply({'ai':{'has_api_key':False,'model':'mock'},'ui':{'theme':'dark','close_to_tray':False},'speech':{},'shortcuts':{},'capabilities':{'native_dictation':True,'whisper_model_present':True}})
                elif self.path=='/api/voice/status':self.reply({'state':'recording','identity':identity})
                elif self.path.startswith('/api/messages'):self.reply([])
                else:self.reply({})
            def do_POST(self):
                size=int(self.headers.get('Content-Length','0'));body=json.loads(self.rfile.read(size) or b'{}')
                calls.append(self.path)
                if self.path=='/api/session':
                    time.sleep(.35)
                    session={'id':identity['session_id'],'title':'synthetic','message_count':0,'updated_at':'2026-10-18T00:00:00Z'}
                    sessions.append(session)
                    self.reply(session)
                elif self.path=='/api/voice/session':self.reply({'session_id':body.get('session_id'),'scope':identity['generation']})
                elif self.path=='/api/voice/start':self.reply({'state':'recording','identity':identity})
                else:self.reply({'state':'idle'})
        api=ThreadingHTTPServer(('127.0.0.1',0),Api);thread=threading.Thread(target=api.serve_forever,daemon=True);thread.start()
        # Private import module, last -I as the production bootstrap contract requires.
        module=root/'generated/qml/auth';module.mkdir(parents=True,mode=0o700)
        (module/'qmldir').write_text('module qml.auth\nsingleton AuthToken 1.0 AuthToken.qml\n')
        (module/'AuthToken.qml').write_text('pragma Singleton\nimport QtQuick\nQtObject { readonly property string token: "synthetic-f4"; readonly property string backendUrl: "http://127.0.0.1:'+str(api.server_port)+'"; readonly property string cacheDir: '+json.dumps(str(root))+'; readonly property bool agentOverlay: false }\n')
        extra='''
    Timer { interval: 150; running: true; repeat: false; onTriggered: { inputBar.text = "Borrador conservado"; root.startDictation() } }
    Timer { interval: 250; running: true; repeat: false; onTriggered: ACTION }
    Timer { interval: 1200; running: true; repeat: false; onTriggered: { console.log("F4_DRAFT", inputBar.text); Qt.quit() } }
'''.replace('ACTION',{'normal':'{}','cancel':'nativeVoice.cancel()','hide':'root.hide()'}[scenario])
        main=resources/'qml/Main.qml';text=main.read_text();main.write_text(text[:text.rfind('}')]+extra+'}\n')
        private=env.copy()
        for key,name in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:
            path=root/name;path.mkdir(mode=0o700);private[key]=str(path)
        private.update(QT_QPA_PLATFORM='offscreen',QML_DISABLE_DISK_CACHE='1',
                       QT_FORCE_STDERR_LOGGING='1',QT_ASSUME_STDERR_HAS_CONSOLE='1')
        try:
            with (out/('main-'+scenario+'.log')).open('w') as log:
                result=subprocess.run(['/usr/bin/qml6','-apptype','widget','-I',str(resources),'-I',str(root/'generated'),str(main)],env=private,stdout=log,stderr=subprocess.STDOUT,timeout=8)
            content=(out/('main-'+scenario+'.log')).read_text()
            (out/('main-'+scenario+'-calls.json')).write_text(json.dumps(calls))
            assert result.returncode==0,content
            assert 'F4_DRAFT Borrador conservado' in content,content
            expected=1 if scenario=='normal' else 0
            assert calls.count('/api/voice/start')==expected,(scenario,calls,content)
            assert '/api/chat' not in calls
            observations.append({'scenario':scenario,'category':'AUTOMATED_QML','status':'PASS','capture_start_requests':calls.count('/api/voice/start'),'chat_requests':0,'draft_preserved':True})
        finally:
            api.shutdown();api.server_close();thread.join(timeout=2)
    return observations
