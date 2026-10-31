"""Fail-closed environment for F4 validation helpers; never infer a desktop profile."""
import os
from pathlib import Path

XDG = {'XDG_CONFIG_HOME': 'config', 'XDG_DATA_HOME': 'data',
       'XDG_CACHE_HOME': 'cache', 'XDG_RUNTIME_DIR': 'runtime'}


def private_env(profile, base=None):
    profile = Path(profile).resolve(strict=True)
    if profile.stat().st_uid != os.getuid() or profile.stat().st_mode & 0o077:
        raise ValueError('Private profile must be owned by this UID with mode 0700')
    inherited = dict(os.environ if base is None else base)
    # Keep toolchain caches explicit before replacing HOME. These are build inputs,
    # never the application profile. No package downloads are allowed by this runner.
    cargo = inherited.get('CARGO_HOME', str(Path.home() / '.cargo'))
    rustup = inherited.get('RUSTUP_HOME', str(Path.home() / '.rustup'))
    env = {k: v for k, v in inherited.items()
           if not k.endswith('_API_KEY') and not k.upper().endswith('_PROXY')
           and not k.startswith(('KDA_', 'KDE_ASSISTANT_', 'PIPEWIRE_', 'PULSE_', 'ORT_', 'LD_'))}
    for key, name in {**XDG, 'HOME': 'home', 'TMPDIR': 'tmp'}.items():
        directory = profile / name
        directory.mkdir(mode=0o700, exist_ok=True)
        env[key] = str(directory)
    env.update(CARGO_HOME=cargo, RUSTUP_HOME=rustup, CARGO_NET_OFFLINE='true',
               PIPEWIRE_REMOTE='f4-unreachable-audio',
               DBUS_SESSION_BUS_ADDRESS='unix:path=' + str(profile / 'absent-bus'))
    require_private(profile, env)
    return env


def require_private(profile, env):
    """Called at entry to every helper that can start the F4 application."""
    if env is None:
        raise ValueError('An explicit private environment is required')
    profile = Path(profile).resolve(strict=True)
    if profile.stat().st_uid != os.getuid() or profile.stat().st_mode & 0o077:
        raise ValueError('Unsafe profile ownership or permissions')
    selected = []
    for key in {**XDG, 'HOME': 'home', 'TMPDIR': 'tmp'}:
        raw = env.get(key, '')
        p = Path(raw)
        if not raw or not p.is_absolute() or p.is_symlink():
            raise ValueError('Missing, relative or symlinked ' + key)
        resolved = p.resolve(strict=True)
        if not resolved.is_relative_to(profile) or resolved == profile:
            raise ValueError('Profile escape: ' + key)
        if not resolved.is_dir() or resolved.stat().st_uid != os.getuid() or resolved.stat().st_mode & 0o077:
            raise ValueError('Unsafe XDG directory: ' + key)
        selected.append(resolved)
    if len(set(selected)) != len(selected):
        raise ValueError('HOME, TMPDIR and XDG directories must be distinct')
    if env.get('PIPEWIRE_REMOTE') != 'f4-unreachable-audio':
        raise ValueError('Unexpected audio socket')
    if env.get('DBUS_SESSION_BUS_ADDRESS') != 'unix:path=' + str(profile / 'absent-bus'):
        raise ValueError('Unexpected session bus')
    if any(k.endswith('_API_KEY') or k.startswith('KDE_ASSISTANT_') for k in env):
        raise ValueError('Credentials/application overrides are forbidden')
