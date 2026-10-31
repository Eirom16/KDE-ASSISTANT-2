#!/usr/bin/env python3
"""Prepare/run ONE separately consented T1 playback, with an isolated headless backend.

No microphone, UI clicks, provider requests, downloads, audio files or global changes.
Other human trials require their own preparation and permission.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private
from f3_runtime import wait_for, children
from live_f4_ptt import application, api, attach_log, save_observation
from compare_f4_spanish import REPO, baseline, private_file, profile_path, write_private
from record_f4_spanish import live_remote

PHRASE = 'Hola. Soy KDE Assistant y esta es una prueba de voz en español.'
PURPOSE = 'live_f4r3_tts_t1'


def prepare(engine):
    engine=profile_path(engine)
    root=engine/('live-t1-'+str(time.time_ns()));root.mkdir(mode=0o700)
    env=private_env(root);require_private(root,env);baseline(env)
    app=application(root,env,False);thread=None
    try:
        url,token,thread=attach_log(app,root,'prepare.log')
        cfg=api(url,token,'/api/config')
        cfg['speech'].update(piper_runtime=str(engine/'venv'),piper_model=str(engine/'voice/es_ES-sharvard-medium.onnx'),piper_speaker=0,piper_length_scale=1.0,
            stt_model='/nonexistent/f4r3-microphone-disabled.bin',stt_engine='whisper',auto_speak=False,
            auto_listen=False,wake_word_enabled=False,chimes_enabled=False)
        cfg['memory'].update(enabled=False,auto_summarize=False)
        cfg['ai'].update(provider='custom',base_url='http://127.0.0.1:1/v1',model='synthetic-not-used',api_key='synthetic-not-used',
                         provider_keys={},provider_base_urls={},provider_models={},enable_tool_calling=False)
        api(url,token,'/api/config',cfg)
        session=api(url,token,'/api/session',{'title':'Prueba privada TTS T1'})
        assert not children(app.pid)
        write_private(root/'scenario.json',dict(purpose=PURPOSE,phrase=PHRASE,session=session['id'],engine_profile=str(engine),
                      microphone=False,raw_audio_saved=False,provider_requests=0,launch_authorized=False))
    finally:
        app.terminate();app.wait(timeout=15)
        if thread:thread.join(timeout=2)
        app.stdout.close()
    print(root,flush=True)


def admit(root,receipt):
    env=private_env(root);require_private(root,env)
    permission=json.loads(private_file(receipt,root,16384).read_text())
    scenario=json.loads(private_file(root/'scenario.json',root,16384).read_text())
    if scenario['purpose']!=PURPOSE or scenario['phrase']!=PHRASE:
        raise ValueError('Unexpected T1 scenario')
    if permission.get('purpose')!=PURPOSE or permission.get('phrase')!=PHRASE or not permission.get('audio_authorization') or not permission.get('ready_reference'):
        raise ValueError('T1 requires separate explicit playback authorization and readiness')
    marker=receipt.with_suffix('.used')
    if marker.exists():raise ValueError('One-use T1 permission already consumed')
    cfg=json.loads(private_file(root/'config/kde-assistant/config.json',root,262144).read_text())
    engine=Path(scenario['engine_profile'])
    required=dict(piper_runtime=str(engine/'venv'),piper_model=str(engine/'voice/es_ES-sharvard-medium.onnx'),piper_speaker=0,
                  piper_length_scale=1.0,stt_model='/nonexistent/f4r3-microphone-disabled.bin',stt_engine='whisper',
                  auto_speak=False,auto_listen=False,wake_word_enabled=False,chimes_enabled=False)
    if any(cfg['speech'].get(k)!=v for k,v in required.items()):raise ValueError('T1 speech configuration changed')
    if cfg['ai']['base_url']!='http://127.0.0.1:1/v1' or cfg['ai']['enable_tool_calling'] or cfg['memory']['enabled'] or cfg['memory']['auto_summarize']:
        raise ValueError('T1 requires inactive synthetic provider and no tools/memory')
    return env,marker,scenario


def launch(root,receipt):
    env,marker,scenario=admit(root,receipt);baseline(env)
    remote=live_remote()  # Host audio socket accessed only AFTER this T1 consent check.
    require_private(root,env)
    write_private(marker,dict(consumed=True,purpose=PURPOSE))
    live={**env,'PIPEWIRE_REMOTE':str(remote)}
    app=None;thread=None;owned=set();transitions=[];previous=None;finished=False
    started=time.monotonic()
    def observation(ended):
        return dict(purpose=PURPOSE,transitions=transitions,ended=ended,playback_completed=finished,
                    child_pids=sorted(owned),children_reaped=ended and all(not Path(f'/proc/{pid}').exists() for pid in owned),
                    human_confirmation='PENDING_USER_REPORT',audible_latency='NOT_MEASURABLE_WITHOUT_HUMAN_REPORT',
                    raw_audio_saved=False,microphone=False)
    try:
        print('T1: se generará y reproducirá una sola frase. No se modifica el volumen.',flush=True)
        app=application(root,live,False)
        url,token,thread=attach_log(app,root,'live.log')
        selected=api(url,token,'/api/voice/session',{'session_id':scenario['session']})
        api(url,token,'/api/speak',{'session_id':scenario['session'],'scope':selected['scope'],'text':PHRASE})
        while time.monotonic()-started<60:
            status=api(url,token,'/api/voice/status');owned.update(children(app.pid))
            if status['state']!=previous:
                previous=status['state'];transitions.append(dict(state=previous,elapsed_ms=(time.monotonic()-started)*1000))
                save_observation(root,observation(False));print('Estado:',previous,flush=True)
            if previous=='error':raise RuntimeError(status.get('error','TTS failed'))
            if previous=='idle':finished=True;break
            time.sleep(.05)
        if not finished:raise RuntimeError('T1 did not finish within safety deadline')
        wait_for(lambda:not children(app.pid),5)
        assert api(url,token,'/api/messages?session_id='+scenario['session'])==[]
    finally:
        if app:
            if app.poll() is None:app.terminate()
            try:app.wait(timeout=15)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
            if thread:thread.join(timeout=2)
            app.stdout.close()
        save_observation(root,observation(True))
    print('T1 terminó. Falta confirmar si escuchaste la frase, su claridad y volumen.',flush=True)


def main():
    p=argparse.ArgumentParser(description=__doc__);m=p.add_mutually_exclusive_group(required=True)
    m.add_argument('--prepare',action='store_true');m.add_argument('--launch',action='store_true')
    p.add_argument('--engine-profile',type=Path);p.add_argument('--profile',type=Path);p.add_argument('--receipt',type=Path)
    args=p.parse_args()
    if args.prepare:
        if args.engine_profile is None:p.error('--engine-profile required')
        prepare(args.engine_profile)
    else:
        if args.profile is None or args.receipt is None:p.error('--profile and --receipt required')
        launch(profile_path(args.profile),args.receipt)


if __name__=='__main__':main()
