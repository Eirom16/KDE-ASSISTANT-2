# F3 — Resultados y evidencia

Baseline `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`; rama
`recovery/f3-kde-wayland-integration`. Sin commits.

El runner crea XDG_CONFIG_HOME, XDG_DATA_HOME, XDG_CACHE_HOME y TMPDIR privados,
retira credenciales/proxies y no registra atajos en el escritorio. No usa voz,
modelos ni proveedor comercial. Los comandos y logs íntegros se conservan bajo
el directorio Evidence anunciado por cada corrida. Los conteos de runners
anidados no se suman como pruebas distintas.

## Reproducción

En este entorno faltaba CMake. Se obtuvo una wheel sólo en `/var/tmp/kda-f3-tools`:

```bash
uv pip install --target /var/tmp/kda-f3-tools cmake
env CMAKE=/var/tmp/kda-f3-tools/cmake/data/bin/cmake python3 scripts/validate_f3.py
env CMAKE=/var/tmp/kda-f3-tools/cmake/data/bin/cmake python3 scripts/validate_f3.py --specific-only --plasma
```

Con CMake en PATH no hace falta `CMAKE=...`. `--specific-only` evita repetir
regresiones ya ejecutadas. El runner completo ejecuta exactamente:

```bash
python3 scripts/validate_f2.py
python3 scripts/validate_f1b3b.py --qml
python3 scripts/validate_gate_w1r.py
```

`qml_test_harness.py` compila un ejecutable temporal con QApplication y
Qt6Widgets/Qt6QuickTest mediante pkg-config. No cambia paquetes globales ni
deshabilita la bandeja productiva. Los runners F2/W1R usan ese harness; el
runner F3 lo comparte por KDA_QML_TEST_RUNNER para evitar recompilarlo.

## Registro de correcciones durante desarrollo

- Primera compilación sin CMake: FAIL de entorno; instalada herramienta local,
  sin root. Error inicial de JoinHandle por referencia inmutable corregido.
- Sonda de portal ausente: corridas interrumpidas al descubrir espera de caché
  de propiedades/autostart. Ahora no hay caché de propiedades en la sonda,
  plazo explícito y bus privado sintético sin service directories. Los tres
  helpers activados en buses privados anteriores se identificaron por su
  DBUS_STARTER_ADDRESS/config y se terminaron mediante pidfd; no se tocó el
  portal de la sesión personal.
- Mock de portal usaba propiedad `Version` en lugar de `version`: FAIL de
  fixture corregido, no se relajó la comprobación productiva.
- Rebind de socket de prueba encontró TIME_WAIT: listener con SO_REUSEADDR,
  sin SO_REUSEPORT; la prueba comprueba liberación con la misma semántica.
- Una corrida no capturaba logs Qt porque KDE los enviaba al journal. Se fuerza
  stderr sólo en el entorno de pruebas y se exige diagnóstico mientras SQLite
  todavía indica pending. No se presenta una muestra posterior como evidencia.
- La primera prueba exploratoria del harness de cuatro casos F2 se ejecutó
  antes de envolver XDG; no se usa como evidencia de aislamiento. Las corridas
  formales posteriores usan los cuatro directorios privados.

## Clasificación

UNIT, AUTOMATED_QML y AUTOMATED_KWIN son categorías distintas. HUMAN_VERIFIED
sólo admite una observación atribuida explícitamente a una persona. Offscreen
no certifica Wayland. IGNORED/FILTERED se registran según salida de cada selección.
Una corrida de desarrollo parcial no equivale a aceptación F3.


## Resultado final automatizado

Entorno real: Pentium Silver N5000, Intel UHD 605, Plasma 6.8.0, Wayland,
Qt 6.12.0, KF6WindowSystem 6.31.0, kernel 7.2.7-1-cachyos, Rust 1.99.0.
Una pantalla 1366×768; pruebas sin credenciales comerciales ni modelos/audio.

