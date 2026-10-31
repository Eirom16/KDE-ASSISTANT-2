# F4-R3 — Piper real: síntesis local y validación aislada

Fecha: 2026-10-19. **PARTIAL / LIVE_TTS NOT_TESTED / NEEDS_FIXES** mientras falta aceptación audible; los gates automatizados constan en RESULTS. Sin commits/push/merge/release/F5/wake ni nuevas capturas físicas.

## Baseline y autorización

Rama `recovery/f4-voice-coordinator`, HEAD `9544a58eba4b6adf6c6343c04db80813b084f76b`, índice vacío. Se preservan los 13 informes/contrato F4, F4-R y F4-R2: [hashes iniciales](F4-R3-EVIDENCE/baseline.json). No se inspeccionó ni cambió configuración, conversaciones, credenciales o metadatos del perfil personal. F4-INC-01 continúa vigente.

El usuario respondió **«Lo autorizo»** a la propuesta completa de 144,84 MiB: [recibo y hash del inventario aprobado](F4-R3-EVIDENCE/download-authorization.json). Autoriza descarga, instalación privada y síntesis offline, sin altavoces ni micrófono. La autorización audible T1 será independiente; no se avanza automáticamente a T2–T5.

## Dependencias y procedencia

Instalación temporal exclusiva: `/var/tmp/kda-f4r3-piper-aqrh6h0h`, raíz 0700 y HOME/TMP/cuatro XDG privados con las guardas F4-R2. Venv Python 3.14.8, pip 26.2.1 ya empaquetado por ensurepip; ningún pip del sistema ni paquete global. Instalación `--no-index --require-hashes --no-compile` desde archivos verificados. [URLs, tamaños, SHA y grafo exacto](F4-R3-EVIDENCE/download-inventory.json), [instalación](F4-R3-EVIDENCE/installation.json), [importaciones reales](F4-R3-EVIDENCE/import-check.json).

| Componente | Versión | Bytes descargados | Licencia declarada |
|---|---|---:|---|
| piper-tts | 1.8.0 | 34.131.442 | GPL-3.0-or-later |
| onnxruntime | 1.31.0 | 23.762.993 | MIT |
| pathvalidate | 3.3.1 | 24.305 | MIT |
| flatbuffers | 25.12.19 | 26.661 | Apache-2.0 |
| numpy | 2.5.4 | 16.721.710 | BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0 según metadatos; conservar avisos de bibliotecas nativas incluidas |
| packaging | 26.3 | 129.956 | Apache-2.0 OR BSD-2-Clause |
| protobuf | 7.36.2 | 343.223 | BSD-3-Clause |
| es_ES-sharvard-medium + JSON + ficha | revisión `c10ece1aade47bb51c153c893d14e5bf8e5b7117` | 76.738.910 | Repo MIT; dataset CC BY 3.0 según ficha; condiciones de pesos/derivación y atribuciones específicas pendientes de revisión de distribución |

