# Resultados verificables de Fase 0

Fecha 2026-10-10. Estado global **PARTIAL**. No se instalaron dependencias, descargaron modelos, enviaron prompts a proveedores reales ni ejecutaron acciones destructivas sobre archivos/apps reales. Se crearon estos informes y fixtures fuera del repositorio. No hubo commit/push ni cambio de producción.

## Baseline y entorno

| Dato | Resultado observado |
| --- | --- |
| Repositorio | `/home/eirom/KDE-ASSISTANT-2`; origin `https://github.com/Eirom16/KDE-ASSISTANT-2` |
| Rama/HEAD | `main`, `12da960ed4722401f1cb42b7a96d9cef912cf90a` |
| Commit | `before the migration to omarchy`, 2026-09-24 17:18:06 -0400 |
| Comparación | `git rev-list --left-right --count HEAD...origin/main`: `0 0`; `git ls-remote origin refs/heads/main`: mismo SHA. No se alteró origin/main. |
| Cambio previo | Sólo `src/backend/hotkey_listener.rs`: importa Context y elimina conversión duplicada. Conservado. |
| Sistema | CachyOS rolling, Linux `7.2.7-1-cachyos`, x86_64, sesión KDE Wayland |
| Rust/Cargo | rustc `1.99.0 (b940084d7 2026-09-28)`; cargo `1.99.0 (5f94df478 2026-08-27)` |
| Qt build | qmake6 3.1, Qt `6.11.2` en `/usr/lib` |
| Qt runtime | `qml6 --version`: `Qml Runtime 6.12.0`; diferencia observada, ABI/compatibilidad completa no probada |
| Compositor | `kwin_wayland --version`: 6.7.5 |
| Python | 3.14.7; AST de main.py y spike_bot.py parsea correctamente |
| Native build | CMake 4.4.4, clang 23.1.1, ALSA pkg-config 1.2.16.1 |
| Piper | `piper-tts` y `piper` ausentes en PATH y rutas comunes del constructor |
| Utilidades ausentes | xdotool, kdotool, wmctrl, playerctl, brightnessctl |
| Utilidades presentes | gtk-launch, xdg-open, wpctl, pactl, nmcli, bluetoothctl, kdeconnect-cli, pdftoppm, dbus-send |
| Python módulos | numpy/onnxruntime encontrables; aiohttp/openwakeword/pipecat no encontrables en intérprete actual. No se validó otra venv. |
| ONNX/modelos preexistentes | `/usr/lib/libonnxruntime.so` presente; grupo de 6 archivos de modelos 151.637.371 bytes; grupo de 4 archivos de librerías 85.507.888 bytes. No acredita que todos los modelos sean utilizables. |
| Qt herramientas | `/usr/lib/qt6/bin/qmllint` y qmlformat presentes; qmllint6 no en PATH |

`plasmashell --version` terminó con señal 11 (returncode -11), sin salida. Fue el proceso de consulta, no evidencia de crash del plasmashell ya activo. Antes de pruebas no había procesos kde-assistant/qml6/piper ni listeners 8765/8766. No se inspeccionaron ni imprimieron claves reales.

Cargo usa el lock local existente, caché y `CARGO_NET_OFFLINE=true`; se retiraron del entorno de pruebas OPENROUTER_API_KEY/GROQ_API_KEY/OPENAI_API_KEY. Este flag sólo impide red de Cargo: no impide reqwest dentro de tests. Una prueba ONNX puede descargar si falta bundle; aquí retornó por bundle existente (`src/backend/wakeword_ml.rs:520`), no hubo descarga. No se ejecutó main completo: su ensure_models y cull_stale_qml tienen efectos no adecuados para esta auditoría sin aislamiento adicional.

## Validación requerida: árbol de trabajo

| Comando exacto | Exit | Duración total del comando | Resultado |
| --- | --- | --- | --- |
| `cargo fmt --check` | 1 | 0,583 s | Diff en `src/backend/hotkey_listener.rs:419` y `src/lib.rs:6`. No se aplicó formato. |
| `cargo check --all-targets` | 0 | 10,225 s | PASS del árbol local modificado previamente. |
| `cargo test --all-targets` | 101 | 47,404 s | **122 passed; 12 failed; 0 ignored; 0 measured; 0 filtered out** en lib; runtime tests 0,24 s. Cargo se detuvo antes de ejecutar targets de integración. |
| `cargo clippy --all-targets -- -D warnings` | 0 | 15,070 s | PASS; no warnings de Clippy. |

Los 12 tests fallaron al construir SpeechService/Piper, no por un resultado de audio medido:

