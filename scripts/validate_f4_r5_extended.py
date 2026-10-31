#!/usr/bin/env python3
"""Extended prepared-Piper faults. Private null sink only; no microphone/downloads."""
import argparse,json,os,signal,shutil,sys,time,urllib.error
from pathlib import Path
sys.dont_write_bytecode=True
from validate_f4_r5 import Scenario,checked
from f4_safety import private_env,require_private
from f3_runtime import wait_for,children,metrics
from benchmark_f4r4 import TEXTS


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',type=Path,required=True);parser.add_argument('--resources',type=Path,required=True)
    a=parser.parse_args();base=a.profile.resolve(strict=True);resources=a.resources.resolve(strict=True)
    env=private_env(base);require_private(base,env);checked();os.umask(0o077)
    # AF_UNIX sun_path is 108 bytes including the terminator; keep private
    # runtime paths short instead of weakening isolation or extending waits.
    root=base/('e-'+str(time.time_ns()));root.mkdir(mode=0o700)
    repo=Path(__file__).resolve().parents[1];rows=[]
    def record(name,**data):
        rows.append(dict(test=name,status='PASS',**data))
        (root/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print('PASS',name,flush=True)
    case_number=0
    def scenario(name,binary=None):
        nonlocal case_number
        case_number+=1
        return Scenario(root/('c'+str(case_number)),resources,binary)
    def finish(s):
        wait_for(lambda:s.status()['state']=='speaking',40);wait_for(s.link,5);wait_for(lambda:s.status()['state']=='idle',20)
    def own_signal(s,pid,value):
        assert pid in children(s.app.pid)
        os.kill(pid,value)
    s=scenario('pending-expiry')
    try:
     s.configure(True,ttl=15,auto=True);row=s.render('pending_ttl_expired',chat=True,delay=35)
     assert row['fast']['prepares']==2 and len(s.calls)==1
     record('llm_pending_model_expired_safe_reload',row=row)
    finally:s.close()
    s=scenario('maxlife')
    try:
     s.configure(True,ttl=15);series=[];pids=[]
     for i in range(8):
      series.append(s.render('absolute_lease',i));pids.append(s.status()['fast']['worker_pid'])
     assert len(set(pids))>=2 and not Path(f'/proc/{pids[0]}').exists()
     record('absolute_lifetime_not_extended_by_repeated_reads',series=series)
    finally:s.close()
    s=scenario('timeout')
    try:
     s.configure(True);s.render('timeout_setup');pid=s.status()['fast']['worker_pid'];own_signal(s,pid,signal.SIGSTOP)
     s.speak();start=time.monotonic();wait_for(lambda:s.status()['state']=='error',22)
     assert not Path(f'/proc/{pid}').exists() and not children(s.app.pid)
     record('reserved_worker_timeout_and_reap',ms=(time.monotonic()-start)*1000)
     # Freeze the load before Ready. The existing 90s deadline is exercised, not enlarged.
     s.configure(True);s.speak();pid=wait_for(lambda:s.status()['fast']['worker_pid'],5);own_signal(s,pid,signal.SIGSTOP);start=time.monotonic()
     wait_for(lambda:s.status()['state']=='error',100)
     assert not Path(f'/proc/{pid}').exists() and not children(s.app.pid)
     record('preparing_worker_timeout_and_reap',ms=(time.monotonic()-start)*1000)
    finally:s.close()
    s=scenario('cancel-preparing')
    try:
     for i in range(5):
      s.configure(True,auto=True);s.delay=3;s.select();answer={}
      def ask():
       import urllib.error
       try:answer['value']=s.api('/api/chat/complete',{'session_id':s.sid,'message':'Texto sintético cancelable'})
       except urllib.error.HTTPError as e:answer['http_status']=e.code;e.read()
      import threading
      t=threading.Thread(target=ask);t.start();pid=wait_for(lambda:s.status()['fast']['worker_pid'],5)
      assert s.status()['state']=='idle';turn=s.api('/api/chat/turn?session_id='+s.sid);start=time.monotonic()
      s.api('/api/chat/cancel',{'session_id':s.sid,'turn_id':turn['turn_id']});t.join(5)
      assert answer.get('http_status')==409 and not t.is_alive() and not children(s.app.pid) and not Path(f'/proc/{pid}').exists()
      record('llm_cancel_preparing_'+str(i),ms=(time.monotonic()-start)*1000)
    finally:s.close()
    s=scenario('barge')
    try:
     s.configure(True);snap=s.speak();wait_for(lambda:s.status()['state']=='speaking',40);wait_for(s.link,5);old=children(s.app.pid)
     newer=s.speak();assert newer['identity']!=snap['identity'] and all(not Path(f'/proc/{pid}').exists() for pid in old)
     try:s.speak();raise AssertionError('Busy synthesis admitted')
     except __import__('urllib.error',fromlist=['HTTPError']).HTTPError as e:assert e.code==409;e.read()
     finish(s);record('barge_in_full_drain_and_busy_rejected')
     s.speak();wait_for(lambda:s.status()['state']=='speaking',20);wait_for(s.link,5);owned=children(s.app.pid);start=time.monotonic();s.app.terminate();s.app.wait(timeout=10)
     wait_for(lambda:all(not Path(f'/proc/{pid}').exists() for pid in owned),5);record('shutdown_during_null_playback',ms=(time.monotonic()-start)*1000)
    finally:s.close()
    private_binary=root/'binary-watch-copy';shutil.copyfile(repo/'target/debug/kde-assistant',private_binary);private_binary.chmod(0o700)
    s=scenario('binary-watch',private_binary)
    try:
     s.configure(True);s.render('binary_watch_setup');pid=s.status()['fast']['worker_pid'];private_binary.chmod(0o500);start=time.monotonic();wait_for(lambda:not children(s.app.pid),5)
     assert not Path(f'/proc/{pid}').exists();record('private_binary_identity_change_revoked',ms=(time.monotonic()-start)*1000)
    finally:s.close();private_binary.unlink()

    s=scenario('expiry-buffered-playback')
    try:
     s.configure(True,ttl=15);s.speak(TEXTS['multi']*3)
     wait_for(lambda:s.status()['state']=='speaking',70);wait_for(s.link,5)
     wait_for(lambda:s.status()['fast']['state']=='cold',35)
     assert s.status()['state']=='speaking', 'TTL cancelled a valid complete PCM buffer'
     wait_for(lambda:s.status()['state']=='idle',40)
     record('normal_ttl_expiry_does_not_cancel_buffered_playback')
    finally:s.close()
    s=scenario('stt-admission')
    try:
     s.configure(True,stt_engine='whisper',stt_model='/nonexistent/f4r5-no-microphone.bin')
     s.render('stt_admission_setup');pid=s.status()['fast']['worker_pid'];scope=s.select()
     s.api('/api/voice/start',{'session_id':s.sid,'scope':scope})
     assert not Path(f'/proc/{pid}').exists()
     wait_for(lambda:s.status()['state']=='error',10)
     assert not children(s.app.pid)
     record('stt_admission_reaps_piper_missing_stt_prevents_microphone')
    finally:s.close()
    print('RESULT',root/'results.json',flush=True)

if __name__=='__main__':main()
