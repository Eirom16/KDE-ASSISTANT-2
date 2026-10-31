#!/usr/bin/env python3
"""One private hands-free trial. Prepare never opens audio; launch requires scoped explicit consent."""
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
import time
import uuid
sys.dont_write_bytecode = True
from compare_f4_spanish import REPO, baseline, private_file, profile_path, write_private
from f4_safety import private_env, require_private
from live_f4_ptt import application, attach_log, api, save_observation
from f3_runtime import children, wait_for
from record_f4_spanish import live_remote
PURPOSE = 'hey_jarvis_background_two_activations_v1'
LIMIT = 180
MODEL_HASHES = {
 'melspectrogram.onnx':'ba2b0e0f8b7b875369a2c89cb13360ff53bac436f2895cced9f479fa65eb176f',
 'embedding_model.onnx':'70d164290c1d095d1d4ee149bc5e00543250a7316b59f31d056cff7bd3075c1f',
 'hey_jarvis_v0.1.onnx':'94a13cfe60075b132f6a472e7e462e8123ee70861bc3fb58434a73712ee0d2cb'}


def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def stop_reason(status):
 """Count detections, including cancelled turns; a pause ends this campaign."""
 bg=status['background']
 if bg['activations']>2:return 'Activation count exceeded consent'
 if bg['state']=='error':return 'Background error; no automatic retry'
 if bg['state'] in ('paused','off') and bg['activations']:
  return 'Pause/disable ended the trial'
 if bg['activations']==2 and status['state'] not in ('recording','transcribing'):
  return 'Second activation completed or cancelled'
 return None


def verify(root, scenario):
 env=private_env(root);require_private(root,env);baseline(env)
 if digest(REPO/'target/debug/kde-assistant')!=scenario['binary_sha256']:raise ValueError('Binary changed')
 for path,expected in scenario['resources'].items():
  if digest(path)!=expected:raise ValueError('Resource changed: '+Path(path).name)
 cfg=root/'config/kde-assistant/config.json'
 if digest(private_file(cfg,root,262144))!=scenario['config_sha256']:raise ValueError('Config changed')
 for path,expected in scenario['qml'].items():
  if digest(private_file(root/path,root,2097152))!=expected:raise ValueError('Private UI changed')
 return env


def close(app, thread):
 if app.poll() is None:app.terminate()
 try:app.wait(timeout=15)
 except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
 if thread:thread.join(timeout=2)
 if app.stdout:app.stdout.close()


