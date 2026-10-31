#!/usr/bin/env python3
"""Offline ML acceptance. Explicit existing resources; no capture, playback or downloads."""
import argparse
import array
import hashlib
import json
import math
from pathlib import Path
import random
import resource
import subprocess
import sys
import tempfile
import wave
sys.dont_write_bytecode = True
from f4_safety import private_env


def pcm16(samples):
    out = array.array('h', (max(-32768, min(32767, round(x))) for x in samples))
    if sys.byteorder != 'little':
        out.byteswap()
    return out.tobytes()


def resample(data, source_rate):
    values = array.array('h', data)
    if sys.byteorder != 'little':
        values.byteswap()
    if source_rate == 16000:
        return data
    ratio = source_rate / 16000
    return pcm16(values[int(i * ratio)] * (1 - (i * ratio % 1))
                 + values[min(int(i * ratio) + 1, len(values) - 1)] * (i * ratio % 1)
                 for i in range(int(len(values) / ratio)))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--diagnostic', type=Path, required=True)
    p.add_argument('--models', type=Path, required=True)
    p.add_argument('--onnx-runtime', type=Path, required=True)
    p.add_argument('--positive-wav', type=Path)
    p.add_argument('--positive-text', default='[[hˈeɪ dʒˈɑːɹvɪs]]',
                   help='English Hey Jarvis phonemes; the available Piper voice is Spanish')
    p.add_argument('--piper-runtime', type=Path)
    p.add_argument('--piper-model', type=Path)
    args = p.parse_args()
    for name in ('binary', 'diagnostic', 'models', 'onnx_runtime'):
        setattr(args, name, getattr(args, name).resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix='kda-jarvis-offline-') as temporary:
        root = Path(temporary)
        env = private_env(root)
        # Worker-only invocations never create a backend or connect to audio/DBus.
        def synth(text):
            if not args.piper_runtime or not args.piper_model:
                p.error('Provide --positive-wav or both existing Piper paths')
            runtime = args.piper_runtime.resolve(strict=True)
            model = args.piper_model.resolve(strict=True)
            checked = subprocess.run([str(args.binary), '--voice-piper-check', str(runtime), str(model)],
                                     env=env, capture_output=True, timeout=30, check=True)
            assert json.loads(checked.stdout) is None, 'Piper resources did not pass verification'
            # Fixture-only synthesis: zero noise and fixed seeds give repeatable PCM.
            # These options do not modify F4's production Piper configuration.
            code = '''import sys,json
sys.path.insert(0,sys.argv[1])
import numpy as np,onnxruntime as ort
from piper import PiperVoice,SynthesisConfig
from piper.config import PiperConfig
ort.set_seed(0); np.random.seed(0)
options=ort.SessionOptions(); options.intra_op_num_threads=1; options.inter_op_num_threads=1
options.add_session_config_entry('session.intra_op.allow_spinning','0')
with open(sys.argv[2]+'.json') as f: config=PiperConfig.from_dict(json.load(f))
voice=PiperVoice(config=config,session=ort.InferenceSession(sys.argv[2],sess_options=options,providers=['CPUExecutionProvider']))
for chunk in voice.synthesize(sys.stdin.buffer.read().decode(),SynthesisConfig(speaker_id=0,length_scale=1,noise_scale=0,noise_w_scale=0)):
 sys.stdout.buffer.write(chunk.audio_int16_bytes)
'''
            command = [str(runtime / 'bin/python'), '-I', '-S', '-B', '-c', code,
                       str(runtime / 'lib/python3.14/site-packages'), str(model)]
            result = subprocess.run(command, input=text.encode(), env=env, cwd=root,
                                    capture_output=True, timeout=120, check=True)
            return resample(result.stdout, 22050)
        if args.positive_wav:
            with wave.open(str(args.positive_wav), 'rb') as wav:
                assert wav.getnchannels() == 1 and wav.getsampwidth() == 2
                assert wav.getnframes() <= wav.getframerate() * 30
                positive = resample(wav.readframes(wav.getnframes()), wav.getframerate())
            provenance = 'explicit supplied WAV'
        else:
            positive = synth(args.positive_text)
            provenance = 'local verified Piper es_ES-sharvard-medium, speaker 0; explicit phonemes: ' + args.positive_text
        rng = random.Random(42)
        cases = {'silence': bytes(32000 * 6),
                 'tone': pcm16(14000 * math.sin(2 * math.pi * 440 * i / 16000) for i in range(96000)),
                 'noise': pcm16(rng.gauss(0, 3000) for _ in range(96000)),
                 'positive': bytes(32000) + positive + bytes(32000 * 2),
                 'repeated': bytes(32000) + positive + bytes(32000) + positive + bytes(32000 * 2)}
        if args.piper_runtime and args.piper_model:
            cases['unrelated_speech'] = bytes(32000) + synth('Hola. ¿Qué hora es? Abre el navegador.') + bytes(64000)
        observations = {'provenance': provenance, 'positive_pcm_sha256': hashlib.sha256(positive).hexdigest(),
                        'cases': {}}
        for name, data in cases.items():
            data += bytes((-len(data)) % 2560)
            wavpath = root / (name + '.wav')
            with wave.open(str(wavpath), 'wb') as wav:
                wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(16000); wav.writeframes(data)
            before = resource.getrusage(resource.RUSAGE_CHILDREN)
            diagnostic = subprocess.run([str(args.diagnostic), str(args.models),
                                         str(args.onnx_runtime), str(wavpath), '0.8'],
                                        env=env, capture_output=True, timeout=60, check=True)
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            detail = json.loads(diagnostic.stdout)
            detail["cpu_seconds_total"] = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
            detail.pop('scores')
            command = [str(args.binary), '--voice-wake-worker', str(args.models), str(args.onnx_runtime), '0.8']
            result = subprocess.run(command, input=data, env=env, capture_output=True, timeout=60, check=True)
            expected = b'RD' if name in ('positive', 'repeated') else b'R'
            detail['protocol'] = result.stdout.decode('ascii')
            detail['pass'] = result.stdout == expected
            observations['cases'][name] = detail
        print(json.dumps(observations, indent=2), flush=True)
        assert all(x['pass'] for x in observations['cases'].values()), 'Offline ML acceptance failed'
        assert not (root / 'config/kde-assistant').exists(), 'Worker must not initialize application profile'


if __name__ == '__main__':
    main()
