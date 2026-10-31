"""Disposable, bounded CPU inference. Rust owns validation, lifetime and playback.

No devices, downloads, conversation authority, server, or audio files.
"""
import hashlib
import json
import os
import struct
import select
import sys
import time

started = time.perf_counter()
python_entered_wall_ms = time.time()*1000
if len(sys.argv) not in (7, 10):
    raise ValueError('Invalid Piper worker arguments')
site_packages, model, scale, speaker, threads, optimization = sys.argv[1:7]
mode, config_path, diagnostics = ('raw', model + '.json', {}) if len(sys.argv) == 7 else (
    sys.argv[7], sys.argv[8], json.loads(sys.argv[9]))
if mode not in ('raw', 'framed', 'warm-experiment', 'managed-v1'):
    raise ValueError('Invalid Piper protocol')
scale, speaker, threads = float(scale), int(speaker), int(threads)
if not 0.5 <= scale <= 2 or not 0 <= speaker < 2 or not 1 <= threads <= 3:
    raise ValueError('Invalid Piper synthesis options')
def request():
    # Benchmark only: two requests maximum, three seconds idle TTL, no server.
    deadline = time.monotonic() + 3
    def exact(size):
        data = bytearray()
        while len(data) < size:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not select.select([0], [], [], remaining)[0]:
                raise TimeoutError('Prepared experiment idle TTL')
            block = os.read(0, size - len(data))
            if not block:
                raise EOFError('Prepared experiment input closed')
            data.extend(block)
        return bytes(data)
    size = struct.unpack('<I', exact(4))[0]
    if not 0 < size <= 16000:
        raise ValueError('Invalid prepared request size')
    return exact(size)


def managed_packet(timeout):
    deadline = time.monotonic() + timeout
    def exact(size):
        data = bytearray()
        while len(data) < size:
            remaining = deadline-time.monotonic()
            if remaining <= 0 or not select.select([0], [], [], remaining)[0]:
                raise TimeoutError('Managed Piper deadline')
            block = os.read(0, size-len(data))
            if not block:
                raise EOFError('Managed Piper input closed')
            data.extend(block)
        return bytes(data)
    size = struct.unpack('<I', exact(4))[0]
    if not 0 < size <= 131072:
        raise ValueError('Managed control limit')
    value = json.loads(exact(size))
    if not isinstance(value, dict) or value.get('protocol') != 1:
        raise ValueError('Managed Piper protocol')
    return value


def uuid_token(value):
    import uuid
    if not isinstance(value, str) or str(uuid.UUID(value)) != value:
        raise ValueError('Invalid opaque identity')
    return value


def decode_text(data):
    if not data.strip() or len(data) > 16000:
        raise ValueError('Invalid Piper input length')
    return data.decode('utf-8')


if mode == 'managed-v1':
    init = managed_packet(90)
    if set(init) != {'protocol', 'kind', 'generation', 'ttl_secs'} or init['kind'] != 'init':
        raise ValueError('Invalid managed initialization')
    generation = uuid_token(init['generation'])
    ttl_secs = init['ttl_secs']
    if type(ttl_secs) is not int or not 15 <= ttl_secs <= 60:
        raise ValueError('Invalid managed TTL')
    text = ''
else:
    text = decode_text(request() if mode == 'warm-experiment' else sys.stdin.buffer.read(16001))


