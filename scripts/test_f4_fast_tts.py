#!/usr/bin/env python3
"""Consent fixtures only: no binary, model, host socket, microphone or playback."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from f4_safety import private_env
from live_f4_fast_tts import PURPOSE, PHRASE, admit, launch


class FastTtsConsent(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='kda-f4-accept-consent-')
        self.root = Path(self.temp.name); private_env(self.root)
        self.cfg = self.root/'config/kde-assistant/config.json'
        self.cfg.parent.mkdir(mode=0o700); self.write(self.cfg, {'speech':{'piper_fast':True}})
        self.scenario = dict(schema=1, purpose=PURPOSE, phrase=PHRASE, nonce='test-only-nonce',
            binary_sha256='test-only-binary', config_sha256=hashlib.sha256(self.cfg.read_bytes()).hexdigest())
        self.write(self.root/'scenario.json', self.scenario)
        self.receipt = self.root/'permission.json'
        self.permission = {k:self.scenario[k] for k in ['purpose','phrase','nonce','binary_sha256']}
        self.permission.update(trial=1, microphone=False, audio_authorization='UNIT fixture', ready_reference='UNIT fixture')
        self.write(self.receipt, self.permission)
    def tearDown(self): self.temp.cleanup()
    def write(self, path, data):
        path.write_text(json.dumps(data)); path.chmod(0o600)
    def test_missing_permission_never_checks_binary_or_host_socket(self):
        for key in self.permission:
            bad = self.permission.copy(); bad.pop(key); self.write(self.receipt, bad)
            with patch('live_f4_fast_tts.verify', side_effect=AssertionError('binary touched')), patch('live_f4_fast_tts.live_remote', side_effect=AssertionError('host audio touched')):
                with self.assertRaises(ValueError): launch(self.root, self.receipt)
    def test_old_t1_wrong_trial_and_wrong_nonce_are_rejected(self):
        for key, value in [('purpose','live_f4r3_tts_t1'),('trial',2),('nonce','another-trial'),('microphone',True)]:
            bad = self.permission.copy(); bad[key] = value; self.write(self.receipt, bad)
            with self.assertRaises(ValueError): admit(self.root, self.receipt)
    def test_valid_admission_remains_isolated(self):
        env, scenario, marker = admit(self.root, self.receipt)
        self.assertEqual(env['PIPEWIRE_REMOTE'], 'f4-unreachable-audio')
        self.assertEqual(scenario['purpose'], PURPOSE); self.assertFalse(marker.exists())
    def test_consumed_or_premature_gate_is_rejected(self):
        self.write(self.receipt.with_suffix('.used'), {'consumed':True})
        with self.assertRaises(ValueError): admit(self.root, self.receipt)
        self.receipt.with_suffix('.used').unlink()
        self.write(self.root/'play-once.json', {'trial':1})
        with self.assertRaises(ValueError): admit(self.root, self.receipt)
    def test_changed_config_or_symlinked_receipt_is_rejected(self):
        self.write(self.cfg, {'speech':{'piper_fast':False}})
        with self.assertRaises(ValueError): admit(self.root, self.receipt)
        self.receipt.unlink(); self.receipt.symlink_to(self.root/'scenario.json')
        with self.assertRaises(ValueError): admit(self.root, self.receipt)

    def test_t2_gui_rejects_t1_consent_before_any_output_or_launch(self):
        import live_f4_voice_gui as gui
        with patch.object(gui, 'verify', side_effect=AssertionError('binary touched')), patch.object(gui, 'live_remote', side_effect=AssertionError('output touched')):
            with self.assertRaises(ValueError): gui.launch(self.root, self.receipt)

    def test_t2_has_its_own_trial_phrase_nonce_and_no_microphone(self):
        import live_f4_voice_gui as gui
        scenario = {**self.scenario, 'purpose':gui.PURPOSE, 'phrase':gui.PHRASE}
        self.write(self.root/'scenario.json', scenario)
        permission = {**self.permission, 'purpose':gui.PURPOSE, 'phrase':gui.PHRASE, 'trial':2}
        self.write(self.receipt, permission)
        env, _, _ = admit(self.root, self.receipt, purpose=gui.PURPOSE, phrase=gui.PHRASE, trial=2)
        self.assertEqual(env['PIPEWIRE_REMOTE'], 'f4-unreachable-audio')
        permission['trial'] = 3; self.write(self.receipt, permission)
        with self.assertRaises(ValueError): admit(self.root, self.receipt, purpose=gui.PURPOSE, phrase=gui.PHRASE, trial=2)

    def test_t3_cannot_reuse_t2_receipt_or_scenario(self):
        import live_f4_voice_gui as gui
        scenario = {**self.scenario, 'purpose':gui.REPLAY_PURPOSE, 'phrase':gui.PHRASE, 'nonce':'t3-only'}
        self.write(self.root/'scenario-t3.json', scenario)
        permission = {**self.permission, **{k:scenario[k] for k in ('purpose','phrase','nonce')}, 'trial':3}
        self.write(self.receipt, permission)
        env, _, _ = admit(self.root, self.receipt, purpose=gui.REPLAY_PURPOSE, phrase=gui.PHRASE, trial=3, scenario_name='scenario-t3.json')
        self.assertEqual(env['PIPEWIRE_REMOTE'], 'f4-unreachable-audio')
        permission.update(purpose=gui.PURPOSE, trial=2); self.write(self.receipt, permission)
        with patch.object(gui, 'verify', side_effect=AssertionError('binary touched')), patch.object(gui, 'live_remote', side_effect=AssertionError('output touched')):
            with self.assertRaises(ValueError): gui.launch(self.root, self.receipt, trial=3)

    def test_t4_rejects_t3_receipt_before_launch_or_audio(self):
        import live_f4_voice_gui as gui
        scenario = {**self.scenario, 'purpose':gui.SESSION_PURPOSE, 'phrase':gui.PHRASE, 'nonce':'t4-only'}
        self.write(self.root/'scenario-t4.json', scenario)
        permission = {**self.permission, **{k:scenario[k] for k in ('purpose','phrase','nonce')}, 'trial':4}
        self.write(self.receipt, permission)
        env, _, _ = admit(self.root, self.receipt, purpose=gui.SESSION_PURPOSE, phrase=gui.PHRASE, trial=4, scenario_name='scenario-t4.json')
        self.assertEqual(env['PIPEWIRE_REMOTE'], 'f4-unreachable-audio')
        permission.update(purpose=gui.REPLAY_PURPOSE, trial=3); self.write(self.receipt, permission)
        with patch.object(gui, 'verify', side_effect=AssertionError('binary touched')), patch.object(gui, 'live_remote', side_effect=AssertionError('output touched')):
            with self.assertRaises(ValueError): gui.launch(self.root, self.receipt, trial=4)

    def test_t5_rejects_other_trial_before_launch_or_audio(self):
        import live_f4_auto_tts as auto
        scenario = {**self.scenario, 'purpose':auto.PURPOSE, 'phrase':auto.PHRASE, 'nonce':'t5-only'}
        self.write(self.root/'scenario.json', scenario)
        permission = {**self.permission, **{k:scenario[k] for k in ('purpose','phrase','nonce')}, 'trial':5}
        self.write(self.receipt, permission)
        env, _, _ = admit(self.root, self.receipt, purpose=auto.PURPOSE, phrase=auto.PHRASE, trial=5)
        self.assertEqual(env['PIPEWIRE_REMOTE'], 'f4-unreachable-audio')
        permission['trial'] = 4; self.write(self.receipt, permission)
        with patch.object(auto, 'verify', side_effect=AssertionError('binary touched')), patch.object(auto, 'live_remote', side_effect=AssertionError('output touched')):
            with self.assertRaises(ValueError): auto.launch(self.root, self.receipt)


if __name__ == '__main__': unittest.main()