```text
backend::http_server::tests::dictate_roundtrip_empty_transcript_releases_flag
backend::http_server::tests::dictate_start_conflicts_with_voice_pipeline
backend::http_server::tests::dictate_stop_without_start_conflicts
backend::speech_service::tests::piper_engine_initializes
backend::voice_pipeline::tests::auto_barge_ignora_mic_alto_si_tts_aun_no_suena
backend::voice_pipeline::tests::barge_in_transita_interrupted_a_listening
backend::voice_pipeline::tests::dictado_activo_aborta_wait_for_speech_rapido
backend::voice_pipeline::tests::dictado_activo_bloquea_start_listening
backend::voice_pipeline::tests::emit_state_mapea_a_strings_legados
backend::voice_pipeline::tests::signal_broadcast_state
backend::voice_pipeline::tests::signal_level_throttled
backend::voice_pipeline::tests::wait_for_speech_times_out_quiet
```

Errores: `inicializando Piper TTS` → `piper-tts no encontrado`. Fixtures panican en `src/backend/http_server.rs:2034`, `speech_service.rs:473` y `voice_pipeline.rs:1120`. No se considera voz funcional porque otros tests de VAD/chunker/estado puro pasen.

### Commit publicado separado del árbol local

Se extrajo `git archive HEAD` a `/tmp/kde-recovery-audit-0hvcyyku/committed-head`, sin checkout/reset del workspace, y se copió el Cargo.lock local para usar la misma resolución. Comando:

```text
cargo check --all-targets --manifest-path /tmp/kde-recovery-audit-0hvcyyku/committed-head/Cargo.toml --target-dir /home/eirom/KDE-ASSISTANT-2/target
```

Exit **101**, 20,022 s; cuatro E0599 por `Context` fuera de scope, líneas HEAD 320/326/331/334 de hotkey_listener, reportados para lib/lib test. No se probó un build release ni una resolución fresca sin lock; no se ejecutó la suite sobre HEAD porque no compila. El check local PASS no corrige el commit remoto.

### Integración existente ejecutada explícitamente

```text
cargo test --test app_paths --test agent_smoke --test tts_stream_smoke --test wake_resample_sim
```

Exit **0**, 13,909 s, con claves retiradas:

- app_paths: **2 passed**, helpers puros; main no usa el resolver.
- agent_smoke: Cargo imprime **1 passed**, pero el cuerpo retorna sin key (`tests/agent_smoke.rs:25`). Estado real **SKIPPED/NOT_TESTED**, ninguna llamada OpenRouter.
- tts_stream_smoke: **1 ignored**, no síntesis/reproducción.
- wake_resample_sim: **1 ignored**, no inferencia ONNX/dataset.

## QML

Para runtime: `QT_QPA_PLATFORM=offscreen`, `QML_DISABLE_DISK_CACHE=1`, `QML_XHR_ALLOW_FILE_READ=1`; XDG_CACHE_HOME/XDG_CONFIG_HOME/XDG_DATA_HOME temporales, sin cambiar HOME. Sin backend ni módulo de token de producción.

| Comando | Resultado exacto | Alcance |
| --- | --- | --- |
| `qml6 -I . -apptype widget tests/qml/AgentUiHarness.qml` | exit 0, 1,379 s, `qml: AgentUiHarness: ok` | PASS assertions del harness de componentes/estados simulados. |
| `qml6 -I . -apptype widget qml/agent/DevPreview.qml` | vivo a 4 s; SIGTERM del auditor, exit -15, 4,015 s; sin stderr | LOAD_SMOKE, no prueba funcional. |
| `qml6 -I . -apptype widget qml/Main.qml` | vivo a 4 s; SIGTERM del auditor, exit -15, 4,023 s | LOAD_SMOKE con warnings, sin auth ni backend. |
| `/usr/lib/qt6/bin/qmllint -I . qml/Main.qml qml/components/SettingsDialog.qml qml/agent/AgentOverlay.qml` | exit 0, 2,281 s; **44 Warning, 0 Error** | Sólo esos tres roots y análisis de imports; no lint completo de todos los QML. |

Main informó QSystemTrayIcon sin icono, señal systemTrayWindowChanged inexistente en offscreen, token len=0, cache vacía y polling desactivado (`HOME no disponible`). No son resultados de tray real; la ausencia de qml.auth generado es intencional en el smoke. Lint incluye refs no calificadas y tipos de parent.checked/text no conocidos. Exit 0 no significa ausencia de warnings.

