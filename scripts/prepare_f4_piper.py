#!/usr/bin/env python3
"""Install an explicitly authorized, hash-pinned Piper evaluation in a private profile."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import time
import urllib.request
import zipfile
sys.dont_write_bytecode = True
from f4_safety import private_env, require_private


def verify_file(path, size, digest, git_blob=False):
    data = path.read_bytes()
    if len(data) != size:
        raise ValueError('Unexpected size: ' + path.name)
    actual = (hashlib.sha1(b'blob ' + str(size).encode() + b'\0' + data).hexdigest()
              if git_blob else hashlib.sha256(data).hexdigest())
    if actual != digest:
        raise ValueError('Integrity mismatch: ' + path.name)
    return hashlib.sha256(data).hexdigest()


def fetch(url, path, size, digest, git_blob=False):
    if not path.exists():
        partial = path.with_suffix(path.suffix + '.partial')
        try:
            opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
            with opener.open(url, timeout=30) as response, partial.open('xb') as output:
                os.chmod(partial, 0o600)
                count = 0
                while chunk := response.read(min(1024 * 1024, size + 1 - count)):
                    count += len(chunk)
                    if count > size:
                        raise ValueError('Download exceeds approved size')
                    output.write(chunk)
            verify_file(partial, size, digest, git_blob)
            partial.rename(path)
        finally:
            partial.unlink(missing_ok=True)
    return verify_file(path, size, digest, git_blob)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--authorization', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    if subprocess.check_output(['git', 'branch', '--show-current'], cwd=repo, text=True).strip() != 'recovery/f4-voice-coordinator':
        raise ValueError('Unexpected branch')
    if subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip() != '9544a58eba4b6adf6c6343c04db80813b084f76b':
        raise ValueError('Unexpected HEAD')
    inventory = json.loads(args.inventory.read_text())
    authorization = json.loads(args.authorization.read_text())
    if authorization != {'scope': 'download-private-install-offline-pcm', 'authorized': True,
                          'inventory_sha256': hashlib.sha256(args.inventory.read_bytes()).hexdigest(),
                          'user_statement': 'Lo autorizo', 'date': '2026-10-19'}:
        raise ValueError('Missing exact authorization receipt')
    root = Path(inventory['private_destination']).resolve(strict=True)
    private = private_env(root)
    env = {key: private[key] for key in ['HOME', 'TMPDIR', 'XDG_CONFIG_HOME',
           'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'XDG_RUNTIME_DIR', 'PIPEWIRE_REMOTE',
           'DBUS_SESSION_BUS_ADDRESS']}
    env.update(PATH='/usr/bin', LANG='C.UTF-8')
    require_private(root, env)
    os.umask(0o077)
    wheels = root / 'wheels'
    voice = root / 'voice'
    wheels.mkdir(mode=0o700, exist_ok=True)
    voice.mkdir(mode=0o700, exist_ok=True)
    verified = []
    for package in inventory['packages']:
        filename = package['filename']
        if Path(filename).name != filename or not package['url'].startswith('https://files.pythonhosted.org/'):
            raise ValueError('Unexpected wheel source/path')
        path = wheels / filename
        digest = fetch(package['url'], path, package['bytes'], package['sha256'])
        with zipfile.ZipFile(path) as archive:
            for entry in archive.infolist():
                name = PurePosixPath(entry.filename)
                if name.is_absolute() or '..' in name.parts or (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('Unsafe wheel member')
            metadata = [n for n in archive.namelist() if n.endswith('.dist-info/METADATA')]
            if len(metadata) != 1:
                raise ValueError('Ambiguous wheel metadata')
            (root / (package['name'] + '-METADATA')).write_bytes(archive.read(metadata[0]))
            for name in archive.namelist():
                if name.endswith('.dist-info/entry_points.txt'):
                    (root / (package['name'] + '-entry_points.txt')).write_bytes(archive.read(name))
        verified.append(dict(name=filename, bytes=package['bytes'], sha256=digest))
        print('verified', filename, flush=True)
    for item in inventory['voice']['files']:
        url = 'https://huggingface.co/' + inventory['voice']['repository'] + '/resolve/' + inventory['voice']['revision'] + '/' + item['path']
        digest = fetch(url, voice / Path(item['path']).name, item['size'],
                       item.get('lfs', {}).get('oid', item['oid']), git_blob='lfs' not in item)
        verified.append(dict(name=Path(item['path']).name, bytes=item['size'], sha256=digest))
        print('verified', item['path'], flush=True)
    config = json.loads((voice / 'es_ES-sharvard-medium.onnx.json').read_text())
    if config['audio']['sample_rate'] != 22050 or config['num_speakers'] != 2:
        raise ValueError('Unexpected voice configuration')
    venv = root / 'venv'
    if venv.exists():
        raise ValueError('Refusing to overwrite a pre-existing venv')
    require_private(root, env)
    subprocess.run([sys.executable, '-I', '-B', '-m', 'venv', str(venv)], env=env, check=True, timeout=120)
    requirements = root / 'requirements.txt'
    requirements.write_text(''.join(p['name'] + '==' + p['version'] + ' --hash=sha256:' + p['sha256'] + '\n' for p in inventory['packages']))
    command = [str(venv / 'bin/python'), '-I', '-B', '-m', 'pip', '--isolated', 'install', '--no-index', '--require-hashes',
               '--no-compile', '--disable-pip-version-check', '--find-links', str(wheels), '-r', str(requirements)]
    require_private(root, env)
    with (root / 'install.log').open('w') as output:
        subprocess.run(command, env=env, stdout=output, stderr=subprocess.STDOUT, check=True, timeout=180)
    result = dict(authorized_scope=authorization['scope'], files=verified, install_command=command,
                  timestamp=time.time(), microphone=False, playback=False)
    (root / 'installation.json').write_text(json.dumps(result, indent=2) + '\n')
    print('Private installation complete', flush=True)


if __name__ == '__main__':
    main()
