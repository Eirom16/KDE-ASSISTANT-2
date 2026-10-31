#!/usr/bin/env python3
"""T5 private GUI fixture: one human Send, one mock reply, no automatic prompt."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import time
import uuid
sys.dont_write_bytecode = True
from compare_f4_spanish import REPO, private_file, profile_path, write_private
from f4_safety import require_private
from f3_runtime import children, wait_for
from live_f4_fast_tts import PreparedBackend, admit, verify
from live_f4_ptt import api, save_observation
from record_f4_spanish import live_remote

PURPOSE = 'f4_r5_human_t5_mock_auto_v1'
PROMPT = 'Prueba de autolectura.'
PHRASE = ('Esta es una respuesta de prueba. La lectura automática comienza '
          'cuando termina la respuesta del asistente.')


def prepare(campaign):
    root = campaign/'t5'; root.mkdir(mode=0o700)
    engine = Path(json.loads(private_file(campaign/'verified.json', campaign, 16384).read_text())['engine_profile'])
    env, digest = verify(root, engine)
    context = PreparedBackend(root, engine, env, 'prepare', reply=PHRASE, auto=False)
    try:
        assert context.mock.calls == 0 and not children(context.app.pid)
        session = context.session
        assert api(context.url, context.token, '/api/messages?session_id='+session) == []
    finally:
        context.close()
    cfg_path = private_file(root/'config/kde-assistant/config.json', root, 262144)
    cfg = json.loads(cfg_path.read_text()); cfg['speech']['auto_speak'] = True
    cfg_path.write_text(json.dumps(cfg, indent=2)+'\n')
    resources = root/'resources'; resources.mkdir(mode=0o700)
    shutil.copytree(REPO/'qml', resources/'qml')
    (resources/'assets').symlink_to(REPO/'assets', target_is_directory=True)
    main = resources/'qml/Main.qml'; source = main.read_text()
    assert 'title: "KDE Assistant v2"' in source
    main.write_text(source.replace('title: "KDE Assistant v2"', 'title: "KDE Assistant · Autolectura F4 privada"', 1))
    write_private(root/'scenario.json', dict(schema=1, purpose=PURPOSE, phrase=PHRASE,
        prompt=PROMPT, nonce=uuid.uuid4().hex, binary_sha256=digest,
        config_sha256=hashlib.sha256(cfg_path.read_bytes()).hexdigest(),
        engine_profile=str(engine), session=session, delay_secs=12,
        microphone=False, physical_audio=False))
    print('T5_PREPARED: empty private conversation, no LLM request or playback.', flush=True)


def launch(root, receipt):
    env, scenario, marker = admit(root, receipt, purpose=PURPOSE, phrase=PHRASE, trial=5)
    engine = Path(scenario['engine_profile']); env, digest = verify(root, engine)
    if digest != scenario['binary_sha256'] or scenario.get('prompt') != PROMPT or scenario.get('delay_secs') != 12:
        raise ValueError('T5 scenario changed')
    display = os.environ.get('WAYLAND_DISPLAY', ''); runtime = os.environ.get('XDG_RUNTIME_DIR', '')
    if not display or not runtime: raise ValueError('Wayland unavailable')
    if not Path(display).is_absolute(): display = str(Path(runtime)/display)
    if not Path(display).is_socket(): raise ValueError('Expected Wayland socket')
    require_private(root, env)
    write_private(marker, dict(purpose=PURPOSE, trial=5, consumed=True))
    live = {**env, 'PIPEWIRE_REMOTE':str(live_remote()), 'WAYLAND_DISPLAY':display,
            'QT_QPA_PLATFORM':'wayland', 'KDE_ASSISTANT_DATA_DIR':str(root/'resources')}
    context = PreparedBackend(root, engine, live, 'live', scenario['session'], ui=True,
        reply=lambda:{'text':PHRASE, 'delay':12}, auto=True)
    result = dict(trial=5, technical='PENDING', human='PENDING', microphone=False,
                  raw_audio_saved=False, backend_pid=context.app.pid,
                  ready_before_synthesis=False, events=[])
    observed_pids = set(); initial_ui = set(); start = time.monotonic()
    previous = None; speaking = False; last_identity = None
    def messages(): return api(context.url, context.token, '/api/messages?session_id='+scenario['session'])
    def content_ok(values):
        return (len(values)==2 and values[0].get('role')=='user' and values[0].get('content')==PROMPT
                and values[1].get('role')=='assistant' and values[1].get('content')==PHRASE
                and all(not item.get('tool_calls') and not item.get('tool_call_id') for item in values))
    try:
        initial_ui = wait_for(lambda:set(children(context.app.pid)), 10)
        result['ui_pids'] = sorted(initial_ui)
        assert context.mock.calls == 0 and messages() == []
        print('T5_GUI_READY: escribe «Prueba de autolectura.» y pulsa Enviar una vez. No pulses Leer; deja terminar la voz.', flush=True)
        while context.app.poll() is None and time.monotonic()-start < 300:
            state = context.status(); fast = state['fast']; elapsed = (time.monotonic()-start)*1000
            observed_pids.update(set(children(context.app.pid))-initial_ui)
            if fast['worker_pid']: observed_pids.add(fast['worker_pid'])
            key = (state['state'], fast['state'], context.mock.calls)
            if key != previous:
                previous = key
                result['events'].append(dict(state=key[0], fast_state=key[1], mock_calls=key[2], ms=elapsed))
                save_observation(root, result); print('STATE', key[0], 'FAST', key[1], flush=True)
            if context.mock.calls > 1: raise ValueError('More than one provider request')
            if fast['state']=='ready' and fast['completed']==0 and state['state']=='idle' and context.mock.calls==1:
                if not result['ready_before_synthesis']:
                    result.update(ready_before_synthesis=True, ready_observed_ms=elapsed)
            if state['state']=='error': raise RuntimeError('Voice engine error')
            if state['state']=='speaking':
                if not speaking:
                    assert state['identity']['session_id']==scenario['session'] and state['identity']['turn_id']
                    result['persisted_before_speaking'] = content_ok(messages())
                    result['speaking_observed_ms'] = elapsed
                    last_identity = state['identity']; speaking = True
            if speaking and state['state']=='idle':
                result.update(mock_calls=context.mock.calls, completed=fast['completed'],
                    prepared_reuses=fast['reuses'], turn_identity=last_identity,
                    identity_preserved=state['identity']==last_identity,
                    persisted_after_speaking=content_ok(messages()), finished_ms=elapsed)
                assert (result['ready_before_synthesis'] and result['persisted_before_speaking']
                    and result['persisted_after_speaking'] and result['identity_preserved']
                    and result['mock_calls']==1 and result['completed']==1 and result['prepared_reuses']==1)
                result['technical']='PASS'
                print('T5_AUTO_READ_COMPLETED: one mock request, prepared worker reused, persisted text unchanged.', flush=True)
                break
            time.sleep(.05)
        else:
            raise TimeoutError('T5 did not complete')
    except BaseException as error:
        result.update(technical='FAILED', failure_type=type(error).__name__)
        raise
    finally:
        context.close()
        result.update(ended=True, children_reaped=True, voice_pids=sorted(observed_pids))
        assert all(not Path('/proc',str(pid)).exists() for pid in observed_pids)
        save_observation(root, result)
    print('T5 closed; awaiting human confirmation. No other trial authorized.', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare', action='store_true'); mode.add_argument('--launch', action='store_true')
    parser.add_argument('--profile', type=Path, required=True); parser.add_argument('--receipt', type=Path)
    args = parser.parse_args(); root = profile_path(args.profile)
    if args.prepare: prepare(root)
    else:
        if not args.receipt: parser.error('--receipt required')
        launch(root, args.receipt)

if __name__ == '__main__': main()