Se inspeccionaron `src/bin/valida_qml.rs`, valida_agent y valida_agent_ui. Los dos primeros dejan proceso vivo 3 s como criterio y no drenan stderr; la prueba directa usa -apptype widget. El harness directo coincide con invocación de valida_agent_ui (`src/bin/valida_agent_ui.rs:8`); no se ejecutaron esos wrappers. **snap_ui NO EJECUTADO**: contiene pkill global de qml6/Xvfb y eliminación del lock X99 (`src/bin/snap_ui.rs:25`). No se capturaron screenshots ni se auditó visualmente todo el producto.

## Reproducciones aisladas

Se creó un crate diagnóstico **fuera del repo** que enlaza su biblioteca pública. No se añadieron tests al código de producción. Sin providers reales: mock HTTP en 127.0.0.1 con puerto efímero y claves constantes ficticias. Los fixtures estuvieron bajo un directorio mktemp 0700 y sólo se escribieron allí.

### Probe biblioteca: seguridad y SSE

`cargo run --manifest-path /tmp/kde-recovery-audit-0hvcyyku/probe/Cargo.toml --target-dir /home/eirom/KDE-ASSISTANT-2/target` (antes de añadir segundo binario). Exit 0, 70,134 s incluyendo build. XDG_CONFIG_HOME y XDG_CACHE_HOME propios del probe. Resultado:

```text
LOCK_FALLBACK disabled_without_lock=true success_with_write_lock=true
DANGLING_SYMLINK success=true outside_created=true
COPY_CHILD_SYMLINK success=true outside_overwritten=true
BACKUP_SYMLINK direct_auto_red_success=true outside_overwritten=true
GLOBAL_TOOL_FLAG disabled_but_executor_success=true
AUTH_PARSE evil_host_accepted=true evil_origin_accepted=true
EMPTY_EXPECTED_TOKEN empty_bearer_accepted=true
TOKEN_FILE token_mode=600 generated_mode=644
INVALID_CONFIG returns_ok=true default_tools_enabled=true wake_enabled=true
UTF8_SPLIT expected=Español actual=Espa��ol correct=false outcome_correct=false
```

Interpretación y condiciones: SEC-01/02/04/05/06/07 y REL-03 en REPORT. No se llamó al HTTP rojo para obtener el resultado de backup: fue executor directo. La prueba de token vacío es unitaria del validador; fallo real de creación de token no inducido. La prueba global confirma el comportamiento del executor, no que desactivar el catálogo necesariamente signifique desactivar adjuntos manuales. system_info leyó información del host sin imprimirla. Body ñ dividido entre dos escrituras con pausa de 150 ms; se observó corrupción tanto en tokens como outcome.

### Router real, SQLite real, proveedor y Piper ficticios

Se construyó AppState con AiService, ToolExecutor, SessionManager, ApprovalManager, SpeechService, VoicePipeline y router reales, config/DB/cache temporales. No se llamó Backend::new, para evitar descargas. Stub Piper sólo satisface búsqueda de ejecutable; termina con 42 si se invoca, no sintetiza. Chimes deshabilitados. No se inicia captura ni wake, ni se carga Whisper.

`cargo run --manifest-path /tmp/kde-recovery-audit-0hvcyyku/probe/Cargo.toml --bin http_probe --target-dir /home/eirom/KDE-ASSISTANT-2/target`: exit **0**, 9,619 s. Salida:

```text
HTTP_AUTH path=/api/health valid_key=false evil_origin=false status=200
HTTP_AUTH path=/api/sessions valid_key=false evil_origin=false status=401
HTTP_AUTH path=/api/sessions valid_key=true evil_origin=false status=200
HTTP_AUTH path=/api/sessions valid_key=true evil_origin=true status=403
HTTP_STREAM first_chunk_ms=42 has_token=true provider_finished=false
HTTP_CHAT final_done=true persisted_messages=2
HTTP_CONFIG status=200 roundtrip_width=600 returns_plaintext_dummy_key=true
HTTP_MANUAL global_tool_calling_disabled=true confirm_sensitive=true yellow_status=200 success=true
HTTP_MANUAL red_status=403
HTTP_CANCEL status=200 body={"status":"cancelled"}
HTTP_CANCEL provider_still_active_300ms=true
HTTP_CANCEL provider_completed_after_cancel=true persisted_messages=3
HTTP_SESSION_DELETE status=200 exists=false
```

Provider mock envía primer token, espera 2 s y después final. 42 ms es una observación de este fixture con build caliente; no mide Internet/LLM. Las banderas del mock significan que la tarea/operación de respuesta siguió viva y completó su intervalo; no miden facturación remota. El código explica el hijo no abortado. Se dejó tiempo para que acabara la tarea fixture antes de salir. Sesión eliminada fue creada por el probe, no una sesión real. La persistencia feliz no valida durabilidad tras crash/fallo de disco.