| Selección | PASS | FAIL | IGNORED / FILTERED | Categoría / evidencia |
|---|---:|---:|---|---|
| F3 desktop Rust | 13 | 0 | 0 / 0 | UNIT; /tmp/kda-f3-final-m25yawoq/f3-unit.log |
| Recursos | 3 | 0 | 0 / 0 | UNIT; mismo log, binario app_paths |
| Tema / respuesta DBus anidada | 3 | 0 | 0 / 154 | UNIT; theme-unit.log |
| F3 QML final | 12 | 0 | 0 skipped | AUTOMATED_QML; /tmp/kda-f3-auth-qml-final-qx427p3_/f3_desktop.log |
| W1R QML final | 12 | 0 | 0 skipped | AUTOMATED_QML; mismo directorio, gate_w1r.log |
| F2 consentimiento QML final | 4 | 0 | 0 skipped | AUTOMATED_QML; mismo directorio, f2_consent.log |
| F1B1 arranque repetido tras cambios finales | 13 | 0 | 1 / 0 | UNIT; /tmp/kda-f3-final-m25yawoq/f1b1-startup.log |
| Procesos/ventanas nativos | 13 grupos | 0 | sin filtros | AUTOMATED_KWIN/QML y procesos; /tmp/kda-f3-validation-5sjtuawb/runtime/results.json |
| fmt / Clippy default / Clippy legacy-fixtures / build default | PASS | 0 | n/a | /tmp/kda-f3-final-m25yawoq/results.json |

Los 12 casos QtTest F3 incluyen init/cleanup y diez funciones de prueba; no
son doce escenarios humanos. La corrida nativa contiene escenarios agrupados,
no un equivalente de trece unitarias. Sus trece grupos incluyen el launcher
privado y la revocación/reinicio con proveedor simulado. El harness conserva
category para cada resultado; los ensayos de procesos no certifican el compositor.