Total **151.879.200 B / 144,843 MiB**. El inventario de propuesta permanece inmutable para validar el hash del recibo de autorización; su campo histórico approved:false no reemplaza el permiso posterior registrado. Siete wheels de [PyPI oficial](https://pypi.org/project/piper-tts/1.8.0/) y tres archivos del [repositorio oficial de voces](https://huggingface.co/rhasspy/piper-voices). ONNX comprobado por SHA-256 LFS; JSON/ficha por hashes Git de la revisión fijada. Configuración 4903 B, distinta del inventario histórico R2; no se mezclaron revisiones.

La [ficha específica](https://huggingface.co/rhasspy/piper-voices/raw/c10ece1aade47bb51c153c893d14e5bf8e5b7117/es/es_ES/sharvard/medium/MODEL_CARD) declara español de España, dos hablantes y 22.050 Hz, corpus SHARVARD CC BY 3.0 y ajuste desde lessac. No se presenta la licencia del dataset como demostración automática de todas las condiciones de los pesos. Se conservaron ficha y avisos de paquetes. El wheel incluye COPYING GPLv3 y avisos de terceros; ORT incluye ThirdPartyNotices. Esta evaluación privada no resuelve ni inicia distribución/F6 y no convierte Piper GPL en Apache-2.0.

Compatibilidad **real comprobada** en el N5000: importaciones, carga y PCM, CPUExecutionProvider explícito. Sin CUDA/Torch/paquetes de entrenamiento, paquetes nuevos fuera del inventario o compilación de dependencias. El soporte de otras versiones de Python/arquitecturas queda NOT_SUPPORTED por este adaptador concreto.

## Auditoría y cambios

El coordinador ya era propietario de admisión, sesión/turno/generación, cancelación y drenaje. La autolectura copia la respuesta tras completar/persistir el turno; no tiene cliente LLM ni executor. Se conserva el supervisor F3/F4 y toda autoridad F2, incluido el catálogo de cuatro herramientas y las otras 19 bloqueadas.

Problemas encontrados: descubrimiento genérico de `piper`, requisito anterior de basename `piper-tts` incompatible con el wheel evaluado, hablante guardado pero ignorado, indicador de disponibilidad basado en PATH y falta de evidencia real. El wheel declara el entrypoint **`piper = piper.__main__:main`**; ese nombre también pertenece a un configurador de ratones Linux. No se ejecutó por PATH. Se comprobó en el código del wheel que `--output-raw` emite PCM16, que acepta speaker/length-scale y que el CLI carga la configuración adyacente aunque parsea `-c`.

El CLI oficial se probó offline. Producción utiliza la API del mismo paquete en un helper de inferencia mínimo para fijar CPU/hilos/spinning explícitamente, sin modificar ni parchear la distribución instalada. No hay servidor Python, conversación paralela o proceso permanente.

Código R3:

- `voice_tts.rs`: selección explícita del venv y voz, verificación de pesos/configuración y 1799 recursos instalados contra hashes derivados de los wheels autorizados. `piper_resources.json` sólo contiene rutas, tamaños y SHA, no modelos. No se eliminan verificaciones por latencia.
- `piper_worker.py`: helper estático incluido en el binario Rust, texto por stdin limitado a 16.000 bytes; PCM16LE mono 22.050 Hz por stdout, límite 16 MiB. Sólo voz española pinneada, evitando ramas de descarga automática de otros idiomas. ORT CPU, tres hilos intra-op, uno inter-op, sin spinning; OpenBLAS/OMP a uno. La configuración de optimización final y mediciones están en RESULTS.
- `voice_coordinator.rs`: preflight supervisado y cancelable de 15 s; síntesis de 90 s y reproducción de 180 s existentes, sin ampliar timeouts. Una operación/cola efectiva uno, síntesis completa antes de reproducir; no se introduce reproducción incremental. Se valida y usa el hablante. No hay I/O adicional bloqueante de modelo en Qt.
- `main.rs`: modos estrictos `--voice-piper-check` y `--voice-piper-worker` anteriores a logger/Backend/config/SQLite; aridad inválida o argumentos internos mal colocados fallan antes del arranque normal. El child Rust valida y hace `exec` del intérprete, conservando PID/pipes y la supervisión; cancelación no abandona un nieto Python.
- `config.rs`, `SettingsDialog.qml`, capacidades HTTP: campo de voz `piper_runtime`, modelo local/ruta absoluta, hablantes 0/1, velocidad existente y disponibilidad explícita. La configuración personal no se migra ni modifica automáticamente.
- Scripts de preparación, benchmarks, prueba con sumidero nulo y primera prueba humana separada; pruebas CLI, selección, integridad y QML. No cambian F2 ni herramientas/permisos.

## Contrato complementario R3

El contrato histórico F4 se conserva intacto como evidencia; esta sección documenta la evolución TTS. El backend no mantiene un modelo TTS residente. Preparar→hablar→idle/error sigue la identidad exacta del coordinador. Cancelar, ocultar/cerrar y cambiar sesión drenan los procesos propios antes de otra operación; los resultados antiguos no pueden reproducirse en una sesión nueva. Detener voz no afirma detener un LLM que siga trabajando.

El preflight y el worker usan el mismo ejecutable con entorno limpiado. Se comprueba que el venv apunta al `/usr/bin/python3` evaluado y se ejecuta directamente ese intérprete del sistema con `-I -B -S` y caché de bytecode inaccesible. Así `pyvenv.cfg` no redirige el descubrimiento de la biblioteca estándar; las dependencias siguen siendo exclusivamente las del site-packages privado verificado. Sólo se añade el site-packages verificado; no se evalúan `.pth`, sitecustomize ni PYTHONPATH. El helper no hereda HOME, DBus, socket de audio, bearer o credenciales; no escribe WAV/PCM ni necesita temporales. Los perfiles HOME/TMP/XDG del backend de pruebas siempre son privados. Los bytes PCM viven en RAM en el producto y se liberan al completar/cancelar. Los PCM instrumentales de benchmark se eliminan tras cada medición.

La verificación no constituye sandbox frente a otro proceso malicioso del mismo UID ni bloqueo forense/inmutabilidad de todos los pathnames. No hay endpoint para inyectar audio/resultados o síntesis en una conversación ajena. `/speak` conserva sesión/scope autenticados, `/speak/stop` identidad exacta. Ningún texto hablado ejecuta herramientas, da consentimiento o envía audio a proveedores. Sin micrófono, escucha continua, wake o atajos globales en R3.

## Estado y siguientes gates

Síntesis real y reproducción a sumidero **no audible** verificadas; cancelación, sesión y mock tienen evidencia automatizada separada de las pruebas humanas. T1–T5 siguen **NOT_TESTED** hasta sus confirmaciones independientes. La combinación preflight+worker final observada tarda aproximadamente 8,16–9,01 s antes de HTTP/salida; la espera sigue siendo material y no se aumentan timeouts; no se promete inicio audible inmediato ni inteligibilidad por disponer de PCM.

T1 está preparada en `/var/tmp/kda-f4r3-piper-aqrh6h0h/live-t1-1792419192735966309`, sin lanzamiento audible; se solicitó autorización independiente para una única frase. T2–T5 no se activan automáticamente.

La instalación temporal se conserva sólo mientras esta evaluación autorizada sigue abierta. Al cerrarla se eliminarán wheels, venv, voz, perfiles y temporales propios; sin prometer borrado forense. No se instala nada en el perfil personal. F4 sigue PARTIAL/NEEDS_FIXES; PTT/STT físicos R2 no se repiten ni se recertifican mediante fixtures.

## Apéndice T1 — Preparación y autorización audible pendiente

Se conserva el escenario `/var/tmp/kda-f4r3-piper-aqrh6h0h/live-t1-1792419192735966309`. Verificación no destructiva PASS: rama/HEAD basales, índice vacío, binario idéntico al evaluado, siete wheels aprobados por tamaño/SHA-256 y preflight Rust de voz/configuración/1799 recursos correcto. Backend headless abierto sólo con HOME/TMP/cuatro XDG privados, PipeWire inaccesible y DBus ausente; idle, cero hijos de voz y cero mensajes en la sesión sintética preparada. Backend cerrado y recogido.

[Comprobaciones T1](F4-R3-EVIDENCE/T1/preparation-check.json). No benchmarks, compilaciones, nuevas descargas, micrófono, salida audible o cambios KDE/perfil personal. El escenario sigue sin usar; la comprobación de la salida real se hará únicamente tras permiso audible explícito. Frase prevista única: «Hola. Soy KDE Assistant y esta es una prueba de voz en español.»

Estado T1: **NOT_TESTED**, autorización audible pendiente. Tras autorizar se pedirá ajustar el volumen a un nivel cómodo; después de una sola reproducción se esperará confirmación de escucha completa, claridad, naturalidad y volumen. No se inician T2–T5 ni se clasifica LIVE_TTS_PASS por integridad o preparación.

### T1 — Autorización, salida y ejecución técnica (valoración humana pendiente)

El usuario autorizó explícitamente «si, lo autorizo» y después confirmó «Volumen cómodo; reproducir ahora». Se consultaron únicamente la salida predeterminada y su volumen con wpctl inspect/get-volume, sin modificarlos: Audio Interno Estéreo analógico / ALC257 Analog. Antes del ajuste manual del usuario figuraba Volume: 1.15; no se impuso ni cambió volumen, mute, ruta o dispositivo. [Autorización](F4-R3-EVIDENCE/T1/audible-authorization.json), [salida](F4-R3-EVIDENCE/T1/output-check.json), [preparación física confirmada](F4-R3-EVIDENCE/T1/readiness.json).

Una única ejecución del escenario preparado: código de salida 0, sin error técnico observado; Preparing 79,4 ms → Speaking 8038,4 ms → Idle 11883,7 ms desde el reloj del launcher. Pared del comando completo 12,241 s. La ventana observada de Speaking fue ≈3,845 s; no es una medición del instante acústico de primer/último audio. No se calculan p50/p95 de esta observación.

[Procesos y ejecución](F4-R3-EVIDENCE/T1/technical-run.json), [estados de reproducción](F4-R3-EVIDENCE/T1/playback-observation.json). Se observaron Backend 11837, preflight 11845, worker 11846 (exec Rust→Python 3.14, mismo PID) y reproductor 11855 (pw-play resuelve a pw-cat). Todos recogidos. El backend volvió a idle, la sesión sigue sin mensajes y no hubo archivos PCM/WAV, micrófono, descarga o proveedor remoto. El recibo de una sola toma quedó consumido; no se repitió la reproducción ni se inició T2–T5.

Finalización técnica correcta; **clasificación LIVE_TTS pendiente de respuesta humana** sobre escucha completa, claridad, naturalidad, volumen y posibles defectos. No se declara LIVE_TTS_PASS por el estado idle o el exit 0.

### Dictamen actualizado T1 — LIVE_TTS_PASS; rendimiento pendiente

Tras la ejecución técnica correcta, el usuario confirmó: «Se escucho perfectamente, sin cortes y sin distorcion. El unico problema son los 8s.» [Confirmación humana](F4-R3-EVIDENCE/T1/human-confirmation.json). **LIVE_TTS_PASS exclusivamente para T1**, una frase española y una reproducción en la salida predeterminada probada. Escucha e integridad audible aceptadas por el usuario; sin puntuaciones individuales inventadas de naturalidad o volumen.

La espera de aproximadamente ocho segundos sigue siendo un defecto de experiencia pendiente, señalado expresamente por el usuario. El estado Speaking observado a 8038,4 ms es consistente con esa percepción; no equivale a medir acústicamente el primer audio. TTS global permanece **PARTIAL**, F4 **NEEDS_FIXES**. Este PASS no acepta el rendimiento ni certifica detener/reanudar/cambiar sesión o autolectura de forma humana. T2–T5 siguen sin realizarse; no se inicia ninguna automáticamente.

Se conservan temporalmente instalación y perfil privados para revisión, sin procesos de voz activos ni grabaciones crudas. No se repitió T1, benchmarks o micrófono; sin descargas, configuración KDE/personal, commits, push/merge/release, wake o F5. Se espera revisión técnica.