## Matriz funcional

PASS_FIXTURE/PASS_SMOKE no significan verificación end-to-end del producto.

| Función | Estado | Evidencia/límite |
| --- | --- | --- |
| Inicio aplicación completa | BLOCKED/NOT_TESTED | Piper ausente y arranque hace descargas/limpieza de procesos; constructor y flujo inspeccionados, no cargo run main. |
| Cierre completo y huérfanos | NOT_TESTED | Sólo procesos fixture propios terminados; no cierre de producto/audio/subprocesos reales. |
| Carga interfaz | PASS_SMOKE | Main/DevPreview offscreen vivos, warnings; no navegación real completa. |
| QML ↔ Rust | PARTIAL | Harness QML independiente y router Rust real probado por reqwest; no se conectó UI real al backend. |
| Proveedores/modelos externos | NOT_TESTED | Sin uso de keys/cuentas/red LLM; custom simulado funciona, listing externo no probado. |
| Chat/streaming | PASS_FIXTURE + FAIL Unicode | Tokens tempranos/Done/historial verificados; split UTF-8 corrupto. |
| Cancelación | FAIL | Retorno cancelled, llamada hija sigue activa; usuario persistido. |
| Sesiones | PASS_FIXTURE/PARTIAL | Crear/listar/borrar y count; rename, concurrencia, reinicio/crash no probados dinámicamente. |
| Config persistente | PASS_FIXTURE + FAIL cerrado | Width 600 se recarga; JSON inválido retorna defaults permisivos. Fallo de disco no inducido. |
| Herramientas | FAIL seguridad/PARTIAL | Fixtures create/copy/edit/read y system_info; no apps/KDE/archivos reales. |
| Confirmación/autorización | PARTIAL/FAIL límite | Endpoint rojo 403; amarilla auto; tests puros de ApprovalManager/policy incluidos en los 122 aprobados. No clic de aprobación QML real. |
| KDE/Wayland | NOT_TESTED | Versiones y source; sin tray/shortcut/overlay end-to-end, DBusMenu no consultado en app activa. |
| Captura/dispositivos audio | NOT_TESTED | No apertura de micrófono; faltantes de audio no se deducen de esa ausencia. |
| STT local/remoto | NOT_TESTED | No transcripción, modelos existentes no cargados en flujo auditado. |
| TTS/reproducción | BLOCKED | Piper real ausente; stub no prueba sonido. |
| VAD | PASS_UNIT/PARTIAL | Tests puros; no corpus ni captura real/latencia. |
| Wake ML | NOT_TESTED | Simulación ignored; fallback inspeccionado, no inferencia real auditada. |
| Avatar/expresiones | PASS_HARNESS/PARTIAL | Assertions simuladas; voz/overlay/sincronización nativa pendientes. |
| Python sidecar | BLOCKED | AST PASS; imports dependencias ausentes, sin lanzamiento ni servidor real. |

## Rendimiento observado y pendientes

Método: procesos qml6 offscreen independientes, sin backend, configuración y caché XDG temporales; 1,5 s de estabilización; 7 muestras /proc/PID/stat y status a intervalos de 1 s. CPU = delta de ticks de usuario+sistema / CLK_TCK / tiempo de pared × 100, equivalente a **un core**, no porcentaje del equipo completo. RSS de proceso, sin sumar compositor/otros servicios. Ventanas de 6,003 s; finalizar sólo PID propio con SIGTERM.

| Proceso | CPU un core | RSS primera/última | Máximo RSS muestreado |
| --- | --- | --- | --- |
| Main.qml offscreen | 1,499% | 102,14 / 102,21 MiB | 102,21 MiB |
| DevPreview.qml offscreen | 2,832% | 62,88 / 62,92 MiB | 62,92 MiB |

No representan app completa en reposo. No se suman entre sí: fueron pruebas independientes. Máximo muestreado no es pico absoluto. No se midieron arranque frío, carga de Whisper/ONNX, latencia STT/TTS, consumo con wake, fuga tras uso prolongado ni cierre del producto. Hipótesis derivadas de I/O síncrono, prewarm y subprocess por oración se etiquetan STATIC en PERF-01, no se convierten en números.

## Registro local y repetición

Evidencia de esta sesión en `/tmp/kde-recovery-audit-0hvcyyku/` (temporal, puede desaparecer): `validations.json`, `head-check.json`, `integration-result.json`, `qml-results.json`, `perf-results.json`, `probe-result.json`; logs `fmt.log`, `check.log`, `test.log`, `clippy.log`, `head-check.log`, `integration.log`, `qml-0.log` a `qml-3.log`, `probe.log`, `http-probe.log`, `perf-Main.log`, `perf-DevPreview.log`. Resultados relevantes quedan transcritos arriba; no depender del directorio temporal como único informe.

