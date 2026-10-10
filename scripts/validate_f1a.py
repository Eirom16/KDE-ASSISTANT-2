#!/usr/bin/env python3
"""Validación local F1A con fixtures privados, lock y selección sin modelos/red."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


PIPER_TESTS = [
    "backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag",
    "backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline",
    "backend::http_server::tests::dictate_stop_without_start_conflicts",
    "backend::speech_service::tests::piper_engine_initializes",
    "backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena",
    "backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening",
    "backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido",
    "backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening",
    "backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados",
    "backend::voice_pipeline::tests::signal_broadcast_state",
    "backend::voice_pipeline::tests::signal_level_throttled",
    "backend::voice_pipeline::tests::wait_for_speech_times_out_quiet",
]
EXTERNAL_TESTS = [
    "backend::wakeword_ml::tests::download_and_extract_onnx_runtime",
    "backend::tool_executor::tests::show_image_huge_url_is_rejected_before_download",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--mounts", action="store_true",
        help="Ejecutar además el test opt-in en un namespace user/mount privado.",
    )
    parser.add_argument(
        "--piper-inherited", action="store_true",
        help="Reproducir aparte los 12 tests heredados; sus fallos hacen fallar el script.",
    )
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    os.umask(0o077)
    evidence = Path(tempfile.mkdtemp(prefix="kda-f1a-check-"))
    env = os.environ.copy()
    for key in list(env):
        if key.endswith("_API_KEY") or key.startswith("KDA_F1A_"):
            env.pop(key)
    for key, directory in [
        ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"),
        ("XDG_DATA_HOME", "data"), ("TMPDIR", "tmp"),
    ]:
        path = evidence / directory
        path.mkdir(mode=0o700)
        env[key] = str(path)
    # Offline de Cargo no bloquea sockets de tests: se excluyen explícitamente
    # los tests de red/descarga y nunca se ejecutan main ni smokes de modelos.
    env["CARGO_NET_OFFLINE"] = "true"
    excluded = PIPER_TESTS + EXTERNAL_TESTS
    skips = [argument for name in excluded for argument in ["--skip", name]]
    commands = [
        ("fmt", ["cargo", "fmt", "--check"]),
        ("check", ["cargo", "check", "--locked", "--all-targets"]),
        ("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"]),
        ("security", ["cargo", "test", "--locked", "--test", "f1a_security"]),
        ("unit-safe", ["cargo", "test", "--locked", "--lib", "--", "--test-threads=1"] + skips),
        ("app-paths", ["cargo", "test", "--locked", "--test", "app_paths"]),
    ]
    if args.mounts:
        commands.append(("mounts", [
            "cargo", "test", "--locked", "--test", "f1a_security", "--", "--exact",
            "search_mount_boundaries_remain_closed", "--ignored", "--nocapture",
        ]))
    if args.piper_inherited:
        commands.append(("piper-inherited", [
            "cargo", "test", "--locked", "--lib", "--", "--exact", "--test-threads=1",
        ] + PIPER_TESTS))
    results = {
        "repo": str(repo), "excluded_unit_tests": excluded,
        "mount_test_opt_in": args.mounts, "piper_reproduction_opt_in": args.piper_inherited,
        "commands": [],
    }
    print(f"Evidencia privada: {evidence}", flush=True)
    print("Selección parcial: 12 tests de Piper y 2 de red/descarga excluidos de unit-safe.", flush=True)
    if not args.mounts:
        print("Test de montajes ignored por defecto; usar --mounts para validarlo.", flush=True)
    try:
        for name, command in commands:
            log = evidence / f"{name}.log"
            with log.open("w") as output:
                result = subprocess.run(command, cwd=repo, env=env, stdout=output, stderr=subprocess.STDOUT)
            results["commands"].append({"name": name, "command": command, "exit": result.returncode})
            (evidence / "results.json").write_text(json.dumps(results, indent=2) + "\n")
            print(f"{name}: exit {result.returncode}; log {log}", flush=True)
            if result.returncode:
                print(log.read_text(), flush=True)
                return 1
        print("PASS de la selección ejecutada; no certifica la suite completa ni voz/proveedores.", flush=True)
        return 0
    finally:
        # Sólo los fixtures creados por esta invocación; se conservan los logs.
        for directory in ["tmp", "config", "cache", "data"]:
            shutil.rmtree(evidence / directory)


if __name__ == "__main__":
    raise SystemExit(main())
