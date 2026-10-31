#!/usr/bin/env python3
"""T1 admission guards with synthetic profiles; no binary, socket or audio device."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from live_f4_tts import admit, launch, PHRASE, PURPOSE
from f4_safety import private_env


class TtsConsent(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='kda-f4r3-consent-')
        self.root=Path(self.temp.name);private_env(self.root)
        self.engine=self.root/'engine'
        self.cfg=dict(speech=dict(piper_runtime=str(self.engine/'venv'),piper_model=str(self.engine/'voice/es_ES-sharvard-medium.onnx'),
            piper_speaker=0,piper_length_scale=1.0,stt_model='/nonexistent/f4r3-microphone-disabled.bin',stt_engine='whisper',
            auto_speak=False,auto_listen=False,wake_word_enabled=False,chimes_enabled=False),
            ai=dict(base_url='http://127.0.0.1:1/v1',enable_tool_calling=False),memory=dict(enabled=False,auto_summarize=False))
        self.config=self.root/'config/kde-assistant/config.json';self.config.parent.mkdir(mode=0o700)
        self.write(self.config,self.cfg)
        self.write(self.root/'scenario.json',dict(purpose=PURPOSE,phrase=PHRASE,engine_profile=str(self.engine)))
        self.receipt=self.root/'consent.json'
        self.permission=dict(purpose=PURPOSE,phrase=PHRASE,audio_authorization='synthetic consent',ready_reference='synthetic readiness')
        self.write(self.receipt,self.permission)
    def tearDown(self):self.temp.cleanup()
    def write(self,path,value):path.write_text(json.dumps(value));path.chmod(0o600)
    def test_missing_consent_or_readiness_never_accesses_host_socket_or_launches(self):
        for field in ['audio_authorization','ready_reference','purpose','phrase']:
            permission=self.permission.copy();permission.pop(field);self.write(self.receipt,permission)
            with patch('live_f4_tts.live_remote',side_effect=AssertionError('host socket touched')),patch('live_f4_tts.application',side_effect=AssertionError('binary launched')):
                with self.assertRaises(ValueError):launch(self.root,self.receipt)
    def test_valid_admission_preserves_unreachable_audio_until_launch(self):
        env,marker,scenario=admit(self.root,self.receipt)
        self.assertEqual(env['PIPEWIRE_REMOTE'],'f4-unreachable-audio')
        self.assertFalse(marker.exists())
        self.assertEqual(scenario['purpose'],PURPOSE)
    def test_used_receipt_is_rejected(self):
        self.write(self.receipt.with_suffix('.used'),dict(consumed=True))
        with self.assertRaises(ValueError):admit(self.root,self.receipt)
    def test_changed_configuration_is_rejected(self):
        for group,key,value in [('speech','auto_speak',True),('speech','stt_model','/other/model'),
                                ('ai','base_url','https://remote.invalid/v1'),('ai','enable_tool_calling',True),
                                ('memory','enabled',True)]:
            old=self.cfg[group][key];self.cfg[group][key]=value;self.write(self.config,self.cfg)
            with self.assertRaises(ValueError):admit(self.root,self.receipt)
            self.cfg[group][key]=old;self.write(self.config,self.cfg)


if __name__=='__main__':unittest.main()
