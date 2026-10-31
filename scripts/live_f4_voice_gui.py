#!/usr/bin/env python3
"""Private GUI acceptance fixtures; each trial requires separate speaker consent."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import time
sys.dont_write_bytecode = True
from live_f4_fast_tts import PreparedBackend, verify, admit
from live_f4_ptt import api, save_observation
from f4_safety import private_env, require_private
from compare_f4_spanish import REPO, private_file, profile_path, write_private
from record_f4_spanish import live_remote
from f3_runtime import children, wait_for

PURPOSE = 'f4_r5_human_t2_gui_stop_v1'
REPLAY_PURPOSE = 'f4_r5_human_t3_gui_replay_v1'
SESSION_PURPOSE = 'f4_r5_human_t4_gui_session_v1'
PHRASE = ('Esta es la prueba de detención de voz. La lectura continuará unos segundos para que puedas pulsar '
          'el botón de detener. Cuando lo pulses, la voz debe terminar de inmediato y no continuar con el '
          'resto de esta frase. No se modificará ningún archivo.')


def prepare(campaign):
    root=campaign/'t2';root.mkdir(mode=0o700)
    engine=Path(json.loads(private_file(campaign/'verified.json',campaign,16384).read_text())['engine_profile'])
    env,digest=verify(root,engine)
    context=PreparedBackend(root,engine,env,'prepare',reply=PHRASE,auto=False)
    try:
        alternative=api(context.url,context.token,'/api/session',{'title':'F4 voz · Sesión alternativa'})['id']
        result=api(context.url,context.token,'/api/chat/complete',{'session_id':context.session,'message':'Prueba sintética de detención de voz, sin herramientas.'})
        assert result and context.mock.calls==1 and not children(context.app.pid)
        assert context.status()['fast']['state']=='cold'
        session=context.session
    finally:context.close()
    resources=root/'resources';resources.mkdir(mode=0o700)
    shutil.copytree(REPO/'qml',resources/'qml');(resources/'assets').symlink_to(REPO/'assets',target_is_directory=True)
    main=resources/'qml/Main.qml';source=main.read_text();assert 'title: "KDE Assistant v2"' in source
    main.write_text(source.replace('title: "KDE Assistant v2"','title: "KDE Assistant · Voz F4 privada"',1))
    write_private(root/'mock-reply.json',{'text':PHRASE,'delay':0})
    cfg=private_file(root/'config/kde-assistant/config.json',root,262144)
    write_private(root/'scenario.json',dict(schema=1,purpose=PURPOSE,phrase=PHRASE,nonce=__import__('uuid').uuid4().hex,
        binary_sha256=digest,config_sha256=hashlib.sha256(cfg.read_bytes()).hexdigest(),engine_profile=str(engine),
        session=session,alternative_session=alternative,physical_audio=False,microphone=False,auto_read=False))
    print('PREPARED_T2',root,flush=True)


def launch(root,receipt,trial=2):
    if trial not in (2,3,4):raise ValueError('Only T2, T3 or T4 supported')
    purpose={2:PURPOSE,3:REPLAY_PURPOSE,4:SESSION_PURPOSE}[trial]
    scenario_name='scenario.json' if trial==2 else f'scenario-t{trial}.json'
    env,scenario,marker=admit(root,receipt,purpose=purpose,phrase=PHRASE,trial=trial,scenario_name=scenario_name)
    engine=Path(scenario['engine_profile']);env,digest=verify(root,engine)
    if digest!=scenario['binary_sha256']:raise ValueError('Binary changed')
    display=os.environ.get('WAYLAND_DISPLAY','');runtime=os.environ.get('XDG_RUNTIME_DIR','')
    if not display or not runtime:raise ValueError('Wayland display unavailable')
    if not Path(display).is_absolute():display=str(Path(runtime)/display)
    if not Path(display).is_socket():raise ValueError('Expected existing Wayland socket')
    require_private(root,env)
    write_private(marker,{'purpose':purpose,'trial':trial,'consumed':True})
    live={**env,'PIPEWIRE_REMOTE':str(live_remote()),'WAYLAND_DISPLAY':display,'QT_QPA_PLATFORM':'wayland',
          'KDE_ASSISTANT_DATA_DIR':str(root/'resources')}
    # DBus remains the private absent bus. Only Wayland and the consented output
    # socket are shared; no activation of the personal application/service.
    def reply():return json.loads(private_file(root/'mock-reply.json',root,32768).read_text())
    context=PreparedBackend(root,engine,live,'live' if trial==2 else f'live-t{trial}',scenario['session'],ui=True,reply=reply,auto=False)
    events=[];previous=None;speaking=False;voice_pids=set();observed=False
    started=time.monotonic();hold_until=started+300
    result={'trial':trial,'technical':'PENDING','human':'PENDING','microphone':False,'raw_audio_saved':False,
            'backend_pid':context.app.pid,'session':context.session,
            'alternative_session':scenario['alternative_session']}
    observation_root=root if trial==2 else root/f't{trial}-observer'
    observation_root.mkdir(mode=0o700,exist_ok=True)
    def store():save_observation(observation_root,{**result,'transitions':events,'ended':False})
    try:
        initial_ui=wait_for(lambda:set(children(context.app.pid)),10)
        result['ui_pids']=sorted(initial_ui)
        instruction={2:'al oír la voz, Detener.',3:'deja terminar la frase completa.',4:'al oír la voz, cambia a «F4 voz · Sesión alternativa», sin pulsar Detener.'}[trial]
        print('GUI_READY: pulsa Leer una vez y '+instruction+' No repitas la lectura.',flush=True)
        while context.app.poll() is None and time.monotonic()<hold_until:
            state=context.status();key=(state['state'],None if not state.get('identity') else state['identity']['generation'])
            if key!=previous:
                previous=key;events.append({'state':key[0],'generation':key[1],'ms':(time.monotonic()-started)*1000})
                store();print('STATE',state['state'],flush=True)
            if state['state']=='speaking' and not observed:
                if trial==4 and state['identity']['session_id']!=scenario['session']:
                    raise ValueError('Speaking session does not match T4')
                speaking=True;voice_pids.update(set(children(context.app.pid))-initial_ui)
                if state['fast']['worker_pid']:voice_pids.add(state['fast']['worker_pid'])
            if speaking and not observed and state['state']=='idle':
                observed=True;cancelled=state.get('identity') is None and state['fast']['worker_pid'] is None
                natural=state.get('identity') is not None and state['fast']['completed']==1
                accepted=cancelled if trial in (2,4) else natural
                if cancelled:wait_for(lambda:all(not Path(f'/proc/{pid}').exists() for pid in voice_pids),5)
                result.update(technical='PASS' if accepted else 'FAILED',cancel_observed=cancelled,
                              voice_pids=sorted(voice_pids),voice_processes_reaped=cancelled,
                              natural_completion=natural,fast_completed=state['fast']['completed'],
                              same_backend_as_t2=trial==2,
                              failure=None if accepted else 'Playback completion does not match trial')
                store();print(f'T{trial}_CANCELLATION_OBSERVED' if trial in (2,4) and cancelled else 'T3_COMPLETED' if trial==3 and natural else 'UNEXPECTED_COMPLETION',flush=True)
                hold_until=time.monotonic()+300
            if state['state']=='error' and not observed:
                result.update(technical='FAILED',failure='Voice engine error');store();break
            close=root/'close-gui.json'
            if close.exists():
                value=json.loads(private_file(close,root,4096).read_text())
                if value=={'nonce':scenario['nonce'],'close':True}:break
            if initial_ui and all(not Path(f'/proc/{pid}').exists() for pid in initial_ui):break
            time.sleep(.1)
    finally:
        context.close();result.update(ended=True,children_reaped=True,transitions=events)
        save_observation(observation_root,result)
    print('Private GUI closed. No further playback was requested by this helper.',flush=True)


def main():
    p=argparse.ArgumentParser(description=__doc__);m=p.add_mutually_exclusive_group(required=True)
    m.add_argument('--prepare',action='store_true');m.add_argument('--launch',action='store_true')
    p.add_argument('--profile',type=Path,required=True);p.add_argument('--receipt',type=Path)
    p.add_argument('--trial',type=int,choices=(2,3,4),default=2)
    a=p.parse_args();root=profile_path(a.profile)
    if a.prepare:prepare(root)
    else:
        if not a.receipt:p.error('--receipt required')
        launch(root,a.receipt,a.trial)
if __name__=='__main__':main()