def check_identity(path, key):
    if key not in diagnostics:
        return  # Legacy offline diagnostic invocation; production always pins FDs.
    info = os.stat(path)
    actual = [info.st_dev, info.st_ino, info.st_size,
              info.st_mtime_ns // 10**9, info.st_mtime_ns % 10**9,
              info.st_ctime_ns // 10**9, info.st_ctime_ns % 10**9]
    if actual != diagnostics[key]:
        raise ValueError('Verified voice resource changed')


def packet(value):
    encoded = json.dumps(value).encode()
    if len(encoded) > 8192:
        raise ValueError('Diagnostic limit')
    sys.stdout.buffer.write(struct.pack('<I', len(encoded)))
    sys.stdout.buffer.write(encoded)


# -S avoids sitecustomize and executable .pth files, including those in a venv.
sys.path.insert(0, site_packages)
ort_started = time.perf_counter()
import onnxruntime
ort_imported = time.perf_counter()
from piper import PiperVoice, SynthesisConfig
from piper.config import PiperConfig
imported = time.perf_counter()
check_identity(model, 'model_identity')
check_identity(config_path, 'config_identity')
with open(config_path, 'r', encoding='utf-8') as source:
    config = PiperConfig.from_dict(json.load(source))
if config.espeak_voice != 'es' or config.sample_rate != 22050 or config.num_speakers != 2:
    raise ValueError('Unsupported voice configuration')
configured = time.perf_counter()
options = onnxruntime.SessionOptions()
options.graph_optimization_level = {
    'all': onnxruntime.GraphOptimizationLevel.ORT_ENABLE_ALL,
    'all-nopack': onnxruntime.GraphOptimizationLevel.ORT_ENABLE_ALL,
    'all-parallelpack': onnxruntime.GraphOptimizationLevel.ORT_ENABLE_ALL,
    'basic': onnxruntime.GraphOptimizationLevel.ORT_ENABLE_BASIC,
    'disabled': onnxruntime.GraphOptimizationLevel.ORT_DISABLE_ALL,
}[optimization]
if optimization == 'all-nopack':
    options.add_session_config_entry('session.disable_prepacking', '1')
if optimization == 'all-parallelpack':
    options.add_session_config_entry('session.prepack.enable_parallel', '1')
options.intra_op_num_threads = threads
options.inter_op_num_threads = 1
options.add_session_config_entry('session.intra_op.allow_spinning', '0')
options.add_session_config_entry('session.inter_op.allow_spinning', '0')
session_started = time.perf_counter()
voice = PiperVoice(config=config, session=onnxruntime.InferenceSession(
    model, sess_options=options, providers=['CPUExecutionProvider']))
loaded = time.perf_counter()
check_identity(model, 'model_identity')
check_identity(config_path, 'config_identity')
if diagnostics:
    os.close(int(model.rsplit('/', 1)[1]))
    os.close(int(config_path.rsplit('/', 1)[1]))
# Wrappers measure actual library stages without changing text, normalization or PCM.
pending_begin = None
managed_started = False
stages = {'phonemize_ms': 0., 'phoneme_ids_ms': 0., 'inference_ms': 0.}
for method, metric in [('phonemize', 'phonemize_ms'), ('phonemes_to_ids', 'phoneme_ids_ms'),
                       ('phoneme_ids_to_audio', 'inference_ms')]:
    original = getattr(voice, method)
    def measured(*args, _call=original, _metric=metric, **kwargs):
        global pending_begin, managed_started
        if _metric == 'inference_ms' and pending_begin is not None:
            packet(pending_begin)
            sys.stdout.buffer.flush()
            pending_begin = None
            managed_started = True
        before = time.perf_counter()
        result = _call(*args, **kwargs)
        stages[_metric] += (time.perf_counter() - before) * 1000
        return result
    setattr(voice, method, measured)
def synthesize(text, request_index, operation_started, managed=None):
    global pending_begin, managed_started
    synthesis_started = time.perf_counter()
    if managed is not None:
        managed_started = False
        pending_begin = dict(protocol=1, kind='begin', generation=generation, request_id=managed)
    elif mode != 'raw':
        sys.stdout.buffer.write(b'KDA4PCM1')
        packet(dict(sample_rate=22050, channels=1, sample_width=2))
        sys.stdout.buffer.flush()
    first = None
    count = 0
    digest = hashlib.sha256()
    chunk_count = 0
    first_chunk_bytes = 0
    supply_gap_ms = 0.
    conversion_ms = ipc_ms = 0.
    for chunk in voice.synthesize(text, SynthesisConfig(speaker_id=speaker, length_scale=scale)):
        if chunk.sample_rate != 22050 or chunk.sample_width != 2 or chunk.sample_channels != 1:
            raise ValueError('Invalid PCM format')
        converted = time.perf_counter()
        pcm = chunk.audio_int16_bytes
        conversion_ms += (time.perf_counter() - converted) * 1000
        ready = time.perf_counter()
        if first is not None:
            supply_gap_ms = max(supply_gap_ms, (ready-first)*1000 - count/44.1)
        else:
            first_chunk_bytes = len(pcm)
        chunk_count += 1
        digest.update(pcm)
        count += len(pcm)
        if not pcm or len(pcm) % 2 or count > 16 * 1024 * 1024:
            raise ValueError('PCM output exceeds limit or malformed')
        if first is None:
            first = time.perf_counter()
        writing = time.perf_counter()
        # One frame buffered; blocking stdout provides backpressure to inference.
        for offset in range(0, len(pcm), 32768):
            part = pcm[offset:offset+32768]
            if mode != 'raw':
                sys.stdout.buffer.write(struct.pack('<I', len(part)))
            sys.stdout.buffer.write(part)
            sys.stdout.buffer.flush()
        ipc_ms += (time.perf_counter() - writing) * 1000
    if not count:
        raise ValueError('Empty PCM')
    result = dict(python_bootstrap_wall_estimate_ms=(python_entered_wall_ms-diagnostics['exec_wall_ms']) if 'exec_wall_ms' in diagnostics else None,
                  import_ms=(imported-started)*1000, ort_import_ms=(ort_imported-ort_started)*1000,
                  piper_import_ms=(imported-ort_imported)*1000, config_ms=(configured-imported)*1000,
                  session_ms=(loaded-session_started)*1000, load_ms=(loaded-imported)*1000,
                  first_pcm_ms=(first-operation_started)*1000,
                  prepared=request_index>0, load_per_request_ms=0. if request_index else (loaded-imported)*1000,
                  chunk_count=chunk_count, first_chunk_bytes=first_chunk_bytes,
                  supply_gap_ms=max(0.,supply_gap_ms), synthesis_ms=(time.perf_counter()-synthesis_started)*1000,
                  pcm_conversion_ms=conversion_ms, ipc_write_ms=ipc_ms, pcm_bytes=count,
                  sample_rate=22050, channels=1, sample_width=2, threads=threads,
                  optimization=optimization, providers=voice.session.get_providers(),
                  native=diagnostics.get('native', {}), **stages)
    if mode != 'raw':
        sys.stdout.buffer.write(struct.pack('<I', 0))
        if managed is not None:
            result.update(protocol=1, kind='done', generation=generation, request_id=managed,
                          pcm_sha256=digest.hexdigest())
        packet(result)
        sys.stdout.buffer.flush()
    print(json.dumps(result), file=sys.stderr)

if mode == 'managed-v1':
    packet(dict(protocol=1, kind='ready', generation=generation, sample_rate=22050,
                channels=1, sample_width=2, ready_ms=(time.perf_counter()-started)*1000))
    sys.stdout.buffer.flush()
    lease_end = time.monotonic() + ttl_secs*2
    index = 0
    while True:
        request_id = None
        managed_started = False
        try:
            remaining = min(ttl_secs, lease_end-time.monotonic())
            if remaining <= 0:
                raise TimeoutError('Managed lifetime')
            value = managed_packet(remaining)
            if time.monotonic() >= lease_end:
                raise TimeoutError('Managed lifetime')
            if set(value) != {'protocol', 'kind', 'generation', 'request_id'} or value['kind'] != 'reserve' or value['generation'] != generation:
                raise ValueError('Invalid managed reservation')
            request_id = uuid_token(value['request_id'])
            packet(dict(protocol=1, kind='reserved', generation=generation, request_id=request_id))
            sys.stdout.buffer.flush()
            value = managed_packet(90)
            if set(value) != {'protocol', 'kind', 'generation', 'request_id', 'text'} or value['kind'] != 'synthesize' or value['generation'] != generation or value['request_id'] != request_id:
                raise ValueError('Invalid managed request')
            text = decode_text(value['text'].encode('utf-8'))
            for key in stages:
                stages[key] = 0.
            synthesize(text, index, time.perf_counter(), managed=request_id)
            index += 1
            text = ''
        except (TimeoutError, EOFError):
            packet(dict(protocol=1, kind='expired', generation=generation))
            sys.stdout.buffer.flush()
            break
        except Exception:
            # Never transport exception repr, input text or partial PCM as a success.
            if managed_started:
                sys.stdout.buffer.write(struct.pack('<I', 0xffffffff))
            packet(dict(protocol=1, kind='error', generation=generation,
                        request_id=request_id, code='piper_failed'))
            sys.stdout.buffer.flush()
            raise SystemExit(1)
else:
    synthesize(text, 0, started)
if mode == 'warm-experiment':
    try:
        text = decode_text(request())
        for key in stages:
            stages[key] = 0.
        synthesize(text, 1, time.perf_counter())
        # Keep the second completed model idle only until the fixed experiment TTL.
        # A third request is rejected; the parent owns cancellation/reaping.
        try:
            request()
            raise ValueError('Prepared experiment request budget exceeded')
        except (TimeoutError, EOFError):
            pass
    except (TimeoutError, EOFError):
        pass