Los anexos conservan el código diagnóstico para repetir sin editar producción. Crear **un directorio mktemp nuevo**; sustituir todas las apariciones de `/tmp/kde-recovery-audit-0hvcyyku` en ambos sources por ese directorio, y el path del repo en Cargo.toml por el checkout auditado. No reutilizar fixtures: symlinks/DB previos alteran el resultado. Separar XDG_CONFIG_HOME/XDG_DATA_HOME/XDG_CACHE_HOME para cada probe; no modificar HOME ni usar config/DB reales.

```bash
# AUDIT_WORKDIR debe apuntar a un mktemp propio, nunca al HOME real.
export CARGO_NET_OFFLINE=true
unset OPENROUTER_API_KEY GROQ_API_KEY OPENAI_API_KEY
export XDG_CONFIG_HOME="$AUDIT_WORKDIR/probe-config"
export XDG_DATA_HOME="$AUDIT_WORKDIR/probe-data"
export XDG_CACHE_HOME="$AUDIT_WORKDIR/probe-cache"
cargo run --manifest-path "$AUDIT_WORKDIR/probe/Cargo.toml" --bin recovery-audit-probe
# Para el HTTP probe: otros XDG_* y PATH con stub descrito debajo.
cargo run --manifest-path "$AUDIT_WORKDIR/probe/Cargo.toml" --bin http_probe
```

Para HTTP probe crear en `$AUDIT_WORKDIR/fake-bin/piper-tts` un archivo ejecutable de prueba y anteponer **sólo en ese proceso** su directorio a PATH:

```python
#!/usr/bin/python3
raise SystemExit(42)
```

No usar ese PATH al iniciar el producto ni reportar voz aprobada con ese stub. Config usa tools amarillas por default, fixture temporal y auth ficticia; nunca cambiar allowed_paths a rutas reales para esta reproducción.

## Integridad de la auditoría

Se compararon SHA-256 de todos los archivos versionados con un inventario tomado antes de la auditoría: **0 archivos versionados cambiaron respecto al inicio**. `git diff --check` pasó. El diff previo de hotkey_listener permanece; las únicas adiciones del trabajo son los cuatro Markdown de docs/recovery. No hay commit ni push.

## Anexos: fuentes de los probes ejecutados

Fuentes diagnósticas fuera del producto, con unwrap en fixtures. El contenido de Cargo y Rust que sigue se preserva para revisión y reproducción; no se propone integrarlo como implementación de Fase 1.

### Cargo.toml

```toml
[package]
name = "recovery-audit-probe"
version = "0.0.0"
edition = "2021"
[dependencies]
kde-assistant-lib = { package = "kde-assistant", path = "/home/eirom/KDE-ASSISTANT-2" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
axum = "0.8"
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
```

### src/main.rs

