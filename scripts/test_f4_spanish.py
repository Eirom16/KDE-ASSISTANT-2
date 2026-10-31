#!/usr/bin/env python3
"""Text, synthetic WAV and private subprocess fixtures; no human/audio device use."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
sys.dont_write_bytecode = True
import tempfile
import unittest
import urllib.error
from unittest.mock import patch
import wave
from compare_f4_spanish import corpus, private_file, profile_path, run_child, vosk_worker, write_private
from f4_safety import private_env
from f4_spanish import alignment, check_phrases, reduced_context
from record_f4_spanish import admit, capture, record_command
from live_f4_ptt import launch as launch_ptt


class SpanishEvaluation(unittest.TestCase):
    def test_live_fixture_records_progress_and_reaps_on_observer_failure(self):
        with tempfile.TemporaryDirectory(prefix='kda-f4r2-live-unit-', dir='/var/tmp') as directory:
            root = Path(directory)
            env = private_env(root)
            scenario = dict(library='/unit/no-library.so', model='/unit/no-model')
            write_private(root/'scenario.json', scenario)
            speech = dict(stt_engine='vosk', stt_language='es', stt_backend='local',
                vosk_library=scenario['library'], vosk_model=scenario['model'], auto_speak=False,
                auto_listen=False, chimes_enabled=False, wake_word_enabled=False,
                max_recording_secs=8, mic_device='')
            folder = root/'config/kde-assistant'
            folder.mkdir(mode=0o700)
            write_private(folder/'config.json', dict(speech=speech,
                ai=dict(api_key='synthetic-f4r2-local-only', enable_tool_calling=False)))
            receipt = root/'unit-consent.json'
            write_private(receipt, dict(purpose='live_f4r2_ptt', max_capture_secs=8,
                runtime_use_authorization='UNIT fake process only',
                microphone_authorization='UNIT fake process; no audio device', ready_reference='UNIT'))
            child = subprocess.Popen([sys.executable, '-B', '-c', 'import time; time.sleep(30)'],
                cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            states = iter(['idle', 'recording', 'transcript_ready'])
            def fake_api(_url, _token, path):
                if path == '/api/sessions':
                    return [dict(message_count=0)]
                state = next(states, None)
                if state is None:
                    partial = json.loads((root/'observation.json').read_text())
                    self.assertEqual(partial['transitions'][-1]['state'], 'transcript_ready')
                    self.assertFalse(partial['ended'])
                    self.assertFalse(partial['children_reaped'])
                    raise urllib.error.URLError('UNIT observer disconnected')
                return dict(state=state)
            try:
                with patch.dict(os.environ, dict(WAYLAND_DISPLAY='/unit/display',
                        XDG_RUNTIME_DIR=str(root/'runtime'), DBUS_SESSION_BUS_ADDRESS='unix:path=/unit/bus')), \
                     patch('live_f4_ptt.live_remote', return_value=Path('/unit/no-audio-socket')), \
                     patch('live_f4_ptt.application', return_value=child), \
                     patch('live_f4_ptt.attach_log', return_value=('http://127.0.0.1:1', 'UNIT', None)), \
                     patch('live_f4_ptt.api', side_effect=fake_api):
                    with self.assertRaises(urllib.error.URLError):
                        launch_ptt(root, receipt)
                result = json.loads((root/'observation.json').read_text())
                self.assertTrue(result['ended'])
                self.assertTrue(result['children_reaped'])
                self.assertTrue(result['backend_reaped'])
                self.assertEqual(result['llm_requests'], 0)
                self.assertEqual(result['session_message_counts'], [0])
                self.assertFalse(result['raw_audio_saved'])
                self.assertEqual((root/'observation.json').stat().st_mode & 0o777, 0o600)
                self.assertFalse(Path(f'/proc/{child.pid}').exists())
                self.assertFalse(list(root.glob('.observation-*')))
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=3)
                child.stdout.close()

    def test_live_ptt_missing_permission_cannot_launch_or_access_live_socket(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private_env(root)
            receipt = root/'consent.json'
            write_private(receipt, dict(purpose='live_f4r2_ptt', max_capture_secs=8,
                runtime_use_authorization='UNIT only; microphone not authorised'))
            with patch('live_f4_ptt.subprocess.Popen') as spawn, patch('live_f4_ptt.live_remote') as remote:
                with self.assertRaisesRegex(ValueError, 'Separate'):
                    launch_ptt(root, receipt)
                spawn.assert_not_called()
                remote.assert_not_called()

    def test_alignment_counts_and_empty_result(self):
        score = alignment('uno dos tres cuatro cinco seis', 'uno otro tres cinco seis extra')
        self.assertEqual((score['substitution'], score['deletion'], score['insertion']), (1, 1, 1))
        self.assertEqual(score['wer'], .5)
        self.assertTrue(alignment('hola', '')['empty_unjustified'])
        self.assertIsNone(alignment('', 'extra')['wer'])
        self.assertEqual(alignment('', '')['wer'], 0)

    def test_unicode_accents_and_numeric_formats_remain_explicit(self):
        self.assertEqual(alignment('¡Sí, qué café!', 'SI\u0301 QUE\u0301 CAFE\u0301')['wer'], 0)
        self.assertGreater(alignment('sí', 'si')['wer'], 0)
        self.assertEqual(alignment('veintitrés', '23')['wer'], 1)
        checks = check_phrases('anota 23 y 105 en Libre Office',
                              [['veintitrés', '23'], ['ciento cinco', '105'], ['LibreOffice', 'Libre Office']])
        self.assertTrue(all(c['present'] for c in checks))
        self.assertFalse(check_phrases('firefoxes', [['Firefox']])[0]['present'])
        with self.assertRaises(ValueError): alignment('palabra '*513, '')

    def test_reduced_window_covers_long_phrase_and_uses_full_when_required(self):
        self.assertEqual(reduced_context(3*16000), 256)
        self.assertEqual(reduced_context(6*16000), 512)
        self.assertEqual(reduced_context(12*16000), 768)
        self.assertEqual(reduced_context(30*16000), 0)
        for seconds in range(1, 31):
            ctx = reduced_context(seconds*16000)
            if ctx: self.assertGreaterEqual(ctx/50, seconds+1)
        for frames in (0, -1, 30*16000+1):
            with self.assertRaises(ValueError): reduced_context(frames)

    def test_corpus_consent_reference_hash_and_wav_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private_env(root)
            path = root/'manifest.json'
            manifest = dict(schema=1, kind='human_speech', authorization=None,
                            samples=[dict(id='short', reference='hola', reference_confirmed=False, file='short.wav')])
            write_private(path, manifest)
            with self.assertRaisesRegex(ValueError, 'consent'): corpus(root)
            manifest['authorization'] = 'UNIT synthetic fixture, no human recorded'
            path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'confirm'): corpus(root)
            manifest['samples'][0]['reference_confirmed'] = True
            path.write_text(json.dumps(manifest))
            wav = root/'short.wav'
            with wave.open(str(wav), 'wb') as audio:
                audio.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
                audio.writeframes(bytes(32000))
            wav.chmod(0o600)
            rows = corpus(root)
            self.assertEqual(rows[0]['frames'], 16000)
            self.assertEqual(len(rows[0]['sha256']), 64)
            wav.chmod(0o644)
            with self.assertRaises(ValueError): corpus(root)
            wav.chmod(0o600)
            manifest['samples'][0]['file'] = '../outside.wav'
            path.write_text(json.dumps(manifest))
            with self.assertRaises(ValueError): corpus(root)

    def test_symlink_output_and_repository_profiles_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as outside:
            root = Path(directory)
            target = Path(outside)/'file'
            target.write_text('synthetic')
            target.chmod(0o600)
            (root/'alias').symlink_to(target)
            with self.assertRaises(ValueError): private_file(root/'alias', root, 100)
            with self.assertRaises(ValueError): profile_path(Path(__file__).resolve().parents[1])

    def test_worker_and_runner_fail_closed_before_native_code_or_spawn(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch('compare_f4_spanish.ctypes.CDLL') as load:
                with self.assertRaises((ValueError, FileNotFoundError)):
                    vosk_worker(argparse.Namespace(), root)
                load.assert_not_called()
            with patch('compare_f4_spanish.subprocess.Popen') as spawn:
                with self.assertRaises(ValueError): run_child([], {}, root, root/'log', 1)
                spawn.assert_not_called()

    def test_timeout_and_compiler_rejection_reap_only_owned_child(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            env = private_env(root)
            marker = root/'pid'
            command = [sys.executable, '-B', '-c',
                       'import os,pathlib,time; pathlib.Path("pid").write_text(str(os.getpid())); time.sleep(30)']
            with patch('compare_f4_spanish.compilers', return_value=[]):
                with self.assertRaises(TimeoutError): run_child(command, env, root, root/'timeout.log', .4)
            pid = int(marker.read_text())
            self.assertFalse(Path(f'/proc/{pid}').exists())
            with patch('compare_f4_spanish.compilers', return_value=[123]), patch('subprocess.Popen') as spawn:
                with self.assertRaises(RuntimeError): run_child(command, env, root, root/'compiler.log', 1)
                spawn.assert_not_called()

    def test_recording_requires_two_confirmations_before_socket_or_spawn(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private_env(root)
            receipt = root/'receipt.json'
            write_private(receipt, dict(purpose='offline_f4r2_corpus', sample='short', max_seconds=8,
                                        authorization_reference='UNIT fixture only'))
            with patch('record_f4_spanish.live_remote') as remote, patch('subprocess.Popen') as spawn:
                with self.assertRaises(ValueError): capture(root, receipt)
                remote.assert_not_called()
                spawn.assert_not_called()

    def test_recording_fixture_is_bounded_reaped_and_consent_not_reusable(self):
        # A Python child supplies zeros, never a socket/device; not LIVE evidence.
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            private_env(root)
            receipt = root/'receipt.json'
            write_private(receipt, dict(purpose='offline_f4r2_corpus', sample='short', max_seconds=1,
                authorization_reference='UNIT synthetic', ready_reference='UNIT synthetic'))
            native_spawn = subprocess.Popen
            def fixture_spawn(command, **kwargs):
                self.assertEqual(command[0], '/usr/bin/pw-record')
                self.assertFalse(any(arg.startswith('--target') for arg in command))
                self.assertTrue(Path(kwargs['env']['HOME']).is_relative_to(root))
                return native_spawn([sys.executable, '-B', '-c',
                    'import os,time; os.write(1,bytes(32000)); time.sleep(30)'], **kwargs)
            with patch('record_f4_spanish.baseline'), \
                 patch('record_f4_spanish.live_remote', return_value=Path('/unit-fixture-socket')), \
                 patch('record_f4_spanish.subprocess.Popen', side_effect=fixture_spawn):
                capture(root, receipt)
            with wave.open(str(root/'short.wav')) as audio:
                self.assertEqual(audio.getnframes(), 16000)
            evidence = json.loads((root/'short-capture.json').read_text())
            self.assertTrue(evidence['reaped'])
            self.assertFalse(Path(f"/proc/{evidence['pid']}").exists())
            self.assertEqual(evidence['product_ptt'], 'NOT_TESTED')
            with self.assertRaises(ValueError): admit(root, receipt)

    @unittest.skipUnless(Path('/usr/bin/pipewire').is_file(), 'Private PipeWire fixture unavailable')
    def test_native_target_property_does_not_pin_default_to_literal_auto(self):
        import subprocess
        from f3_runtime import wait_for
        from compare_f4_spanish import baseline, REPO
        # Nested private TMPDIRs can exceed sockaddr_un's 108-byte limit.
        # Keep this daemon private while giving its native socket a short path.
        with tempfile.TemporaryDirectory(prefix='kda-f4r2-pw-', dir='/var/tmp') as directory:
            root = Path(directory)
            env = private_env(root)
            baseline(env)
            daemon = subprocess.Popen(['/usr/bin/pipewire', '-c', str(REPO/'tests/fixtures/f4/pipewire.conf')],
                                      env=env, cwd=root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                remote = root/'runtime/f4-synthetic'
                wait_for(remote.exists, 3)
                clients = {**env, 'PIPEWIRE_REMOTE': str(remote)}
                for explicit in (True, False):
                    command = record_command()
                    if explicit: command.insert(-1, '--target=auto')
                    child = subprocess.Popen(command, cwd=root, env=clients,
                                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    try:
                        def own_node():
                            nodes = json.loads(subprocess.check_output(['/usr/bin/pw-dump'],
                                                                       env=clients, cwd=root, timeout=3))
                            return next((n for n in nodes if n.get('info', {}).get('props', {}).get('media.category') == 'Capture'), None)
                        node = wait_for(own_node, 3)
                        self.assertEqual(node['info']['props'].get('target.object'), 'auto' if explicit else None)
                    finally:
                        child.kill()
                        child.wait(timeout=3)
            finally:
                daemon.terminate()
                daemon.wait(timeout=3)


if __name__ == '__main__': unittest.main()