Comandos finales adicionales (entorno privado creado por el harness):

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings
cargo test --locked --test f3_desktop --test app_paths -- --test-threads=1
cargo test --locked --test f1b1_text_startup -- --test-threads=1
cargo test --locked --lib backend::kde_integration::tests -- --test-threads=1
cargo build --locked --bin kde-assistant
env CMAKE=/var/tmp/kda-f3-tools/cmake/data/bin/cmake python3 scripts/validate_f3.py --runtime-only --plasma
```

Cada suite QML final se invocó así, sustituyendo SUITE por f3_desktop,
gate_w1r y f2_consent:

```bash
/tmp/kda-f3-final-m25yawoq/qmltest-qapplication -import /home/eirom/KDE-ASSISTANT-2 -input /home/eirom/KDE-ASSISTANT-2/tests/qml/tst_SUITE.qml -o -,txt
```

Las listas exactas de argv están en results.json. Las corridas Rust usan
CMAKE privado y las cuatro variables XDG/TMPDIR privadas. Tras la última
modificación Rust se repitieron Clippy en ambos modos, unitarias específicas,
arranque y build; tras el ajuste final del remapeo QML se repitieron las tres
suites QML y la corrida nativa. No se presenta un gate anterior como si hubiese
sido ejecutado nuevamente sobre cambios posteriores.

## Regresiones obligatorias

Los tres comandos obligatorios PASS en `/tmp/kda-f3-validation-piho0cu3`.
F2 y F1B3B escriben evidencias anidadas; el padre registra el resultado del
comando sin duplicar artificialmente sus conteos.

Selección heredada F1B3B, en
`/tmp/kda-f3-validation-piho0cu3/tmp/kda-f1b3b-check-z4i8qkve`:

| Suite | PASS | FAIL | IGNORED | FILTERED |
|---|---:|---:|---:|---:|
| F1A | 19 | 0 | 1 | 0 |
| F1B1 | 13 | 0 | 1 | 0 |
| F1B2 | 20 | 0 | 3 | 0 |
| F1B3A | 26 | 0 | 1 | 0 |
| F1B3B | 33 | 0 | 3 | 0 |
| lib-safe | 154 | 0 | 1 | 2 |
| app_paths | 3 | 0 | 0 | 0 |
| Qt opt-in F1B1 | 1 | 0 | 0 | 13 |
| Qt opt-in F1B2 | 3 | 0 | 0 | 20 |
| Qt opt-in F1B3B | 2 | 0 | 0 | 34 |

F2: 17 PASS, cero FAIL/IGNORED; HTTP: 1 PASS/156 FILTERED;
QML consentimiento: 4 PASS. W1R tenía 11 PASS en la corrida completa; el
nuevo caso pin Wayland deshabilitado eleva su repetición final a 12 PASS.
Los IGNORED corresponden a selecciones opt-in existentes; se muestran además
las ejecuciones Qt expresamente habilitadas. No se eliminó ni ocultó un fallo
reduciendo la suite.

## Plasma Wayland observado

PASS: backend/QML desde otro CWD, perfiles independientes, duplicado que activa
sin crear hijo, registro SNI por PID exacto, DBusMenu real, activa→oculta,
oculta→muestra, minimizada→restaurada con una Activate, maximizado conservado,
ventana propia desenfocada→intento de foco, cerrar→tray→restaurar y launcher
privado `gio launch`. No se instaló un launcher en el perfil personal.

PASS: cinco pares de apertura/cierre con precondiciones sincronizadas; fallo
del hijo cierra backend, libera puerto y recoge Qt; muerte del padre mata Qt;
reinicio explícito sin lock obsoleto; recursos ausentes fallan con diagnóstico.
Nueva conversación desde DBusMenu revoca el primer consentimiento; una segunda
solicitud sintética queda revocada al matar QML. Reinicio sin replay, dos llamadas
al proveedor local, ningún contenido del fixture enviado y SQLite integrity/FK OK.
Salir desde DBusMenu termina backend e hijo.

El foco sintético fue activo en la corrida final. Durante desarrollo hubo
corridas con actividad todavía no propagada a Qt y una llamada sin token que
no recuperó foco. Se corrigió el remapeo también para maximizado y el harness
espera la propagación de precondiciones; no presupone paridad de toggles a ciegas.
Si KWin rechaza el foco sintético, el runner registra BLOCKED para ese escenario
y continúa las pruebas independientes; no fuerza el resultado del intento.
La política de foco mediante gesto humano sigue NOT_TESTED.

Consentimiento observado **mientras seguía pending**, antes y después de tomar
la muestra: pid 79261, `at_ms=1792260665715`, active=true, minimized=false,
stacking_index=4, geometría 575×708 en (395,10). QML registró popup visible y
con foco a `1792260665686`, 14 ms desde presentación, z=300.
`consent-kwin-pending.json` y app.log conservan los datos. No prueba la causa
del episodio Red W1H-05 ni garantiza foco en toda sesión futura.

Los scripts temporales KWin sólo operan sobre PID/caption propios y se
comprueba unload; el helper desenfocado también pertenece al harness. No hay
control de otras aplicaciones, inyección global de teclado/cursor ni libei.

## Rendimiento — muestras, sin presupuesto universal

Corrida `/tmp/kda-f3-validation-5sjtuawb/runtime/performance.json` sin compiladores
simultáneos. CPU es porcentaje de un núcleo; memoria en KiB. Sólo una muestra
por condición; no es un benchmark comparativo contra baseline.

| Condición | Intervalo | CPU backend / Qt | RSS backend / Qt | PSS backend / Qt |
|---|---|---|---|---|
| Visible reposo | 3.016 s | 0.00% / 12.27% | 29984 / 188992 | 26517 / 150630 |
| Oculta reposo | 3.018 s | 0.00% / 1.99% | 29984 / 189004 | 26517 / 150642 |
| Cinco pares abrir/cerrar | 5.793 s | 0.00% / 11.22% | 29984 / 189008 | 26517 / 150736 |

Salud HTTP: 0.076 s; canal UI acepta activación: 1.210 s. Esto no mide el
primer frame interactivo. Los ciclos incluyen esperas y observadores KWin;
los porcentajes sólo contabilizan procesos backend/Qt, no todo el escritorio.
Estado normal: un backend y un hijo Qt. El helper existe sólo durante la
prueba de foco; comandos de inspección y launcher son transitorios. Tray se
mide incluido en Qt, sin atribución separada. Coste del atajo real NOT_TESTED.
No se introdujo polling de teclado/recents; permanece el sondeo de hijos
heredado (200 ms) y espera headless (100 ms). El reposo visible QML aún consume
CPU apreciable; su análisis profundo sigue en F5. No se certifica ausencia de
regresión cuantitativa sin una comparación controlada equivalente.

## Aceptación pendiente

- HUMAN_VERIFIED: NOT_TESTED en F3; no se reutiliza la aceptación humana F2
  para certificar ventanas/atajos nuevos.
- GlobalShortcuts real: NOT_TESTED por elección explícita del usuario. El
  portal v2 está disponible; mock cubre creación/listado/binding, denegación,
  respuestas adelantadas/excesivas, filtros, release sin PTT y cierre.
- HiDPI físico, varias pantallas, escala en vivo, X11, overlay heredado y
  empaquetado instalado: NOT_TESTED; F5/F6 fuera del alcance.
- Blur y pin superior Wayland de esta implementación: NOT_SUPPORTED. No se
  pretende que Qt translucidez/hint equivalgan a soporte efectivo de KWin.
- No hubo notificaciones de prueba reales, modificación de volumen/brillo,
  configuración KDE personal, root ni acciones sobre aplicaciones ajenas.

Estado final PARTIAL. El código queda READY_FOR_REVIEW, con aceptación humana
pendiente y sin commits; no se declara F3 completa.


### Repetición de cierre

`validate_f3.py --specific-only --plasma` PASS en
`/tmp/kda-f3-validation-r60fqfpx`: fmt, 13 unitarias F3, tres de recursos,
tres de tema, 12 QtTest F3, build productivo y los trece grupos de procesos/KWin.
El runner ahora incluye explícitamente las unitarias de tema aun al omitir
regresiones. La selección completa anterior conserva los tres gates heredados.

Se retiró además el fallback inoperante de lectura de entorno en Main, tray,
ajustes y auditoría: el bearer sólo viene de AuthToken.token. La repetición
QML final (12/12/4 PASS) está en
`/tmp/kda-f3-auth-qml-final-qx427p3_`. No se pasa bearer por env al hijo ni
se registra su longitud. Los textos de error apuntan al módulo privado.


La repetición nativa posterior a esa retirada también PASS en
`/tmp/kda-f3-validation-n0xkz35n`: trece grupos, incluidos launcher, foco
sintético, consentimiento todavía pending, nueva sesión, revocación por pérdida
de QML, recuperación y salida de bandeja. Los logs/SQLite privados se conservan
como evidencia. Revisión final: 41 históricos SHA-256 sin cambios, índice vacío,
HEAD obligatorio conservado, un worktree y cero backend/Qt de pruebas vivos.
`git diff --check` PASS. Sin commits/push/merge/release.


## Auditoría final — reproducciones y alcance

Evidencia raíz: `/var/tmp/kda-f3-audit-577vs_tr`. Los conteos anteriores son
corridas de implementación, no se reinterpretan como ejecuciones de esta auditoría.
El entorno reutiliza CMake privado, XDG/TMPDIR 0700 por comando, sin credenciales,
portal sintético sin autostart y proveedor loopback. No se registra un atajo real.

| Ensayo previo a corrección | Resultado observado | Evidencia |
|---|---|---|
| Cuatro casos portal (early activation/close, handle, bound IDs) | FAIL 4/4; no se falseó PASS | reproduce-portal-private.log |
| Drop del owner | FAIL: seguía entregando toggles | reproduce-owner-drop.log |
| Pérdida del owner DBus del portal | FAIL: estado seguía registrado | reproduce-portal-service-loss.log |
| Gate completo inicial | FAIL en reinicio F1B2; demás selecciones F2 pasaron | gate-f3-initial-fail.log |
| F1B2 reinicio focalizado | FAIL reproducido, database_backend_already_running | reproduce-f1b2-restart.log |
| Experimento Arc/Drop/flock | Demuestra Weak vacío mientras Drop aún conserva flock | arc-release-proof.log y arc_release.rs |
| F1B2 tras corregir sólo fixture | PASS 1, 22 FILTERED | fixed-f1b2-restart.log |

La regresión F1B2 mantiene cancelación, conexión cerrada, contenido e integridad
persistida. Sólo espera el recurso real; no se elimina la exclusión, no se
reintenta Backend::new ni una operación con efectos. El primer intento de las
cuatro pruebas privadas también encontró sun_path demasiado largo; HAR-01
corrige únicamente el transporte del harness a socket abstracto aleatorio.
Un primer borrador de la prueba HTTP no importaba StreamExt y no compiló;
se corrigió el test, sin cambio al middleware productivo.

Después de corregir: portal-final.log registra 21 PASS F3 y 1 PASS de proceso/hilo;
desktop-http-fixed.log registra 1 PASS / 157 FILTERED. Los gates finales y los
árboles staged se documentan a continuación y en F3-COMMIT-VALIDATION.md; no se
suman repeticiones/selecciones anidadas como pruebas independientes.


### Gates finales sobre el workspace auditado

Todos los siguientes comandos terminaron con exit 0. Logs/argv/tiempos en
la raíz de auditoría, un JSON y un log por nombre. CMake privado y el aislamiento
XDG descrito arriba se aplicaron en cada comando.

| Evidencia | Comando exacto | Resultado |
|---|---|---|
| gate-f3.log | `python3 scripts/validate_f3.py` | PASS |
| gate-plasma.log | `python3 scripts/validate_f3.py --specific-only --plasma` | PASS |
| gate-f2.log | `python3 scripts/validate_f2.py` | PASS |
| gate-f1b3b.log | `python3 scripts/validate_f1b3b.py --qml` | PASS |
| gate-w1r.log | `python3 scripts/validate_gate_w1r.py` | PASS |
| fmt.log | `cargo fmt --check` | PASS |
| clippy.log | `cargo clippy --locked --all-targets -- -D warnings` | PASS |
| clippy-fixtures.log | `cargo clippy --locked --features legacy-fixtures --all-targets -- -D warnings` | PASS |
| production-build.log | `cargo build --locked --bin kde-assistant` | PASS |


Selección F3: 21 PASS de escritorio/portal, 1 PASS parent-death por hilo,
1 PASS HTTP/SSE (157 FILTERED), 3 PASS recursos, 3 PASS tema (155 FILTERED),
12 PASS QtTest F3 (incluye hooks). F2 mantiene 17 PASS; F1A 19/1 IGNORED,
F1B1 13/1 IGNORED, F1B2 20/3 IGNORED, F1B3A 26/1 IGNORED y F1B3B
33/3 IGNORED, sin FAIL. Las selecciones Qt opt-in de F1B1/F1B2/F1B3B también
pasan en validate_f1b3b.py --qml. No se suman selecciones anidadas/repetidas.
F2 QML: 4 PASS; W1R: 12 PASS.

La corrida Plasma de auditoría conserva 14 grupos de runtime, todos PASS,
en `/var/tmp/kda-f3-audit-577vs_tr/profiles/gate-plasma/tmp/kda-f3-validation-9xlqgoyk`.
Incluye /proc para verificar spawning_tid == backend_pid, SNI, maximizado,
restauración de minimizada, foco sintético, launcher, consentimiento pendiente,
revocación por nueva sesión/pérdida de QML y salida. No es HUMAN_VERIFIED.
Sus muestras se conservan en runtime/performance.json; no se usan como prueba
cuantitativa de regresión frente a las muestras únicas originales.

### Árbol staged A y autorización de consolidación

Commit A local `e4bb565e6961c1262a93121ab88057713157c6ac`, árbol
`7c1b5d9db9f61e98f0508f109595f10a779d9229`: 29 archivos explícitos.
Antes del commit se creó git archive del árbol staged, se verificaron 255 blobs,
permisos y ausencia de extras, y se ejecutaron build, validate_f2.py,
validate_f1b3b.py --qml y validate_gate_w1r.py desde el archivo extraído:
todos PASS. W1R A tiene 11 PASS; el caso nuevo del pin pertenece a B.
Se empleó un QApplication runner externo auditado como shim de qmltestrunner
para los scripts antiguos de A, sin cambiar sus fuentes ni la bandeja productiva.

El primer intento de commit falló por identidad Git no configurada; no creó
commit. Se reutilizó únicamente por comando la identidad Codex del baseline,
sin escribir configuración global/local. El tree del commit A coincide con
el staged probado y su padre es el baseline obligatorio.

El árbol B y el parche baseline→HEAD se verifican antes de emitir el dictamen
final en F3-COMMIT-VALIDATION.md y F3-CLOSURE.md, creados después de ambos commits.
La aceptación de escritorio sigue PARTIAL y la tecla real NOT_TESTED; esa
limitación, expresamente aceptada por dirección para consolidar el código,
no se convierte en certificación humana ni en autorización para abrir el portal.