```rust
use kde_assistant_lib::{
    backend::{
        ai_service::AiService,
        approvals::{ApprovalManager, ApprovalPolicy},
        auth,
        tool_executor::{ToolCtx, ToolExecutor},
    },
    models::{Config, Message, StreamEvent, ToolCall},
};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, RwLock},
};
fn tc(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: "audit-fixture".into(),
        name: name.into(),
        arguments: serde_json::from_value(args).unwrap(),
    }
}
#[tokio::main]
async fn main() {
    let root = PathBuf::from("/tmp/kde-recovery-audit-0hvcyyku/fixtures");
    std::fs::create_dir_all(root.join("allowed/dest")).unwrap();
    std::fs::create_dir_all(root.join("outside")).unwrap();
    let mut cfg = Config::default();
    cfg.tools.system_info = false;
    cfg.tools.allowed_paths = vec![];
    let cfg = Arc::new(RwLock::new(cfg));
    let ex = ToolExecutor::new(cfg.clone());
    let call = tc("system_info", serde_json::json!({}));
    let a = ex.execute(&call, &ToolCtx::auto(None)).await.unwrap();
    let lock = cfg.write().await;
    let b = ex.execute(&call, &ToolCtx::auto(None)).await.unwrap();
    drop(lock);
    println!(
        "LOCK_FALLBACK disabled_without_lock={} success_with_write_lock={}",
        !a.success, b.success
    );
    let mut cfg = Config::default();
    cfg.tools.allowed_paths = vec![root.join("allowed").display().to_string()];
    let cfg = Arc::new(RwLock::new(cfg));
    let ex = ToolExecutor::new(cfg.clone());
    std::os::unix::fs::symlink(
        root.join("outside/new.txt"),
        root.join("allowed/dangling.txt"),
    )
    .unwrap();
    let r=ex.execute(&tc("create_file",serde_json::json!({"path":root.join("allowed/dangling.txt"),"content":"DANGLING_FIXTURE"})),&ToolCtx::auto(None)).await.unwrap();
    println!(
        "DANGLING_SYMLINK success={} outside_created={}",
        r.success,
        root.join("outside/new.txt").exists()
    );
    std::fs::write(root.join("allowed/src.txt"), "COPY_FIXTURE").unwrap();
    std::fs::write(root.join("outside/victim.txt"), "ORIGINAL_FIXTURE").unwrap();
    std::os::unix::fs::symlink(
        root.join("outside/victim.txt"),
        root.join("allowed/dest/src.txt"),
    )
    .unwrap();
    let r=ex.execute(&tc("copy_file",serde_json::json!({"source":root.join("allowed/src.txt"),"destination":root.join("allowed/dest"),"overwrite":true})),&ToolCtx::auto(None)).await.unwrap();
    println!(
        "COPY_CHILD_SYMLINK success={} outside_overwritten={}",
        r.success,
        std::fs::read_to_string(root.join("outside/victim.txt")).unwrap() == "COPY_FIXTURE"
    );
    std::fs::write(root.join("allowed/edit.txt"), "BACKUP_FIXTURE").unwrap();
    std::fs::write(root.join("outside/backup-victim.txt"), "ORIGINAL_FIXTURE").unwrap();
    std::os::unix::fs::symlink(
        root.join("outside/backup-victim.txt"),
        root.join("allowed/edit.bak"),
    )
    .unwrap();
    let r=ex.execute(&tc("edit_file",serde_json::json!({"path":root.join("allowed/edit.txt"),"mode":"overwrite","content":"UPDATED_FIXTURE"})),&ToolCtx::auto(None)).await.unwrap();
    println!(
        "BACKUP_SYMLINK direct_auto_red_success={} outside_overwritten={}",
        r.success,
        std::fs::read_to_string(root.join("outside/backup-victim.txt")).unwrap()
            == "BACKUP_FIXTURE"
    );
    cfg.write().await.ai.enable_tool_calling = false;
    let r = ex
        .execute(
            &tc(
                "read_file",
                serde_json::json!({"path":root.join("allowed/edit.txt")}),
            ),
            &ToolCtx::auto(None),
        )
        .await
        .unwrap();
    println!(
        "GLOBAL_TOOL_FLAG disabled_but_executor_success={}",
        r.success
    );
    let mut h = axum::http::HeaderMap::new();
    h.insert("host", "localhost.attacker.invalid:8765".parse().unwrap());
    h.insert(
        "origin",
        "https://localhost.attacker.invalid".parse().unwrap(),
    );
    println!(
        "AUTH_PARSE evil_host_accepted={} evil_origin_accepted={}",
        auth::valid_host(&h),
        auth::valid_origin(&h)
    );
    h.insert("authorization", "Bearer ".parse().unwrap());
    println!(
        "EMPTY_EXPECTED_TOKEN empty_bearer_accepted={}",
        auth::valid_bearer(&h, "")
    );
    let token = auth::ensure_server_token().unwrap();
    let inc = auth::write_qml_auth_module(&token, false).unwrap();
    use std::os::unix::fs::PermissionsExt;
    println!(
        "TOKEN_FILE token_mode={:o} generated_mode={:o}",
        std::fs::metadata(auth::server_token_path().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        std::fs::metadata(inc.join("qml/auth/AuthToken.qml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    );
    let path = Config::config_path().unwrap();
    std::fs::write(&path, "{ malformed fixture").unwrap();
    let loaded = Config::load().await.unwrap();
    println!(
        "INVALID_CONFIG returns_ok=true default_tools_enabled={} wake_enabled={}",
        loaded.tools.edit_file && loaded.tools.read_file,
        loaded.speech.wake_word_enabled
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut req = vec![0u8; 32768];
        let _ = s.read(&mut req).await;
        let data =
            "data: {\"choices\":[{\"delta\":{\"content\":\"Español\"}}]}\n\ndata: [DONE]\n\n"
                .as_bytes();
        let split = data.iter().position(|&b| b == 0xc3).unwrap() + 1;
        s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",data.len()).as_bytes()).await.unwrap();
        s.write_all(&data[..split]).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        s.write_all(&data[split..]).await.unwrap();
    });
    let mut c = Config::default();
    c.ai.provider = "custom".into();
    c.ai.base_url = format!("http://{addr}");
    c.ai.api_key = "DUMMY_AUDIT_FIXTURE".into();
    let c = Arc::new(RwLock::new(c));
    let ai = AiService::new(c.clone()).await.unwrap();
    let (tx, mut rx) = mpsc::channel(32);
    let task = tokio::spawn(async move {
        ai.run_agent(
            vec![Message::user("fixture")],
            vec![],
            Arc::new(ToolExecutor::new(c)),
            Arc::new(ApprovalManager::new()),
            ApprovalPolicy::from_config(true),
            None,
            tx,
        )
        .await
    });
    let mut text = String::new();
    while let Some(e) = rx.recv().await {
        if let StreamEvent::Token { content } = e {
            text.push_str(&content)
        }
    }
    let out = task.await.unwrap().unwrap();
    server.await.unwrap();
    println!(
        "UTF8_SPLIT expected=Español actual={} correct={} outcome_correct={}",
        text,
        text == "Español",
        out.response == "Español"
    );
    let _unused: HashMap<String, String> = HashMap::new();
}
```

