#!/usr/bin/env python3
"""Human F4 acceptance: prepare silently; T1 needs its own one-use consent.

Reuses the R3/R5 backend, isolation, API and consent helpers. No microphone,
remote provider, downloads, shell, PCM files or changes to the personal profile.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private
from f3_runtime import children, wait_for
from live_f4_ptt import application, api, attach_log
from compare_f4_spanish import REPO, baseline, private_file, profile_path, write_private
from record_f4_spanish import live_remote
from live_f4_tts import PHRASE
from benchmark_f4 import compilers

PURPOSE = 'f4_r5_human_t1_prepared_manual_v1'


def binary_hash():
    expected = json.loads((REPO/'docs/recovery/F4-R5-EVIDENCE/final-source.json').read_text())['final_binary_sha256']
    actual = hashlib.sha256((REPO/'target/debug/kde-assistant').read_bytes()).hexdigest()
    if actual != expected:
        raise ValueError('Binary differs from the accepted R5 build')
    return actual


def verify(root, engine):
    env = private_env(root); require_private(root, env); baseline(env)
    if compilers():
        raise ValueError('Wait for compilation to finish')
    digest = binary_hash()
    result = subprocess.run([str(REPO/'target/debug/kde-assistant'), '--voice-piper-check',
        str(engine/'venv'), str(engine/'voice/es_ES-sharvard-medium.onnx')],
        cwd=root, env=env, capture_output=True, timeout=15)
    if result.returncode or json.loads(result.stdout) is not None:
        raise ValueError('Piper resources did not pass full integrity validation')
    return env, digest


class HeldMock:
    """Pending prewarm mock, or an explicit synthetic reply for GUI fixtures."""
    def __init__(self, reply=None):
        self.stop = threading.Event(); self.calls = 0
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass
            def do_POST(self):
                size = int(self.headers.get('Content-Length', '0'))
                if self.path != '/v1/chat/completions' or not 0 < size <= 131072:
                    self.send_error(400); return
                body = json.loads(self.rfile.read(size))
                if body.get('tools') or body.get('model') != 'synthetic-f4-acceptance':
                    self.send_error(400); return
                owner.calls += 1
                self.send_response(200); self.send_header('Content-Type', 'text/event-stream')
                self.send_header('Connection', 'close'); self.end_headers()
                try:
                    self.wfile.write(b'data: {"choices":[{"delta":{"content":""},"finish_reason":null}]}\n\n')
                    self.wfile.flush()
                    if reply is None:
                        owner.stop.wait(90)
                        # T1: deliberately no final text, finish_reason or DONE.
                    else:
                        value = reply() if callable(reply) else {'text':reply, 'delay':0}
                        if not isinstance(value.get('text'), str) or not 0 < len(value['text'].encode()) <= 16000 or not 0 <= value.get('delay',0) <= 15:
                            return
                        if owner.stop.wait(value.get('delay',0)): return
                        data = {'choices':[{'delta':{'content':value['text']}, 'finish_reason':'stop'}]}
                        self.wfile.write(('data: '+json.dumps(data)+'\n\ndata: [DONE]\n\n').encode()); self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass
            def do_GET(self):
                if self.path != '/v1/models': self.send_error(404); return
                data = json.dumps({'data':[{'id':'synthetic-f4-acceptance'}]}).encode()
                self.send_response(200); self.send_header('Content-Type','application/json')
                self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
    def close(self):
        self.stop.set(); self.server.shutdown(); self.server.server_close(); self.thread.join(3)


class PreparedBackend:
    def __init__(self, root, engine, env, name, session=None, *, ui=False, reply=None, auto=True):
        # env may carry the host output socket only after the separate consent gate.
        guarded = private_env(root); require_private(root, guarded); baseline(guarded); binary_hash()
        self.root = root; self.engine = engine; self.mock = HeldMock(reply)
        self.app = None; self.log_thread = None; self.ask_thread = None
        self.turn = None; self.owned = set(); self.chat_http = None
        try:
            self.app = application(root, env, ui)
            self.url, self.token, self.log_thread = attach_log(self.app, root, name+'.log')
            cfg = api(self.url, self.token, '/api/config')
            cfg['speech'].update(piper_runtime=str(engine/'venv'),
                piper_model=str(engine/'voice/es_ES-sharvard-medium.onnx'), piper_speaker=0,
                piper_length_scale=1.0, piper_fast=True, piper_fast_ttl_secs=30,
                stt_engine='whisper', stt_backend='local', stt_model='/nonexistent/f4-acceptance-no-microphone.bin',
                auto_speak=auto, auto_listen=False, wake_word_enabled=False, chimes_enabled=False)
            cfg['memory'].update(enabled=False, auto_summarize=False)
            cfg['character']['enabled'] = False
            cfg['ui'].update(close_to_tray=False, always_on_top=False)
            cfg['ai'].update(provider='custom', base_url=f'http://127.0.0.1:{self.mock.server.server_port}/v1',
                model='synthetic-f4-acceptance', api_key='synthetic-unused', provider_keys={},
                provider_base_urls={}, provider_models={}, provider_endpoint_approvals={}, enable_tool_calling=False)
            api(self.url, self.token, '/api/config', cfg)
            self.session = session or api(self.url, self.token, '/api/session', {'title':'F4 voz privada T1'})['id']
            self.scope = api(self.url, self.token, '/api/voice/session', {'session_id':self.session})['scope']
            if not ui: assert not children(self.app.pid), 'Enabling Fast Voice must not preload'
            else: assert self.status()['fast']['worker_pid'] is None, 'GUI startup must not preload'
        except BaseException:
            self.close(); raise
    def status(self):
        self.owned.update(children(self.app.pid))
        return api(self.url, self.token, '/api/voice/status')
    def warm(self):
        def ask():
            request = urllib.request.Request(self.url+'/api/chat/complete',
                data=json.dumps({'session_id':self.session, 'voice_scope':self.scope,
                                 'message':'Preparación sintética; no realizar acciones.'}).encode(),
                headers={'Authorization':'Bearer '+self.token, 'Content-Type':'application/json'})
            try:
                with urllib.request.urlopen(request, timeout=80) as response:
                    self.chat_http = response.status
            except urllib.error.HTTPError as error:
                self.chat_http = error.code; error.read()
            except OSError:
                self.chat_http = 'connection_closed'
        self.ask_thread = threading.Thread(target=ask, daemon=True); self.ask_thread.start()
        start = time.monotonic()
        while time.monotonic()-start < 30:
            state = self.status(); fast = state['fast']
            if state['state'] != 'idle' or fast['state'] in ['error', 'fallback']:
                raise RuntimeError('Silent preparation failed or attempted voice activity')
            if fast['state'] == 'ready':
                assert fast['worker_pid'] and fast['completed'] == 0 and fast['reuses'] == 0
                assert self.mock.calls == 1
                self.turn = api(self.url, self.token, '/api/chat/turn?session_id='+self.session)['turn_id']
                return dict(ready=True, ready_ms=fast['ready_ms'], worker_pid=fast['worker_pid'],
                            generation=fast['generation'], completed=0, mock_calls=1)
            time.sleep(.05)
        raise TimeoutError('ONNX Ready not confirmed; no playback admitted')
    def close(self):
        if self.app:
            self.owned.update(children(self.app.pid))
            if self.app.poll() is None:
                try:
                    turn = self.turn or api(self.url, self.token, '/api/chat/turn?session_id='+self.session).get('turn_id')
                    if turn:
                        api(self.url, self.token, '/api/chat/cancel', {'session_id':self.session, 'turn_id':turn})
                except (AttributeError, OSError, ValueError):
                    pass
                self.app.terminate()
                try: self.app.wait(timeout=15)
                except subprocess.TimeoutExpired: self.app.kill(); self.app.wait(timeout=5)
            if self.log_thread: self.log_thread.join(3)
            if self.app.stdout: self.app.stdout.close()
        self.mock.close()
        if self.ask_thread: self.ask_thread.join(3)
        wait_for(lambda:all(not Path(f'/proc/{pid}').exists() for pid in self.owned), 5)


def prepare(campaign):
    root = campaign/'t1'; root.mkdir(mode=0o700)
    verified = json.loads(private_file(campaign/'verified.json', campaign, 16384).read_text())
    engine = Path(verified['engine_profile'])
    env, digest = verify(root, engine)
    context = PreparedBackend(root, engine, env, 'prepare')
    try:
        observed = context.warm(); session = context.session
    finally:
        context.close()
    assert context.chat_http == 409 and not Path(f"/proc/{observed['worker_pid']}").exists()
    cfg = private_file(root/'config/kde-assistant/config.json', root, 262144)
    scenario = dict(schema=1, purpose=PURPOSE, nonce=str(uuid.uuid4()), phrase=PHRASE,
        session=session, engine_profile=str(engine), binary_sha256=digest,
        config_sha256=hashlib.sha256(cfg.read_bytes()).hexdigest(), preparation=observed,
        children_reaped=True, microphone=False, physical_audio=False, launch_authorized=False)
    write_private(root/'scenario.json', scenario)
    print('PREPARED', root, flush=True)
    return scenario


def admit(root, receipt, *, purpose=PURPOSE, phrase=PHRASE, trial=1, scenario_name='scenario.json'):
    env = private_env(root); require_private(root, env)
    scenario = json.loads(private_file(root/scenario_name, root, 16384).read_text())
    permission = json.loads(private_file(receipt, root, 16384).read_text())
    if scenario.get('schema') != 1 or scenario.get('purpose') != purpose or scenario.get('phrase') != phrase:
        raise ValueError('Wrong acceptance scenario')
    for key in ['purpose', 'nonce', 'phrase', 'binary_sha256']:
        if permission.get(key) != scenario.get(key):
            raise ValueError('Permission does not match this R5 T1 scenario')
    if not permission.get('audio_authorization') or not permission.get('ready_reference'):
        raise ValueError('Separate T1 speaker authorization and readiness required')
    if permission.get('trial') != trial or permission.get('microphone') is not False:
        raise ValueError('Only the specifically consented playback without microphone is permitted')
    marker = receipt.with_suffix('.used')
    if marker.exists() or (root/'play-once.json').exists():
        raise ValueError('Permission/gate already used or created before admission')
    cfg = private_file(root/'config/kde-assistant/config.json', root, 262144)
    if hashlib.sha256(cfg.read_bytes()).hexdigest() != scenario['config_sha256']:
        raise ValueError('Prepared configuration changed')
    return env, scenario, marker


def launch(root, receipt):
    env, scenario, marker = admit(root, receipt)
    engine = Path(scenario['engine_profile']); env, digest = verify(root, engine)
    if digest != scenario['binary_sha256']:
        raise ValueError('Prepared binary changed')
    write_private(marker, {'purpose':PURPOSE, 'trial':1, 'consumed':True})
    remote = live_remote()  # No host socket access before the consent gate.
    live = {**env, 'PIPEWIRE_REMOTE':str(remote)}
    context = None; result = dict(trial=1, technical='FAILED', human='PENDING', microphone=False,
        raw_audio_saved=False, physical_playbacks_requested=0)
    try:
        context = PreparedBackend(root, engine, live, 'live', scenario['session'])
        result['preparation'] = context.warm()
        print('READY_FOR_ONE_PLAYBACK', flush=True)
        # Operator announces Ready, then opens this private one-shot gate.
        gate = root/'play-once.json'; deadline = time.monotonic()+15
        while not gate.exists() and time.monotonic() < deadline: time.sleep(.05)
        if not gate.exists(): raise TimeoutError('Play gate not opened in time; no audio')
        go = json.loads(private_file(gate, root, 4096).read_text())
        if go != {'trial':1, 'nonce':scenario['nonce']}:
            raise ValueError('Wrong playback gate')
        state = context.status()
        if state['fast']['state'] != 'ready' or state['fast']['generation'] != result['preparation']['generation']:
            raise ValueError('Prepared worker expired or changed; no audio')
        start = time.monotonic(); previous = None; seen_speaking = False; events = []
        api(context.url, context.token, '/api/speak', {'session_id':context.session,
            'scope':context.scope, 'text':PHRASE})
        result['physical_playbacks_requested'] = 1
        while time.monotonic()-start < 30:
            state = context.status()
            if state['state'] != previous:
                previous = state['state']; events.append({'state':previous, 'ms':(time.monotonic()-start)*1000})
                print('STATE', previous, flush=True)
            if previous == 'error': raise RuntimeError('T1 synthesis/playback failed')
            if previous == 'speaking': seen_speaking = True
            if previous == 'idle' and seen_speaking: break
            time.sleep(.05)
        else: raise TimeoutError('T1 playback did not finish')
        assert state['fast']['completed'] == 1 and state['fast']['reuses'] == 1
        assert state['fast']['worker_pid'] == result['preparation']['worker_pid']
        result.update(technical='PASS', transitions=events, prepared_worker_reused=True,
                      mock_calls=context.mock.calls, audible_latency='NOT_MEASURED')
    finally:
        if context:
            context.close(); result['children_reaped'] = True
        write_private(root/'technical-t1.json', result)
    print('T1 finished; awaiting human confirmation. No other trial authorized.', flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare', action='store_true'); mode.add_argument('--launch', action='store_true')
    parser.add_argument('--profile', type=Path, required=True); parser.add_argument('--receipt', type=Path)
    args = parser.parse_args(); root = profile_path(args.profile)
    if args.prepare: prepare(root)
    else:
        if args.receipt is None: parser.error('--receipt required for a one-use T1 authorization')
        launch(root, args.receipt)

if __name__ == '__main__': main()
