#!/usr/bin/env python3
"""Synthetic environment regressions. Does not launch the application."""
import os
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import tempfile
import unittest
from unittest.mock import patch
from f4_safety import XDG, private_env, require_private
from f4_runtime import runtime
from f4_qml_lifecycle import validate


class Safety(unittest.TestCase):
    def test_private_defaults_replace_profile_and_strip_credentials(self):
        with tempfile.TemporaryDirectory() as root:
            env = private_env(root, {'GROQ_API_KEY': 'synthetic', 'KDE_ASSISTANT_DATA_DIR': '/invalid',
                                     'PIPEWIRE_REMOTE': 'personal', 'LD_PRELOAD': '/invalid'})
            require_private(root, env)
            self.assertNotIn('GROQ_API_KEY', env)
            self.assertNotIn('LD_PRELOAD', env)
            self.assertTrue(Path(env['HOME']).is_relative_to(root))
            for key in {**XDG, 'HOME': 'home', 'TMPDIR': 'tmp'}:
                for value in ('', '/invalid', 'relative'):
                    invalid = {**env, key: value}
                    with self.assertRaises((ValueError, FileNotFoundError)):
                        require_private(root, invalid)

    def test_helpers_fail_before_spawning_or_listening_with_missing_isolation(self):
        with tempfile.TemporaryDirectory() as root:
            for helper in (runtime, validate):
                with patch('subprocess.Popen') as spawn, patch('socket.socket') as socket:
                    with self.assertRaises(ValueError):
                        helper(Path('.'), Path(root), {}, Path(root))
                    spawn.assert_not_called()
                    socket.assert_not_called()

    def test_socket_escape_credentials_permissions_and_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as root, tempfile.TemporaryDirectory() as outside:
            env = private_env(root, {})
            for bad in ({**env, 'XDG_DATA_HOME': outside},
                        {**env, 'HOME': outside},
                        {**env, 'TMPDIR': outside},
                        {**env, 'HOME': env['XDG_CONFIG_HOME']},
                        {**env, 'PIPEWIRE_REMOTE': 'pipewire-0'},
                        {**env, 'DBUS_SESSION_BUS_ADDRESS': 'unix:path=/invalid'},
                        {**env, 'OPENAI_API_KEY': 'synthetic'}):
                with self.assertRaises(ValueError): require_private(root, bad)
            p = Path(env['XDG_DATA_HOME'])
            p.chmod(0o755)
            with self.assertRaises(ValueError): require_private(root, env)
            p.chmod(0o700)
            p.rmdir()
            p.symlink_to(outside, target_is_directory=True)
            with self.assertRaises(ValueError): require_private(root, env)


if __name__ == '__main__': unittest.main()