### src/bin/http_probe.rs

```rust
use kde_assistant_lib::{
    backend::{
        ai_service::AiService,
        approvals::ApprovalManager,
        chime_player::ChimePlayer,
        http_server::{router, AppState},
        session_manager::SessionManager,
        speech_service::SpeechService,
        tool_executor::ToolExecutor,
        voice_pipeline::VoicePipeline,
    },
    models::Config,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{broadcast, RwLock},
};
#[tokio::main]
async fn main() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let llm_addr = listener.local_addr().unwrap();
    let alive = Arc::new(AtomicBool::new(false));
    let after = Arc::new(AtomicBool::new(false));
    let a = alive.clone();
    let z = after.clone();
    let provider = tokio::spawn(async move {
        loop {
            let (mut s, _) = listener.accept().await.unwrap();
            let a = a.clone();
            let z = z.clone();
            tokio::spawn(async move {
                let mut b = vec![0u8; 32768];
                let _ = s.read(&mut b).await;
                a.store(true, Ordering::SeqCst);
                let first = b"data: {\"choices\":[{\"delta\":{\"content\":\"Hola \"}}]}\n\n";
                let last=b"data: {\"choices\":[{\"delta\":{\"content\":\"fixture\"}}]}\n\ndata: [DONE]\n\n";
                let h=format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",first.len()+last.len());
                s.write_all(h.as_bytes()).await.unwrap();
                s.write_all(first).await.unwrap();
                tokio::time::sleep(Duration::from_secs(2)).await;
                let _ = s.write_all(last).await;
                z.store(true, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(100)).await;
                a.store(false, Ordering::SeqCst);
            });
        }
    });
    let mut c = Config::default();
    c.ai.provider = "custom".into();
    c.ai.base_url = format!("http://{llm_addr}");
    c.ai.api_key = "DUMMY_PROVIDER_FIXTURE".into();
    c.ai.enable_tool_calling = false;
    c.memory.auto_summarize = false;
    c.speech.wake_word_enabled = false;
    c.tools.allowed_paths = vec!["/tmp/kde-recovery-audit-0hvcyyku/http-fixtures".into()];
    std::fs::create_dir_all(&c.tools.allowed_paths[0]).unwrap();
    let cfg = Arc::new(RwLock::new(c));
    let ai = Arc::new(AiService::new(cfg.clone()).await.unwrap());
    let sessions = Arc::new(Mutex::new(SessionManager::new().await.unwrap()));
    let tools = Arc::new(ToolExecutor::with_audit(cfg.clone(), sessions.clone()));
    let approvals = Arc::new(ApprovalManager::new());
    let speech = Arc::new(SpeechService::new(cfg.clone()).await.unwrap());
    let chimes = Arc::new(ChimePlayer::new().await.unwrap());
    chimes.set_enabled(false);
    let voice = Arc::new(VoicePipeline::new(
        cfg.clone(),
        speech.clone(),
        ai.clone(),
        tools.clone(),
        approvals.clone(),
        sessions.clone(),
        chimes,
    ));
    let state = AppState {
        ai,
        tools,
        approvals,
        config: cfg,
        sessions: sessions.clone(),
        speech,
        voice: voice.clone(),
        agent_events: broadcast::channel(32).0,
        local_token: "DUMMY_LOCAL_FIXTURE".into(),
        chat_tasks: Arc::new(Mutex::new(HashMap::new())),
        dictation_active: voice.dictation_flag(),
        sidecar_active: Arc::new(AtomicBool::new(false)),
    };
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(l, router(state)).await.unwrap() });
    let cl = reqwest::Client::builder().no_proxy().build().unwrap();
    let url = format!("http://{addr}");
    for (path, key, origin) in [
        ("/api/health", false, None),
        ("/api/sessions", false, None),
        ("/api/sessions", true, None),
        ("/api/sessions", true, Some("https://evil.invalid")),
    ] {
        let mut r = cl.get(format!("{url}{path}"));
        if key {
            r = r.bearer_auth("DUMMY_LOCAL_FIXTURE")
        }
        if let Some(o) = origin {
            r = r.header("Origin", o)
        }
        let res = r.send().await.unwrap();
        println!(
            "HTTP_AUTH path={path} valid_key={key} evil_origin={} status={}",
            origin.is_some(),
            res.status().as_u16()
        );
    }
    let r = cl
        .post(format!("{url}/api/session"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&serde_json::json!({"title":"AUDIT_FIXTURE"}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let sid = body["id"].as_str().unwrap();
    let t = std::time::Instant::now();
    let mut r = cl
        .post(format!("{url}/api/chat"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&serde_json::json!({"message":"fixture","session_id":sid}))
        .send()
        .await
        .unwrap();
    let first = r.chunk().await.unwrap().unwrap();
    println!(
        "HTTP_STREAM first_chunk_ms={} has_token={} provider_finished={}",
        t.elapsed().as_millis(),
        String::from_utf8_lossy(&first).contains("event: token"),
        after.load(Ordering::SeqCst)
    );
    let rest = r.text().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!(
        "HTTP_CHAT final_done={} persisted_messages={}",
        rest.contains("event: done"),
        sessions.lock().unwrap().count_messages(sid).unwrap()
    );
    let mut config: serde_json::Value = cl
        .get(format!("{url}/api/config"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    config["ui"]["window_width"] = serde_json::json!(600);
    let res = cl
        .post(format!("{url}/api/config"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&config)
        .send()
        .await
        .unwrap();
    let loaded = Config::load().await.unwrap();
    println!(
        "HTTP_CONFIG status={} roundtrip_width={} returns_plaintext_dummy_key={}",
        res.status().as_u16(),
        loaded.ui.window_width,
        config["ai"]["api_key"] == "DUMMY_PROVIDER_FIXTURE"
    );
    let res=cl.post(format!("{url}/api/tools/execute")).bearer_auth("DUMMY_LOCAL_FIXTURE").json(&serde_json::json!({"name":"create_file","arguments":{"path":"/tmp/kde-recovery-audit-0hvcyyku/http-fixtures/manual.txt","content":"FIXTURE"}})).send().await.unwrap();
    let status = res.status().as_u16();
    let j: serde_json::Value = res.json().await.unwrap();
    println!("HTTP_MANUAL global_tool_calling_disabled=true confirm_sensitive=true yellow_status={status} success={}",j["success"]);
    let res = cl
        .post(format!("{url}/api/tools/execute"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&serde_json::json!({"name":"edit_file","arguments":{}}))
        .send()
        .await
        .unwrap();
    println!("HTTP_MANUAL red_status={}", res.status().as_u16());
    after.store(false, Ordering::SeqCst);
    let mut r = cl
        .post(format!("{url}/api/chat"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&serde_json::json!({"message":"cancel fixture","session_id":sid}))
        .send()
        .await
        .unwrap();
    let _ = r.chunk().await.unwrap();
    let res = cl
        .post(format!("{url}/api/chat/cancel"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .json(&serde_json::json!({"session_id":sid}))
        .send()
        .await
        .unwrap();
    println!(
        "HTTP_CANCEL status={} body={}",
        res.status().as_u16(),
        res.text().await.unwrap()
    );
    drop(r);
    tokio::time::sleep(Duration::from_millis(300)).await;
    println!(
        "HTTP_CANCEL provider_still_active_300ms={}",
        alive.load(Ordering::SeqCst)
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!(
        "HTTP_CANCEL provider_completed_after_cancel={} persisted_messages={}",
        after.load(Ordering::SeqCst),
        sessions.lock().unwrap().count_messages(sid).unwrap()
    );
    let res = cl
        .delete(format!("{url}/api/session?session_id={sid}"))
        .bearer_auth("DUMMY_LOCAL_FIXTURE")
        .send()
        .await
        .unwrap();
    println!(
        "HTTP_SESSION_DELETE status={} exists={}",
        res.status().as_u16(),
        sessions.lock().unwrap().get_session(sid).unwrap().is_some()
    );
    server.abort();
    provider.abort();
}
```