def prepare(models, runtime, stt):
 models=models.resolve(strict=True);runtime=runtime.resolve(strict=True);stt=stt.resolve(strict=True)
 root=Path(tempfile.mkdtemp(prefix='kda-hey-jarvis-human-',dir='/var/tmp'));env=private_env(root);baseline(env)
 binary=REPO/'target/debug/kde-assistant'
 resources={str(models/n):digest(models/n) for n in MODEL_HASHES}
 assert all(resources[str(models/n)]==h for n,h in MODEL_HASHES.items())
 assert digest(stt)=='be07e048e1e599ad46341c8d2a135645097a538221678b7acdd1b1919c6e1b21','Expected previously evaluated multilingual tiny'
 resources[str(stt)]=digest(stt);resources[str(runtime)]=digest(runtime)
 for arguments,expected in [(['--voice-model-check',str(stt)],b'null'),(['--voice-wake-stream',str(models),str(runtime),'0.8'],b'R')]:
  result=subprocess.run([str(binary),*arguments],cwd=root,env=env,input=b'',capture_output=True,timeout=20,check=True)
  assert result.stdout==expected
 app=application(root,env,False);thread=None
 try:
  url,token,thread=attach_log(app,root,'prepare.log')
  cfg=api(url,token,'/api/config')
  cfg['speech'].update(stt_engine='whisper',stt_model=str(stt),stt_language='es',stt_backend='local',
    wake_word_model_path=str(models),wake_word_threshold=.8,wake_word_enabled=False,
    auto_speak=False,auto_listen=False,chimes_enabled=False,piper_fast=False,
    max_recording_secs=12,stt_timeout_secs=120)
  cfg['ai'].update(provider='custom',base_url='http://127.0.0.1:1/v1',api_key='',provider_keys={},
    provider_base_urls={},provider_models={},provider_endpoint_approvals={},model='offline-no-send',enable_tool_calling=False)
  cfg['memory'].update(enabled=False,auto_summarize=False);cfg['character']['enabled']=False
  cfg['ui'].update(close_to_tray=True,always_on_top=False)
  for k,v in cfg['tools'].items():
   if isinstance(v,bool):cfg['tools'][k]=(k=='confirm_sensitive')
  api(url,token,'/api/config',cfg)
  sid=api(url,token,'/api/session',{'title':'Hey Jarvis · prueba privada'})['id']
  api(url,token,'/api/voice/session',{'session_id':sid})
  status=api(url,token,'/api/voice/status')
  assert status['state']=='idle' and not status['background']['enabled'] and not status['background']['microphone_active']
  assert not children(app.pid)
 finally:close(app,thread)
 resources_dir=root/'resources';resources_dir.mkdir(mode=0o700)
 shutil.copytree(REPO/'qml',resources_dir/'qml');(resources_dir/'assets').symlink_to(REPO/'assets',target_is_directory=True)
 main=resources_dir/'qml/Main.qml';text=main.read_text()
 text=text.replace('title: "KDE Assistant v2"','title: "KDE Assistant · Hey Jarvis privado"',1)
 text=text.replace('    function sendMessage(text) {','    function sendMessage(text) { return; // Private trial: sending is disabled.\n',1)
 text=text.replace('                voiceEnabled: root.voiceAvailable && !nativeVoice.backgroundEnabled','                voiceEnabled: false // Only the background option is exercised.',1)
 main.write_text(text)
 for item in (resources_dir/'qml').rglob('*'):
  if item.is_file():item.chmod(0o600)
 qml={str(p.relative_to(root)):digest(p) for p in (resources_dir/'qml').rglob('*') if p.is_file()}
 scenario={'purpose':PURPOSE,'nonce':str(uuid.uuid4()),'binary_sha256':digest(binary),'resources':resources,
  'config_sha256':digest(root/'config/kde-assistant/config.json'),'qml':qml,'session_id':sid,
  'max_wall_secs':LIMIT,'max_activations':2,'reference':['Abre el navegador','Qué hora es'],
  'microphone_opened':False,'background_enabled':False,'remote_provider':False,'tools':False,'automatic_send':False}
 write_private(root/'scenario.json',scenario);verify(root,scenario)
 print(json.dumps({'profile':str(root),'nonce':scenario['nonce'],'binary_sha256':scenario['binary_sha256'],
  'resource_checks':'PASS','isolated_headless_startup':'PASS','microphone_opened':False,'background_enabled':False},indent=2))


