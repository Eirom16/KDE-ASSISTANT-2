#!/usr/bin/env python3
"""F4 deterministic validation. Private XDG; unreachable audio socket; no real credentials/hardware."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import sys
sys.dont_write_bytecode = True
import time
from qml_test_harness import build_runner
from voice_corpus import generate, wer
from f4_runtime import runtime
from f4_pipewire import fixture
from f4_qml_lifecycle import validate as validate_main
from f4_safety import private_env


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    repo=Path(__file__).resolve().parents[1]
    out=args.output or Path(tempfile.mkdtemp(prefix='kda-f4-results-'))
    out.mkdir(parents=True,exist_ok=True)
    results=[]
    with tempfile.TemporaryDirectory(prefix='kda-f4-private-') as temporary:
        profile=Path(temporary)
        env=private_env(profile)
        env.update(QT_QPA_PLATFORM='offscreen',PIPEWIRE_REMOTE='f4-unreachable-audio',DBUS_SESSION_BUS_ADDRESS='unix:path='+str(profile/'absent-bus'))
        runner=build_runner(repo,profile,env)
        generate(profile/'corpus')
        assert wer('Hola, mundo','hola mundo')==0 and wer('uno dos','uno tres')==0.5
        commands=[
            ('isolation','UNIT',['python3','scripts/test_f4_safety.py']),
            ('spanish-evaluator','UNIT',['python3','-B','scripts/test_f4_spanish.py']),
            ('build','UNIT',['cargo','build','--locked','--bin','kde-assistant']),
            ('internal-cli','UNIT',['cargo','test','--locked','--test','f4_cli','--','--test-threads=1']),
            ('coordinator','UNIT',['cargo','test','--locked','--lib','voice_coordinator::tests','--','--test-threads=1']),
            ('warm-piper','UNIT',['cargo','test','--locked','--lib','warm_piper::tests','--','--test-threads=1']),
            ('piper-ipc','UNIT',['cargo','test','--locked','--lib','piper_protocol::tests','--','--test-threads=1']),
            ('piper-watch','UNIT',['cargo','test','--locked','--lib','piper_resource_watch::tests','--','--test-threads=1']),
            ('fast-migration','UNIT',['cargo','test','--locked','--lib','voice_fast','--','--test-threads=1']),
            ('process','UNIT',['cargo','test','--locked','--lib','voice_process::tests','--','--test-threads=1']),
            ('tts-stream','UNIT',['cargo','test','--locked','--lib','voice_pcm::tests','--','--test-threads=1']),
            ('tts-selection','UNIT',['cargo','test','--locked','--lib','voice_tts::tests','--','--test-threads=1']),
            ('stt-selection','UNIT',['cargo','test','--locked','--lib','voice_stt::tests','--','--test-threads=1']),
            ('vosk-guards','UNIT',['cargo','test','--locked','--lib','vosk::tests','--','--test-threads=1']),
            ('native-http','UNIT',['cargo','test','--locked','--lib','f4_native_tests','--','--test-threads=1']),
            ('audio','AUTOMATED_AUDIO_FIXTURE',['cargo','test','--locked','--test','f4_audio','--','--test-threads=1']),
            ('qml','AUTOMATED_QML',[str(runner),'-import',str(repo),'-input',str(repo/'tests/qml/tst_f4_voice.qml'),'-o','-,txt']),
        ]
        for name,category,command in commands:
            start=time.monotonic()
            result=subprocess.run(command,cwd=repo,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=600)
            (out/f'{name}.log').write_text(result.stdout)
            results.append(dict(name=name,category=category,command=command,exit=result.returncode,seconds=round(time.monotonic()-start,3)))
            (out/'results.json').write_text(json.dumps(results,indent=2))
            print(name,result.returncode,flush=True)
            if result.returncode:print(result.stdout)
        pipewire=fixture(repo,profile,out)
        (out/'pipewire.json').write_text(json.dumps(pipewire,indent=2))
        runtime_result=runtime(repo,profile,env,out)
        (out/'runtime.json').write_text(json.dumps(runtime_result,indent=2))
        (out/'main-lifecycle.json').write_text(json.dumps(validate_main(repo,profile,env,out),indent=2))
        # Voice source cannot dispatch effects, call LLM, request credentials or upload PCM.
        source=(repo/'src/backend/voice_coordinator.rs').read_text()
        assert all(word not in source for word in ('AiService','ToolExecutor','reqwest::','SttApiClient','execute_for_turn'))
        assert '/api/chat' not in (repo/'qml/components/VoiceController.qml').read_text()
    return int(any(item['exit'] for item in results))
if __name__=='__main__':raise SystemExit(main())
