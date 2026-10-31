#!/usr/bin/env python3
"""Prepare an isolated manual PTT test; live launch requires separate consent.

No synthetic clicks, automatic recording, downloads or personal application data.
--prepare starts only a headless private Backend with unreachable audio, then
stops it. --launch requires a one-use receipt recording explicit permission to
use the evaluation Vosk artifacts for PTT AND microphone/ready confirmations.
Only the human can press Grabar/Detener in the application's own window.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from compare_f4_spanish import REPO, baseline, private_file, profile_path, write_private
from f3_runtime import children, wait_for
from f4_safety import private_env, require_private
from record_f4_spanish import live_remote


def application(root, env, ui):
    command = [str(REPO/'target/debug/kde-assistant'), '--no-shortcuts', '--http-port', '0']
    if not ui:
        command.append('--ui-off')
    return subprocess.Popen(command, cwd=root, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True, umask=0o077)


def api(base, token, path, body=None):
    request = urllib.request.Request(base+path,
        data=json.dumps(body).encode() if body is not None else None,
        headers={'Authorization': 'Bearer '+token, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=5) as response:
        return json.load(response)


def attach_log(app, root, name):
    # The owned process log stays private and bounded; never publish bearer/data.
    log = root/name
    state = {'url': None}
    def drain():
        count = 0
        with log.open('x') as output:
            log.chmod(0o600)
            for line in app.stdout:
                match = re.search(r'Núcleo de texto listo en (http://127.0.0.1:\d+)', line)
                if match:
                    state['url'] = match[1]
                if count < 262144:
                    output.write(line[:262144-count])
                    output.flush()
                    count += len(line)
    thread = threading.Thread(target=drain, daemon=True)
    thread.start()
    url = wait_for(lambda: state['url'] or (app.poll() is not None and 'FAILED'), 20)
    if url == 'FAILED':
        raise RuntimeError('Private Backend failed; inspect only its private log')
    token = (root/'config/kde-assistant/server.token').read_text().strip()
    return url, token, thread


def prepare(library, model):
    root = Path(tempfile.mkdtemp(prefix='kda-f4r2-ptt-', dir='/var/tmp'))
    env = private_env(root)
    require_private(root, env)
    baseline(env)
    app = application(root, env, False)
    try:
        _, _, thread = attach_log(app, root, 'prepare.log')
        assert not children(app.pid), 'No voice/UI children allowed during preparation'
    finally:
        app.terminate()
        app.wait(timeout=15)
        app.stdout.close()
    thread.join(timeout=2)
    config = root/'config/kde-assistant/config.json'
    cfg = json.loads(private_file(config, root, 262144).read_text())
    cfg['speech'].update(stt_engine='vosk', vosk_library=str(library), vosk_model=str(model),
        stt_language='es', stt_backend='local', auto_speak=False, auto_listen=False,
        chimes_enabled=False, wake_word_enabled=False, max_recording_secs=8,
        vad_min_record_ms=8000, vad_silence_ms=10000, mic_device='')
    cfg['ui'].update(close_to_tray=False, always_on_top=False)
    cfg['character']['enabled'] = False
    cfg['memory'].update(enabled=False, auto_summarize=False)
    cfg['ai'].update(provider='custom', base_url='http://127.0.0.1:1/v1',
        api_key='synthetic-f4r2-local-only', provider_keys={}, provider_base_urls={},
        provider_models={}, provider_endpoint_approvals={}, model='mock-f4r2',
        enable_tool_calling=False)
    config.write_text(json.dumps(cfg, indent=2)+'\n')
    resources = root/'resources'
    resources.mkdir(mode=0o700)
    shutil.copytree(REPO/'qml', resources/'qml')
    (resources/'assets').symlink_to(REPO/'assets', target_is_directory=True)
    main = resources/'qml/Main.qml'
    original = main.read_text()
    # Change only the private window title, to make the test instance recognisable.
    if 'title: "KDE Assistant v2"' not in original:
        raise RuntimeError('Inspect current window title before preparing live test')
    main.write_text(original.replace('title: "KDE Assistant v2"',
        'title: "KDE Assistant · PTT aislado F4-R2"', 1))
    write_private(root/'scenario.json', dict(purpose='live_f4r2_ptt', max_capture_secs=8,
        phrase='Hola, ¿qué hora es?', microphone='NOT_ACTIVATED', launch_authorized=False,
        library=str(library), model=str(model), llm='SYNTHETIC_LOOPBACK_ONLY',
        retention='No raw audio saved; private test profile removed at evaluation closure'))
    print(root)


def admit(root, receipt):
    env = private_env(root)
    require_private(root, env)
    permission = json.loads(private_file(receipt, root, 16384).read_text())
    if permission.get('purpose') != 'live_f4r2_ptt' or permission.get('max_capture_secs') != 8:
        raise ValueError('Live test consent scope mismatch')
    for key in ['runtime_use_authorization', 'microphone_authorization', 'ready_reference']:
        if not isinstance(permission.get(key), str) or not permission[key].strip():
            raise ValueError('Separate Vosk use, microphone and ready permissions required')
    scenario = json.loads(private_file(root/'scenario.json', root, 16384).read_text())
    cfg = json.loads(private_file(root/'config/kde-assistant/config.json', root, 262144).read_text())
    required = dict(stt_engine='vosk', stt_language='es', stt_backend='local',
        vosk_library=scenario['library'], vosk_model=scenario['model'], auto_speak=False,
        auto_listen=False, chimes_enabled=False, wake_word_enabled=False,
        max_recording_secs=8, mic_device='')
    if any(cfg['speech'].get(k) != v for k, v in required.items()):
        raise ValueError('Private speech configuration no longer matches consent scope')
    if cfg['ai']['api_key'] != 'synthetic-f4r2-local-only' or cfg['ai']['enable_tool_calling']:
        raise ValueError('Synthetic provider without tools required')
    marker = receipt.with_suffix('.used')
    if marker.exists():
        raise ValueError('Live test permission already consumed')
    return env, marker


def save_observation(root, value):
    # Keep bounded, transcript-free evidence even if the terminal disappears.
    descriptor, pending = tempfile.mkstemp(prefix='.observation-', dir=root)
    try:
        with os.fdopen(descriptor, 'w') as output:
            json.dump(value, output, indent=2)
            output.write('\n')
        os.replace(pending, root/'observation.json')
    finally:
        Path(pending).unlink(missing_ok=True)


def launch(root, receipt):
    env, marker = admit(root, receipt)
    baseline(env)
    display = os.environ.get('WAYLAND_DISPLAY', '')
    runtime = os.environ.get('XDG_RUNTIME_DIR', '')
    bus = os.environ.get('DBUS_SESSION_BUS_ADDRESS', '')
    if not display or not runtime or not bus:
        raise ValueError('Existing Wayland display/session bus required')
    if not Path(display).is_absolute():
        display = str(Path(runtime)/display)
    remote = live_remote()  # Only after explicit scoped consent has passed.
    calls = []
    class Mock(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass
        def do_POST(self):
            count = int(self.headers.get('Content-Length', '0'))
            if not 0 <= count <= 2097152:
                self.send_error(413)
                return
            self.rfile.read(count)  # Discard; never log or send the request elsewhere.
            calls.append(self.path)
            data = b'data: {"choices":[{"delta":{"content":"Respuesta mock local"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n'
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
    mock = ThreadingHTTPServer(('127.0.0.1', 0), Mock)
    server_thread = threading.Thread(target=mock.serve_forever, daemon=True)
    server_thread.start()
    cfg_path = root/'config/kde-assistant/config.json'
    cfg = json.loads(private_file(cfg_path, root, 262144).read_text())
    cfg['ai']['base_url'] = f'http://127.0.0.1:{mock.server_port}/v1'
    cfg_path.write_text(json.dumps(cfg, indent=2)+'\n')
    require_private(root, env)
    write_private(marker, dict(consumed=True, purpose='live_f4r2_ptt'))
    live = {**env, 'PIPEWIRE_REMOTE': str(remote), 'DBUS_SESSION_BUS_ADDRESS': bus,
        'WAYLAND_DISPLAY': display, 'QT_QPA_PLATFORM': 'wayland',
        'KDE_ASSISTANT_DATA_DIR': str(root/'resources')}
    app = None
    log_thread = None
    transitions, counts, owned = [], [], set()
    def observation(ended):
        return dict(transitions=transitions, llm_requests=len(calls),
            session_message_counts=counts, observed_children=len(owned),
            ended=ended,
            children_reaped=ended and all(not Path(f'/proc/{pid}').exists() for pid in owned),
            backend_reaped=ended and (app is None or app.poll() is not None),
            human_verification='PENDING_USER_REPORT', raw_audio_saved=False)
    try:
        app = application(root, live, True)
        url, token, log_thread = attach_log(app, root, 'live.log')
        print('Instancia privada preparada. El micrófono permanece apagado hasta pulsar Grabar.', flush=True)
        started = time.monotonic()
        previous = None
        save_observation(root, observation(False))
        while app.poll() is None and time.monotonic()-started < 300:
            status = api(url, token, '/api/voice/status')
            if status['state'] != previous:
                previous = status['state']
                transitions.append(dict(state=previous, elapsed_ms=(time.monotonic()-started)*1000))
                counts = [s.get('message_count') for s in api(url, token, '/api/sessions')]
                save_observation(root, observation(False))
                print('Estado:', previous, flush=True)
            owned.update(children(app.pid))
            time.sleep(.1)
    finally:
        # Persist the last state before cleanup, which a second terminal signal
        # can interrupt. Do not mark reaping complete until it actually finishes.
        save_observation(root, observation(False))
        if app:
            if app.poll() is None:
                app.terminate()
            try:
                app.wait(timeout=15)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait(timeout=5)
            if log_thread:
                log_thread.join(timeout=2)
            if log_thread is None or not log_thread.is_alive():
                app.stdout.close()
        mock.shutdown()
        mock.server_close()
        server_thread.join(timeout=2)
        # No transcript, hypothesis or raw audio in this observation record.
        save_observation(root, observation(True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--prepare', action='store_true')
    mode.add_argument('--launch', action='store_true')
    parser.add_argument('--library', type=Path)
    parser.add_argument('--model', type=Path)
    parser.add_argument('--profile', type=Path)
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    if args.prepare:
        if not args.library or not args.model or not args.library.is_absolute() or not args.model.is_absolute():
            parser.error('Preparation needs explicit absolute model/runtime paths')
        prepare(args.library, args.model)
    else:
        if not args.profile or not args.receipt:
            parser.error('Live launch requires private profile and explicit consent receipt')
        launch(profile_path(args.profile), args.receipt)


if __name__ == '__main__':
    main()
