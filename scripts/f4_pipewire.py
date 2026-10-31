"""Private PipeWire daemon with a generated 440Hz source. No physical devices/modules."""
import json
import math
import os
from pathlib import Path
import struct
import subprocess
import time
from f3_runtime import wait_for

def fixture(repo, profile, out):
    runtime=profile/'virtual-pipewire';runtime.mkdir(mode=0o700)
    env={'PATH':'/usr/bin','XDG_RUNTIME_DIR':str(runtime),'XDG_CONFIG_HOME':str(runtime),'PIPEWIRE_REMOTE':'f4-synthetic'}
    with (out/'pipewire-daemon.log').open('w') as log:
        daemon=subprocess.Popen(['/usr/bin/pipewire','-c',str(repo/'tests/fixtures/f4/pipewire.conf')],env=env,stdout=log,stderr=log)
        recorder=None
        try:
            wait_for(lambda:(runtime/'f4-synthetic').exists())
            # Explicitly requested raw mono 16k output; source runs at 48k.
            with (runtime/'capture.pcm').open('wb') as output, (out/'pipewire-capture.log').open('w') as errors:
                recorder=subprocess.Popen(['/usr/bin/pw-record','--raw','--format','s16','--rate','16000','--channels','1','--quality','4','--target','f4-synthetic-source','--properties','{"node.dont-reconnect":true,"node.dont-fallback":true,"node.dont-move":true}','-'],env=env,stdout=output,stderr=errors)
                node=wait_for(lambda:next((n for n in json.loads(subprocess.check_output(['/usr/bin/pw-dump'],env=env,timeout=3)) if n.get('info',{}).get('props',{}).get('media.category')=='Capture'),None))
                subprocess.run(['/usr/bin/pw-cli','set-param',str(node['id']),'PortConfig','{ direction = Input mode = dsp format = { mediaType = audio mediaSubtype = raw format = F32P rate = 48000 channels = 1 position = [ MONO ] } }'],env=env,check=True,capture_output=True,timeout=3)
                port=wait_for(lambda:subprocess.check_output(['/usr/bin/pw-link','-i'],env=env,text=True,timeout=3).strip())
                subprocess.run(['/usr/bin/pw-link','f4-synthetic-source:capture_MONO',port],env=env,check=True,capture_output=True,timeout=3)
                wait_for(lambda:(runtime/'capture.pcm').stat().st_size>=64000,6)
                recorder.terminate();recorder.wait(timeout=3)
            pcm=(runtime/'capture.pcm').read_bytes();audio=[s[0]/32768 for s in struct.iter_unpack('<h',pcm[:len(pcm)//2*2])]
            trimmed=audio[1600:-1600]
            rms=math.sqrt(sum(x*x for x in trimmed)/len(trimmed))
            crossings=sum(a<=0<b for a,b in zip(trimmed,trimmed[1:]))
            hz=crossings/(len(trimmed)/16000)
            assert 0.69<rms<0.72 and abs(hz-440)<5, (rms,hz)
            result={'name':'pipewire-synthetic-capture','category':'AUTOMATED_AUDIO_FIXTURE','status':'PASS','source_rate':48000,'destination_rate':16000,'channels':1,'format':'PCM16LE','bytes':len(pcm),'rms':rms,'frequency_hz':hz,'physical_microphone':'NOT_TESTED','record_pid':recorder.pid,'daemon_pid':daemon.pid}
            return result
        finally:
            if recorder and recorder.poll() is None:recorder.kill();recorder.wait(timeout=3)
            daemon.terminate();daemon.wait(timeout=3)
            assert not Path(f'/proc/{daemon.pid}').exists()
            if recorder:assert not Path(f'/proc/{recorder.pid}').exists()