def launch(root,receipt):
 scenario=json.loads(private_file(root/'scenario.json',root,262144).read_text());env=verify(root,scenario)
 permission=json.loads(private_file(receipt,root,16384).read_text());marker=receipt.with_suffix('.used')
 if marker.exists():raise ValueError('Permission already consumed')
 if any(permission.get(k)!=v for k,v in {'purpose':PURPOSE,'nonce':scenario['nonce'],'binary_sha256':scenario['binary_sha256'],'max_wall_secs':LIMIT,'max_activations':2}.items()):raise ValueError('Consent scope mismatch')
 if not isinstance(permission.get('authorization_reference'),str) or not permission['authorization_reference'].strip():raise ValueError('Explicit microphone/background permission missing')
 display=os.environ.get('WAYLAND_DISPLAY','');host_runtime=os.environ.get('XDG_RUNTIME_DIR','')
 display=Path(display) if Path(display).is_absolute() else Path(host_runtime)/display
 if not display.is_socket():raise ValueError('Wayland unavailable')
 remote=live_remote() # Reached only after the explicit receipt has passed.
 write_private(marker,{'consumed':True,'purpose':PURPOSE})
 live={**env,'PIPEWIRE_REMOTE':str(remote),'WAYLAND_DISPLAY':str(display),'QT_QPA_PLATFORM':'wayland',
  'KDE_ASSISTANT_DATA_DIR':str(root/'resources')}
 # DBus remains private/absent: no personal activation, shortcuts or notifications.
 app=application(root,live,True);thread=None;owned=set();events=[];previous=None;result={'human':'PENDING','raw_audio_saved':False}
 started=time.monotonic();seen=set();microphone_seen=False;maximum_activations=0
 try:
  url,token,thread=attach_log(app,root,'live.log')
  print('GUI_READY: activa Hey Jarvis una sola vez. Minimiza; di Hey Jarvis y la primera orden sin clic adicional. Revisa/edita el borrador y pulsa Cancelar. Espera listening; repite Hey Jarvis con la segunda frase. No pulses Enviar.',flush=True)
  while app.poll() is None and time.monotonic()-started<LIMIT:
   owned.update(children(app.pid));status=api(url,token,'/api/voice/status');bg=status['background']
   microphone_seen=microphone_seen or bg['microphone_active']
   maximum_activations=max(maximum_activations,bg['activations'])
   key=(status['state'],bg['state'],bg['activations'])
   if key!=previous:
    previous=key;events.append({'state':key[0],'background':key[1],'activations':key[2],'ms':(time.monotonic()-started)*1000})
    print('STATE',key,flush=True)
   reason=stop_reason(status)
   if status['state']=='transcript_ready' and status.get('identity'):
    identity=status['identity']['generation']
    if identity not in seen:
     seen.add(identity);print('DRAFT_READY',len(seen),flush=True)
   if reason:
    result['stop_reason']=reason
    result['drafts_observed']=len(seen)
    result['activations_observed']=bg['activations']
    if bg['activations']>2 or bg['state']=='error':result['failure']=reason
    api(url,token,'/api/voice/background',{'action':'pause'})
    break
   save_observation(root,{**result,'transitions':events,'ended':False})
   time.sleep(.05)
  api(url,token,'/api/voice/background',{'action':'disable'})
  sessions=api(url,token,'/api/sessions');result['message_count']=sum(s.get('message_count',0) for s in sessions)
 except Exception as error:
  result['failure']=type(error).__name__
  raise
 finally:
  if app.poll() is None and thread:
   try:
    api(url,token,'/api/voice/background',{'action':'disable'})
    sessions=api(url,token,'/api/sessions')
    result['message_count']=sum(s.get('message_count',0) for s in sessions)
   except Exception:result['disable_api_unavailable']=True
  owned.update(children(app.pid));close(app,thread)
  result.update(ended=True,transitions=events,backend_reaped=app.poll() is not None,
   owned_children_reaped=all(not Path(f'/proc/{pid}').exists() for pid in owned),
   microphone_opened_after_authorization=microphone_seen,activations_observed=maximum_activations,
   wall_seconds=time.monotonic()-started)
  save_observation(root,result)
 print('CLOSED: prueba terminada; escucha desactivada y procesos propios cerrados.',flush=True)


def main():
 p=argparse.ArgumentParser(description=__doc__);mode=p.add_mutually_exclusive_group(required=True)
 mode.add_argument('--prepare',action='store_true');mode.add_argument('--launch',action='store_true')
 for key in ['models','onnx-runtime','stt-model','profile','receipt']:p.add_argument('--'+key,type=Path)
 a=p.parse_args()
 if a.prepare:
  if not all([a.models,a.onnx_runtime,a.stt_model]):p.error('Existing model/runtime paths required')
  prepare(a.models,a.onnx_runtime,a.stt_model)
 else:
  if not a.profile or not a.receipt:p.error('Explicit profile and consent receipt required')
  launch(profile_path(a.profile),a.receipt)
if __name__=='__main__':main()
