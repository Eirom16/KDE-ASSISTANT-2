#!/usr/bin/env python3
"""CC0 authored signal fixtures. No recordings, playback, downloads or personal data."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import struct
import wave

PHRASES = [
    ("short", "Hola, ¿qué hora es?"),
    ("long", "Quiero preparar una lista de tareas para mañana y revisar primero los asuntos pendientes de esta semana."),
    ("numbers", "Recuérdame los números veintitrés, ciento cinco y dos mil veintiséis."),
    ("applications", "¿Cómo puedo configurar Dolphin, Firefox y LibreOffice?"),
    ("natural", "Explícame cómo organizar mis documentos sin mover ningún archivo."),
]

def generate(directory, rate=48000):
    if rate not in (16000, 48000): raise ValueError('Fixture rate must be 16000 or 48000')
    directory.mkdir(parents=True, exist_ok=True)
    rng = random.Random(4)
    records = []
    for name in ("silence", "noise", "tone"):
        values = [0.0 if name == "silence" else rng.uniform(-0.04, 0.04) if name == "noise" else 0.15*math.sin(2*math.pi*440*i/rate) for i in range(3*rate)]
        path = directory / f"{name}.wav"
        with wave.open(str(path), "wb") as audio:
            audio.setparams((1, 2, rate, 0, "NONE", "not compressed"))
            audio.writeframes(b"".join(struct.pack("<h", round(v*32767)) for v in values))
        records.append({"id":name, "file":path.name,"reference":"", "kind":"synthetic_non_speech", "license":"CC0-1.0", "sample_rate":rate, "seconds":3, "sha256":hashlib.sha256(path.read_bytes()).hexdigest()})
    for name, reference in PHRASES:
        records.append({"id":name,"reference":reference,"file":None,"kind":"human_speech_pending_explicit_consent","status":"NOT_TESTED"})
    (directory / "manifest.json").write_text(json.dumps({"license":"CC0-1.0 for authored text and generated signals", "warning":"No human speech included; no Spanish accuracy acceptance or dialect evidence", "samples":records}, ensure_ascii=False, indent=2))

def wer(reference, hypothesis):
    import re
    def words(text):
        return re.findall(r"[^\W_]+", text.lower(), re.UNICODE)
    ref, hyp = words(reference), words(hypothesis)
    if not ref:
        return 0.0 if not hyp else None
    previous = list(range(len(hyp)+1))
    for i, a in enumerate(ref, 1):
        row = [i]
        for j, b in enumerate(hyp, 1):
            row.append(min(row[-1]+1, previous[j]+1, previous[j-1]+(a!=b)))
        previous = row
    return previous[-1]/len(ref)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--rate", type=int, choices=[16000,48000], default=48000)
    args = parser.parse_args()
    generate(args.directory,args.rate)
