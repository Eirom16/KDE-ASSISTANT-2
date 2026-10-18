#!/usr/bin/env python3
"""F3.5: synthetic loopback credentials only; isolated XDG and genuine process restarts.
--plasma renders only this harness's windows on the current Wayland session.
Never contacts real providers, registers shortcuts, or reads personal configuration.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.dont_write_bytecode = True
from f3_runtime import children, wait_for, kwin
from qml_test_harness import build_runner


def qml_tests(repo, base, env, runner):
    cfg={'config_revision':'synthetic-revision','ai':{'provider':'groq','api_key':'','provider_keys':{},'credential_states':{'groq':'stored'},'base_url':'http://127.0.0.1:1234/v1','provider_base_urls':{},'model':'fixture-chat','provider_models':{},'provider_endpoint_approvals':{'groq':'http://127.0.0.1:1234/v1'},'temperature':0.7,'max_tokens':2048,'enable_tool_calling':False},'ui':{'theme':'dark','close_to_tray':True},'speech':{'piper_model':'es_ES-sharvard-medium','piper_length_scale':1,'stt_language':'auto','mic_device':'','stt_model':'base','wake_word':'hey jarvis','wake_greeting':'Sí, dígame','wake_word_threshold':0.5,'auto_speak':False,'auto_listen':False,'chimes_enabled':False},'tools':{},'shortcuts':{'toggle':'Super+Shift+A','push_to_talk':'Super+Shift+V','new_session':'Ctrl+Shift+K','menu':'Super+Shift+M'},'memory':{'enabled':False,'auto_summarize':False}}
    class Api(BaseHTTPRequestHandler):
        def log_message(self,*args): pass
        def reply(self,status,value):
            data=json.dumps(value).encode(); self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers()
            try:self.wfile.write(data)
            except (BrokenPipeError,ConnectionResetError):pass
        def do_GET(self): self.reply(200,cfg)
        def do_POST(self):
            data=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            if self.path.startswith('/api/ai-') and data.get('provider')=='groq' and data.get('approved_endpoint') != data.get('base_url','').rstrip('/'):
                self.reply(502,{'category':'untrusted_endpoint','error':'Confirma esta URL alternativa'}); return
            if self.path=='/api/ai-models': self.reply(200,{'models':['fixture-chat','another-chat']}); return
            if self.path=='/api/ai-test': self.reply(200,{'status':'ok','message':'Conexión simulada verificada'}); return
            if data['ai']['model']=='stale-revision': self.reply(409,{'error':'La configuración cambió; vuelve a cargarla'}); return
            if data['ai']['model']=='slow-save':
                import time
                time.sleep(0.25)
            if data['ai']['model']=='fail-write': self.reply(500,{'error':'Escritura simulada fallida'})
            elif data['ai']['model']=='fake-success': self.reply(200,{'status':'pending'})
            else: self.reply(200,{'status':'ok'})
    server=ThreadingHTTPServer(('127.0.0.1',0),Api)
    worker=threading.Thread(target=server.serve_forever,daemon=True);worker.start()
    try:
        test=base/'tst_f35_settings.qml'
        test.write_text((repo/'tests/qml/tst_f35_settings.qml').read_text().replace('F35_BACKEND',f'http://127.0.0.1:{server.server_port}').replace('F35_IMAGES',str(base)))
        result=subprocess.run([str(runner),'-import',str(repo),'-input',str(test),'-o','-,txt'],env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=90)
        (base/'qml.log').write_text(result.stdout)
        assert result.returncode==0, result.stdout[-6000:]
        assert 'Unable to assign' not in result.stdout, result.stdout[-6000:]
        assert len(list(base.glob('desktop-*.png')))==9
        return {'name':'qml_layout_keyboard_save_errors','status':'PASS','counts':re.findall(r'Totals: .*',result.stdout),'screenshots':9}
    finally: server.shutdown();server.server_close();worker.join(timeout=3)


def runtime_tests(repo, base, env, plasma):
    calls=[]
    class Provider(BaseHTTPRequestHandler):
        def log_message(self,*args): pass
        def reply(self,data,kind):
            self.send_response(200);self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
        def do_GET(self):
            assert self.path=='/v1/models'
            assert self.headers.get('Authorization')=='Bearer synthetic-f35-not-a-real-key'
            calls.append('models')
            self.reply(json.dumps({'data':[{'id':'fixture-chat'}]}).encode(),'application/json')
        def do_POST(self):
            assert self.path=='/v1/chat/completions'
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            assert body['model']=='fixture-chat' and body['max_completion_tokens']==64 and body['stream'] is True
            assert 'tools' not in body and 'HTTP-Referer' not in self.headers
            calls.append('chat')
            self.reply(b'data: {"choices":[{"delta":{"content":"OK"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n','text/event-stream')
    provider=ThreadingHTTPServer(('127.0.0.1',0),Provider)
    worker=threading.Thread(target=provider.serve_forever,daemon=True);worker.start()
    provider_url=f'http://127.0.0.1:{provider.server_port}/v1'
    profile=base/'runtime';profile.mkdir(mode=0o700)
    private=env.copy()
    for key,folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:
        path=profile/folder;path.mkdir(mode=0o700);private[key]=str(path)
    private['QT_QPA_PLATFORM']='wayland' if plasma else 'offscreen'
    resources=profile/'resources';resources.mkdir(mode=0o700)
    shutil.copytree(repo/'qml',resources/'qml')
    (resources/'assets').symlink_to(repo/'assets',target_is_directory=True)
    private['KDE_ASSISTANT_DATA_DIR']=str(resources)
    original=(repo/'qml/Main.qml').read_text()
    injected=(repo/'tests/qml/F35Runtime.qmlpart').read_text().replace('F35_PROVIDER',provider_url)
    binary=repo/'target/debug/kde-assistant'
    pids=[];app=None;qml=None
    config=profile/'config/kde-assistant/config.json'
    def launch(mode):
        nonlocal app,qml
        main=original.rstrip(); assert main.endswith('}')
        (resources/'qml/Main.qml').write_text(main[:-1]+injected.replace('F35_MODE',mode)+'\n}\n')
        output=profile/(mode+'.log')
        with output.open('w') as log:
            app=subprocess.Popen([str(binary),'--no-shortcuts','--http-port','0'],cwd=profile,env=private,stdout=log,stderr=subprocess.STDOUT)
        qml=wait_for(lambda:next((pid for pid in children(app.pid) if b'/qml/Main.qml' in Path(f'/proc/{pid}/cmdline').read_bytes()),None))
        marker='F35_UI_SAVE_PASS' if mode=='save' else 'F35_UI_RESTART_PASS'
        wait_for(lambda: marker in output.read_text() or app.poll() is not None,25)
        assert marker in output.read_text(), output.read_text()[-4000:]
        assert app.poll() is None
        observation=kwin(profile,qml) if plasma else None
        if plasma: assert observation and observation['found'] and not observation['minimized']
        pids.append({'mode':mode,'backend':app.pid,'qml':qml,'marker':marker,'kwin':observation})
    def stop():
        nonlocal app
        app.terminate();app.wait(timeout=15)
        assert app.returncode==0
        wait_for(lambda:not Path(f'/proc/{qml}').exists())
        app=None
    def api(path,body=None,authorized=True):
        module=next((profile/'cache/kde-assistant/qml-generated').glob('*/qml/auth/AuthToken.qml')).read_text()
        url=json.loads(re.search(r'readonly property string backendUrl: ("[^"]+")',module)[1])
        token=(profile/'config/kde-assistant/server.token').read_text().strip() if authorized else 'invalid-test-token'
        req=urllib.request.Request(url+path,data=json.dumps(body).encode() if body is not None else None,headers={'Authorization':'Bearer '+token,'Content-Type':'application/json'})
        try:
            with urllib.request.urlopen(req,timeout=30) as response:return response.status,json.load(response)
        except urllib.error.HTTPError as error:return error.code,json.load(error)
    try:
        launch('save')
        assert not calls, 'Saving must not call the provider'
        assert config.stat().st_mode & 0o777==0o600
        before=config.read_bytes()
        stop()
        launch('restart')
        assert config.read_bytes()==before
        status,cfg=api('/api/config');assert status==200
        assert cfg['ai']['api_key']=='' and cfg['ai']['provider_keys']=={}
        assert 'synthetic-f35-not-a-real-key' not in json.dumps(cfg)
        status,_=api('/api/ai-test',{},False);assert status==401 and not calls
        status,result=api('/api/ai-test',{'provider':'groq','base_url':provider_url,'model':'fixture-chat'})
        assert status==200 and result['status']=='ok' and calls==['models','chat']
        cfg['ai']['provider_models']['groq']=''
        cfg['ai']['model']=''
        status,_=api('/api/config',cfg);assert status==400 and config.read_bytes()==before
        # A backup symlink makes the secure writer fail before publishing.
        _,cfg=api('/api/config')
        backup=config.with_suffix('.json.bak');backup.unlink()
        sentinel=profile/'sentinel';sentinel.write_text('untouched');backup.symlink_to(sentinel)
        cfg['ui']['close_to_tray']=False
        status,_=api('/api/config',cfg);assert status==500 and config.read_bytes()==before and sentinel.read_text()=='untouched'
        _,current=api('/api/config');assert current['ui']['close_to_tray'] is True
        backup.unlink()
        # Switch repeatedly. Every key is synthetic and remains scoped.
        for selected,key in [('openrouter','synthetic-or'),('custom','synthetic-custom'),('groq','')]:
            _,cfg=api('/api/config');cfg['ai']['provider']=selected
            cfg['ai']['provider_base_urls'][selected]=provider_url
            cfg['ai']['provider_models'][selected]='fixture-chat'
            cfg['ai']['provider_keys']={selected:key}
            status,_=api('/api/config',cfg);assert status==200
        disk=json.loads(config.read_text());assert disk['ai']['provider_keys']=={'groq':'synthetic-f35-not-a-real-key','openrouter':'synthetic-or','custom':'synthetic-custom'}
        _,cfg=api('/api/config');cfg['delete_provider_keys']=['groq'];status,_=api('/api/config',cfg);assert status==200
        _,cfg=api('/api/config');assert cfg['ai']['credential_states']['groq']=='none'
        assert cfg['ai']['credential_states']['openrouter']=='stored'
        # Restore synthetic Groq for another genuine process restart.
        cfg['ai']['provider_keys']={'groq':'synthetic-f35-not-a-real-key'}
        status,_=api('/api/config',cfg);assert status==200
        # A second confirmed save makes the app's own backup the recovery source.
        _,cfg=api('/api/config'); status,_=api('/api/config',cfg); assert status==200
        stop();launch('restart-again');stop()
        # Corruption fails closed; explicit recovery from last valid private backup.
        assert json.loads(backup.read_text())['ai']['provider_keys']['groq']=='synthetic-f35-not-a-real-key'
        corrupt=b'{broken-f35'
        config.write_bytes(corrupt)
        log=profile/'corrupt.log'
        with log.open('w') as output:
            failed=subprocess.run([str(binary),'--no-shortcuts','--http-port','0'],env=private,cwd=profile,stdout=output,stderr=subprocess.STDOUT,timeout=15)
        assert failed.returncode!=0 and config.read_bytes()==corrupt
        preserved=profile/'config/kde-assistant/config.corrupt-preserved';config.rename(preserved)
        # Explicit operator recovery, never an automatic fallback or replay.
        config.write_bytes(backup.read_bytes()); config.chmod(0o600)
        launch('restart-recovered');stop()
        assert preserved.read_bytes()==corrupt
        for log in profile.glob('*.log'):
            text=log.read_text();assert 'synthetic-f35-not-a-real-key' not in text
            assert 'TypeError' not in text and 'ReferenceError' not in text and 'Binding loop' not in text
        return {'name':'genuine_backend_qml_restart_http_storage_recovery','status':'PASS','platform':private['QT_QPA_PLATFORM'],'processes':pids,'provider_calls':calls,'invalid_save':400,'failed_write':500,'local_auth_rejection':401,'manual_corrupt_recovery':'PASS','config_sha256_before_restart':hashlib.sha256(before).hexdigest()}
    finally:
        if app is not None and app.poll() is None: app.terminate();app.wait(timeout=15)
        provider.shutdown();provider.server_close();worker.join(timeout=3)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--plasma',action='store_true');args=parser.parse_args()
    repo=Path(__file__).resolve().parent.parent
    os.umask(0o077);base=Path(tempfile.mkdtemp(prefix='kda-f35-validation-'));print('Evidence: '+str(base),flush=True)
    env=os.environ.copy()
    for key in list(env):
        if key.endswith('_API_KEY') or key.upper().endswith('_PROXY') or key.startswith('KDA_F1'):env.pop(key)
    env.update(QT_QPA_PLATFORM='offscreen',QML_DISABLE_DISK_CACHE='1',QT_QUICK_CONTROLS_STYLE='Basic',CARGO_NET_OFFLINE='true',QT_FORCE_STDERR_LOGGING='1',QT_ASSUME_STDERR_HAS_CONSOLE='1')
    for key,folder in [('XDG_CONFIG_HOME','config'),('XDG_DATA_HOME','data'),('XDG_CACHE_HOME','cache')]:
        directory=base/folder;directory.mkdir(mode=0o700);env[key]=str(directory)
    results=[]
    def save(): (base/'results.json').write_text(json.dumps({'results':results,'real_groq':'NOT_TESTED','human_keyboard':'NOT_TESTED'},indent=2))
    try:
        for name,cmd in [('fmt',['cargo','fmt','--check']),('providers',['cargo','test','--locked','--test','f35_providers']),('build',['cargo','build','--locked','--bin','kde-assistant'])]:
            with (base/(name+'.log')).open('w') as output:done=subprocess.run(cmd,cwd=repo,env=env,stdout=output,stderr=subprocess.STDOUT)
            results.append({'name':name,'status':'PASS' if done.returncode==0 else 'NEEDS_FIXES','exit':done.returncode});save();assert done.returncode==0,name
        runner=build_runner(repo,base,env)
        results.append(qml_tests(repo,base,env,runner));save();print('QML: PASS',flush=True)
        results.append(runtime_tests(repo,base,env,args.plasma));save();print('Restart, storage and loopback: PASS',flush=True)
    except Exception as error:
        results.append({'name':'validation','status':'NEEDS_FIXES','reason':str(error)});save();raise
    return 0

if __name__=='__main__':raise SystemExit(main())
