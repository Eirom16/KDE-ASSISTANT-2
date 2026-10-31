# Auditoría de recuperación — diagnóstico

Fecha: 2026-10-10. Estado: **PARTIAL**. Se inspeccionó el código y se ejecutaron validaciones, reproducciones de seguridad y una integración HTTP con proveedor simulado. No se certifica el producto completo: voz real, proveedores externos, bandeja, atajos y overlay en Plasma requieren pruebas adicionales. No se inició Fase 1.

Base: rama `main`, commit `12da960ed4722401f1cb42b7a96d9cef912cf90a` (`before the migration to omarchy`, 2026-09-24). `origin/main` y la referencia remota consultada con `git ls-remote` coinciden; diferencia local/remota `0 0`. La modificación previa del usuario en `src/backend/hotkey_listener.rs` agrega `Context` y elimina un segundo `as_millis()`. Se conservó. Los resultados del árbol de trabajo **no equivalen** a los del commit publicado.

Se recomienda **reconstrucción parcial**, entregada en incrementos: conservar datos SQLite, contratos útiles, render QML y utilidades probadas; reconstruir los límites de autorización, ejecución/cancelación y coordinación de servicios. Una reescritura completa no está justificada por la evidencia. Arquitectura y comparación: [ARCHITECTURE.md](ARCHITECTURE.md). Pruebas y reproducciones: [RESULTS.md](RESULTS.md). Trabajo futuro y puertas de aceptación: [RECOVERY-PLAN.md](RECOVERY-PLAN.md).

## Criterio de evidencia y gravedad

- **CONFIRMED**: reproducción aislada o resultado observable de un comando.
- **STATIC**: comportamiento visible en el código; no supone una prueba del producto integrado.
- **NOT_TESTED**: falta prueba; **BLOCKED**: una dependencia/condición concreta la impide.
- **P0**: pérdida efectiva de un límite de seguridad, bloquea una entrega con herramientas.
- **P1**: fiabilidad, privacidad o seguridad material; necesario para v0.1 según alcance.
- **P2**: mantenimiento, documentación, optimización o acabado, después de los límites esenciales.

Las prioridades no son puntuaciones CVSS. Las herramientas corren con los privilegios del usuario, sin sandbox de filesystem. No se demostró escalada a root ni ejecución arbitraria de shell. Las reproducciones escribieron exclusivamente fixtures temporales.

## Hallazgos P0

### SEC-01 — Configuración permisiva cuando el lock está ocupado

**CONFIRMED.** `src/backend/tool_executor.rs:172` usa `try_read().unwrap_or_default()`; los defaults habilitan herramientas (`src/models/config.rs:674`) y permiten Documentos/Descargas/Escritorio (`src/models/config.rs:439`). Cada operación y la validación de rutas pueden obtener snapshots diferentes (`src/backend/tool_executor.rs:181`).

**Impacto:** una herramienta deshabilitada puede ejecutarse; la allowlist activa puede sustituirse por la predeterminada. **Condición:** coincidencia con un escritor del `RwLock`, por ejemplo actualización de configuración, cuyo lock se mantiene durante `save().await` (`src/backend/http_server.rs:1178`). El mismo patrón existe en `src/backend/ai_service.rs:143`: bajo contención puede cambiar proveedor/modelo a defaults. Si existe una key de fallback válida, el prompt podría enviarse al proveedor predeterminado en lugar del seleccionado; esta consecuencia es STATIC, no se usó red real. La prueba mantuvo un write guard: `system_info=false` fue rechazado sin contención y ejecutado con ella. No se tocó el directorio real Documentos para probar el efecto sobre archivos.

**Reproducción segura:** fixture de `Config`, `ToolExecutor`, `tools.system_info=false`; ejecutar antes y mientras se retiene `cfg.write().await`. Resultado: `disabled_without_lock=true success_with_write_lock=true`. **Recuperación propuesta:** snapshot válido inmutable por operación, fallo cerrado, y decisión de autorización dentro del límite de ejecución. No usar defaults ante contención.

### SEC-02 — Escape de rutas mediante symlinks y destinos derivados

**CONFIRMED** en tres casos independientes:

1. `create_file`: `Path::exists()` sigue links; un link colgante se trata como ruta inexistente (`src/backend/tool_executor.rs:200`, `:207`). Tras validar su padre, `fs::write` sigue el link (`:300`). Un link dentro de `allowed` hacia `outside/new.txt` inexistente creó el archivo fuera de la allowlist.
2. `copy_file`: valida el directorio destino (`:697`), agrega el nombre del origen (`:709`) y copia sin revalidar ese hijo (`:722`). Con `overwrite=true`, un hijo symlink sobrescribió el fixture exterior.
3. `edit_file`: el backup `.bak` se deriva de una ruta válida (`:335`) y se copia sin validar ese destino (`:337`). Un `.bak` symlink sobrescribió otro fixture exterior. Se invocó directamente el executor; esto **no** demuestra bypass rojo del endpoint manual, que devuelve 403.

**Impacto:** creación/sobrescritura fuera de las rutas permitidas, con los permisos del usuario. **Condiciones:** capacidad de preparar links en la zona permitida, una llamada autorizada a la operación y, para copia, `overwrite=true`. La operación de edición puede tener consentimiento legítimo sobre el archivo principal sin que ese consentimiento cubra el backup externo.

**Reproducción segura:** dos directorios hermanos temporales, autorizar sólo uno y colocar links hacia el otro; verificar contenido de los fixtures tras las llamadas. No requiere carrera. También hay una ventana **STATIC** TOCTOU entre canonicalización y acceso async (`:181`, `:292`, `:296`, `:300`, `:341`, `:722`): revalidar strings por sí solo no garantiza seguridad frente a sustitución posterior. **Recuperación propuesta:** acceso relativo a descriptores de directorio, resolución que no siga symlinks inesperados, creación exclusiva, validar destinos derivados y política explícita de reemplazo. La decisión debe cubrir la operación real, no sólo un path mostrado.

## Hallazgos P1

### SEC-03 — Autorización distribuida y endpoint manual como capacidad amplia

**CONFIRMED/STATIC.** `AiService::run_agent` pide confirmaciones (`src/backend/ai_service.rs:490`), pero `ToolExecutor::execute` sólo despacha y audita (`src/backend/tool_executor.rs:78`, `:133`); `ToolCtx.decided` es metadato, no prueba de consentimiento. `POST /api/tools/execute` deniega rojo (`src/backend/http_server.rs:1004`), pero ejecuta amarillo con `ToolCtx::auto` (`:1030`) aunque `confirm_sensitive=true`. La prueba HTTP creó un fixture con `ai.enable_tool_calling=false`, respuesta 200/success=true; rojo dio 403. El flag global filtra schemas, no todas las invocaciones.

**Impacto/condiciones:** cualquier cliente que posea el bearer completo puede ejecutar amarillas sin el flujo de confirmación del agente. La UI usa el endpoint para acciones explícitas; ese caso puede ser intencional, pero el servidor no distingue un clic humano de un cliente autónomo. El sidecar utiliza precisamente esta ruta para function calling (`voice_agent/main.py:480`, `:575`), aunque hoy no se arranca automáticamente. No se afirmó acceso sin bearer.

**Reproducción segura:** router real, token ficticio, proveedor local ficticio y create_file en tmp; ver RESULTS. **Recuperación:** definir capacidades por origen y consentimiento emitido por el servidor, vinculado a argumentos normalizados, sesión y turno. Redefinir operaciones destructivas: `copy_file(overwrite=true)` y `close_app` son amarillas por defecto (`src/backend/tool_registry.rs:32`), por lo que en voz automática el riesgo de pérdida de datos es mayor.

### SEC-04 — El bearer vacío no falla cerrado

**CONFIRMED** para el validador; cadena de fallo **STATIC**. `Backend::http_state` sustituye errores de creación de token por `String::new()` (`src/backend/mod.rs:130`). El comentario promete rechazo, pero `valid_bearer` compara dos cadenas vacías (`src/backend/auth.rs:147`, `:157`): `Authorization: Bearer ` devuelve true si expected es vacío.

**Impacto/condición:** si falla la creación/lectura del token y aun así arranca el router, una petición local con bearer vacío pasa ese chequeo. No se provocó un fallo en el token del usuario ni se demostró una ruta de navegador hasta la API. **Reproducción:** HeaderMap ficticio y expected vacío. **Recuperación:** rechazar token vacío y no publicar API privilegiada si falla su inicialización.

### SEC-05 — Token duplicado en archivo QML y capacidad de leer claves

**CONFIRMED/STATIC.** `ensure_server_token` deja el fixture `server.token` en 0600, pero `write_qml_auth_module` escribe `AuthToken.qml` con modo 0644 bajo umask 022 (`src/backend/auth.rs:142`). El mismo token autoriza la API completa; GET config devuelve la configuración con claves en claro (`src/backend/http_server.rs:1170`), confirmado sólo con clave ficticia. Token también se pasa por entorno a QML (`src/main.rs:376`, `:403`).

**Impacto/condiciones:** otro usuario podría leer el token si los directorios ancestrales son atravesables; el fixture de auditoría tuvo un ancestro 0700, por tanto **no** se probó lectura entre usuarios ni se inspeccionó ninguna clave real. Código con el UID del usuario ya tiene acceso amplio a sus archivos: este token no aporta aislamiento frente a ese adversario. `auth.rs:42` reutiliza archivos sin corregir permisos y `:61` ignora fallo de chmod. **Reproducción:** generar token ficticio, stat de ambos archivos, GET config con dummy key. **Recuperación:** creación privada desde el primer byte, verificar permisos, no duplicar secretos en assets públicos, API de configuración con claves redactadas y escritura separada.

### SEC-06 — Host y Origin no se comparan como autoridades exactas

**CONFIRMED.** `src/backend/auth.rs:182` acepta prefijos; `:196` acepta substrings. `Host: localhost.attacker.invalid:8765` y `Origin: https://localhost.attacker.invalid` pasan. Un Origin ordinario ajeno fue rechazado con 403 en la integración.

**Impacto/condiciones:** endurecimiento de origen inefectivo frente a nombres preparados. Sigue siendo necesario un bearer válido, salvo el fallo condicionado SEC-04; esto **no prueba por sí solo** DNS rebinding, CSRF ni acceso remoto. **Reproducción:** HeaderMap sin red. **Recuperación:** parsear URL/autoridad y permitir hosts/puertos exactos del transporte elegido; mantener auth independiente.

### SEC-07 — Configuración dañada reactiva defaults; escritura no atómica

**CONFIRMED/STATIC.** JSON inválido retorna `Ok(Config::default())` (`src/models/config.rs:573`), habilitando read/edit y wake word en la prueba. `save` copia backup, escribe en el archivo final y luego cambia permisos (`:601`, `:606`, `:610`); no usa rename atómico y no falla si chmod falla. La API reemplaza memoria antes de guardar (`src/backend/http_server.rs:1178`).

**Impacto/condiciones:** corrupción/interrupción de escritura puede reactivar capacidades previamente deshabilitadas; un fallo de persistencia deja RAM y disco divergentes. Existe una ventana de permisos al crear secretos y destino de backup sin endurecimiento frente a links. No se cortó energía ni se alteró config real. **Reproducción:** JSON inválido sólo en XDG_CONFIG_HOME temporal; inyección de fallo de escritura pendiente. **Recuperación:** validación explícita, configuración inválida bloquea capacidades, escritura atómica privada, publicación de memoria tras guardar y migraciones versionadas.

### SEC-08 — Resultados no confiables, descargas y límites tardíos

**STATIC.** Se envuelven resultados con advertencia de datos y truncado a 8k caracteres (`src/backend/ai_service.rs:632`), medida útil pero sin garantía contra prompt injection. No se valida que una llamada pertenezca al catálogo permitido del turno antes de despachar (`:471`, `:579`). El executor sí comprueba muchos flags individuales, con el fallo SEC-01. `show_image` acepta HTTP arbitrario y redirects, lee el body completo antes de limitarlo a 10 MB (`src/backend/tool_executor.rs:434`, `:475`); esto permite peticiones a servicios locales/privados bajo la capacidad de red de la app. `read_file` lee todo antes de truncar (`:360`); extracción de documentos lee/comprime antes de algunos límites (`src/backend/document_suite.rs:128`, `:158`).

**Impacto/condiciones:** contenido de web/archivos puede intentar inducir acciones; URLs controladas por LLM y respuestas sin Content-Length pueden consumir memoria o acceder a red privada. No se ejecutó una inyección contra proveedor real, SSRF contra servicios reales ni un archivo bomba. **Reproducción segura futura:** mock HTTP local con redirect y respuesta acotada, documento sintético pequeño que exceda un límite reducido, LLM simulado que solicite herramientas ausentes del catálogo. **Recuperación:** autorización independiente del prompt, catálogo efectivo, límites durante lectura, política explícita de destinos/redirects y normalización de resultados.

### SEC-09 — Procesos con alcance excesivo y sin cancelación garantizada

**STATIC.** Inicio elimina procesos coincidentes con `qml6.*Main\\.qml` usando `pkill -f` (`src/main.rs:478`, `:500`), sin verificar propiedad del proceso/instancia. `src/bin/snap_ui.rs:25` mata globalmente `qml6|Xvfb` y elimina el lock X99 (`:26`). No se ejecutaron estas rutas. `close_app` puede usar una app recordada globalmente o la ventana activa (`src/backend/tool_executor.rs:805`, `:827`); fallback SIGTERM por nombre (`:1424`) no expresa consentimiento sobre una ventana/instancia concreta.

`run_cmd` usa timeout alrededor de output sin `kill_on_drop` (`src/backend/tool_executor.rs:1550`); Piper carece de timeout/cancelación supervisada (`src/backend/tts/piper.rs:201`). No se halló uso de `sh -c` en la ruta de herramientas inspeccionada: argumentos separados es una fortaleza, aunque `.desktop` ejecuta el programa definido por la instalación del usuario.

**Impacto/condiciones:** pérdida de trabajo en otro proceso, hijos supervivientes, bloqueo ante ejecutable que no termina. **Reproducción segura futura:** proceso dummy propio en tmp que duerma, cancelación y comprobación de PID; nunca usar snap_ui o close_app sobre apps reales para validar. **Recuperación:** supervisión de hijos propios, identificadores concretos, timeout+kill+wait y cierre ordenado.

### SEC-10 — Texto privado se conserva en logs y auditoría

**STATIC.** Chat imprime parte del prompt (`src/backend/http_server.rs:1705`), voz imprime transcripción/respuesta (`src/main.rs:224`, `:276`), STT remoto imprime texto (`src/backend/speech_service.rs:251`) y Piper imprime parte del texto sintetizado (`src/backend/tts/piper.rs:235`). Auditoría serializa argumentos completos antes de truncar (`src/backend/tool_executor.rs:137`, `src/backend/session_manager.rs:218`), sin redacción de credenciales/contenido. No se encontraron claves reales mediante estas pruebas ni se imprimieron archivos privados; la reproducción usó sólo contenido ficticio.

**Impacto/condiciones:** logs de sesión o exportados para soporte y DB pueden contener documentos/transcripciones, incluso si el usuario sólo pretendía procesarlos; truncar no elimina secretos. No se comprobó acceso de otro usuario a esos logs/DB. **Reproducción segura futura:** introducir una cadena centinela ficticia en prompt/args, capturar logger y consultar audit fixture, comprobar qué persiste. **Recuperación:** logs de contenido opt-in, redacción antes de persistir, políticas de retención/exportación y permisos privados. La configuración ya almacena keys en plaintext, por lo que SEC-05/07 son especialmente relevantes.

### REL-01 — El commit publicado no compila; reproducibilidad ausente

**CONFIRMED.** Árbol local: check y Clippy pasan. Archivo de HEAD en checkout aislado: check exit 101, cuatro E0599 por trait `Context` ausente (`src/backend/hotkey_listener.rs:320`, `:326`, `:331`, `:334`, numeración de HEAD). HEAD además conserva un segundo `as_millis()` sobre un valor ya convertido; no se atribuye un diagnóstico adicional al compilador porque primero falla el trait. fmt local falla en `src/backend/hotkey_listener.rs:419` y `src/lib.rs:6`.

`Cargo.lock` existe localmente pero no está versionado por `.gitignore:15`; no hay CI, rust-version ni toolchain fijado (`Cargo.toml:1`). Rust 1.75 anunciado no fue comprobado. Python no tiene manifiesto/lock de dependencias. **Recuperación:** recuperar una base reproducible y pruebas herméticas antes de aceptar funcionalidad. La prueba de HEAD reutilizó el lock local: no valida resolución fresca de dependencias.

### REL-02 — Voz obligatoria impide un producto de texto independiente

**CONFIRMED/STATIC.** 12 tests fallan por ausencia de Piper. `SpeechService::new` depende de constructor Piper (`src/backend/speech_service.rs:50`); éste falla sin ejecutable (`src/backend/tts/piper.rs:61`). Backend inicializa voz/modelos antes de la UI/API (`src/backend/mod.rs:76`, `:89`), incluso con wake word apagado. El aviso de voz no disponible posterior no rescata ese error.

Arranque await descarga secuencial de base/Piper/openWakeWord (`src/backend/mod.rs:154`, `src/backend/model_downloader.rs:213`), sin seleccionar servicios por capacidades; elegir tiny añade tiny y conserva base (`src/backend/mod.rs:160`). Modelos se declaran sin hash (`model_downloader.rs:213`), cliente de descarga sin timeout explícito (`:137`). No se midió un arranque frío ni se descargaron modelos. **Recuperación:** texto debe arrancar sin audio/modelos/red; carga opcional bajo demanda, descarga consentida verificable y estado de disponibilidad separado.

### REL-03 — Streaming corrupto y finalización demasiado tolerante

**CONFIRMED** UTF-8: `String::from_utf8_lossy` por chunk de red (`src/backend/ai_service.rs:318`) transforma `Español` en `Espa��ol` si se divide el byte de ñ. Integración normal entrega tokens antes de terminar el proveedor (42 ms al primer chunk en fixture, no latencia de proveedor real).

**STATIC:** se ignoran deltas JSON inválidos (`:341`), EOF puede aceptarse sin DONE (`:383`), argumentos de tools inválidos se sustituyen por objeto vacío (`:674`), finish_reason no dirige estado (`:95`). **Recuperación:** buffer de bytes/decoder incremental, contrato de fin/error y casos de fragmentación, truncamiento y múltiples llamadas en mocks.

### REL-04 — Cancelación corta el coordinador, deja viva la petición

**CONFIRMED.** `/api/chat/cancel` aborta el handle de `run_agent` (`src/backend/http_server.rs:189`); éste contiene `call_task` separado (`src/backend/ai_service.rs:429`). Soltar el JoinHandle al abortar no aborta ese hijo. En fixture: respuesta cancelled/200, proveedor todavía activo a 300 ms y completó después de 2 s. Se conservaron 3 mensajes: turno completo anterior y usuario del turno cancelado. Contradice comentario de cancelación que dice no persistir nada (`http_server.rs:181`, `:1737`).

**Impacto:** consumo remoto tras cancelar, estado/persistencia ambiguos, posibles respuestas tardías. No se cobró ninguna API real. **Recuperación:** scope de tareas por turno, cancellation token propagado, cierre de stream y estados cancelled/partial explícitos, pruebas de disconnect y cancelación durante herramientas/STT/TTS.

### REL-05 — Persistencia de turnos y herramientas no es transaccional

**STATIC; happy path HTTP CONFIRMED.** SQLite real conserva usuario+assistant en fixture. `add_message` inserta y actualiza sesión por separado (`src/backend/session_manager.rs:468`); eliminación usa varios statements (`:337`). HTTP ignora errores de guardado (`src/backend/http_server.rs:1737`, `:1761`) y persiste nuevos mensajes sólo si el agente termina bien (`:1755`). Una herramienta puede tener efecto y perder su historia ante error posterior. Voz conserva sólo intercambio final (`src/backend/voice_pipeline.rs:833`, `:1099`). SQLite no habilita foreign_keys explícitamente al abrir (`session_manager.rs:73`).

Manual tools escribe un resultado tool sin assistant tool_calls asociado (`http_server.rs:1039`). Texto corta los últimos 40 mensajes sin reparación de pares (`:1858`), a diferencia de voz (`voice_pipeline.rs:151`). Imágenes guardadas no se convierten a contenido de visión para el LLM (`src/backend/ai_service.rs:223`). **Impacto:** contextos inválidos o incompletos, UI y realidad divergen. **Recuperación:** journal de turno/efectos, transacciones, errores visibles y proyección de historial coherente; contrato de adjuntos explícito.

### VOI-01 — Estado y sesión de voz no quedan vinculados al turno

**STATIC.** `process_utterance` crea op-id (`src/backend/voice_pipeline.rs:795`) pero no lo comprueba antes de publicar resultado. La máquina aplica incluso transiciones declaradas inválidas (`src/backend/voice_state.rs:149`). Se consulta sesión al construir contexto (`voice_pipeline.rs:997`) y de nuevo al persistir (`:839`): cambiarla entre ambos puede guardar en otra sesión. El grace de barge-in resta timestamp u64 de un valor truncado a AtomicU32 (`:365`, `:500`); la comparación de 350 ms no representa tiempo transcurrido real.

**Impacto:** resultados tardíos y estado avatar desordenados, interferencia de turnos y barge-in. Tests correspondientes bloqueados por Piper; no se afirma reproducción en micrófono. **Recuperación:** una autoridad de estado, identidad de turno/sesión inmutable, reloj monotónico, transiciones rechazadas y cancelación cooperativa.

### VOI-02 — Política de captura/privacidad contradice el contrato

**STATIC.** `main.rs:153` arranca captura sólo con wake habilitado: apagar wake afecta también PTT/dictado nativos. `speech_service.rs:235` con motor `auto` elige STT remoto cuando el proveedor activo es Groq y tiene key (`:277`), mientras config default es auto (`src/models/config.rs:322`) y documentación promete voz local. Wake fallback emite detección por energía/ZCR sin reconocer frase (`src/backend/hotword.rs:270`), contradiciendo el requisito de ML.

**Impacto:** modo PTT dependiente de wake; audio puede salir del equipo según disponibilidad de key, sin elección inequívoca local/remoto; falsas activaciones en fallback. No se capturó ni envió audio. **Recuperación:** captura y activación separadas, local predeterminado, STT remoto sólo con elección explícita, ML ausente deshabilita wake y conserva PTT.

### VOI-03 — Sidecar experimental incompatible con una autoridad única

**STATIC/BLOCKED.** `voice_agent/main.py:2` afirma estar gestionado por main, pero `src/main.rs:152` deja `sidecar_child=None`; `src/backend/mod.rs:119` inicializa sidecar_active=false sin activación en src. Python mantiene otro micro/pipeline Groq/Piper (`main.py:455`), umbral 0.25 (`:84`), schemas/contexto propios (`:476`). Control de 8766 no tiene auth (`:388`, `:400`); si se inicia manualmente, un proceso local puede abrir conversación/captura y provocar transcripción remota (`:369`, `:382`). `HistoryTap` envía respuesta vacía (`:341`).

**Condición de riesgo:** lanzar manualmente Python con dependencias/key/modelos; no ocurrió en esta auditoría. Módulos aiohttp/openwakeword/pipecat ausentes en el intérprete auditado. **Reproducción futura segura:** dependencias aisladas y STT/transporte ficticios; sin micrófono ni key real. **Decisión propuesta:** retirar el sidecar de la arquitectura de producción; conservarlo sólo como experimento documentado hasta decidir su archivo/eliminación. Un posible futuro worker de inferencia nunca poseerá permisos, sesión ni conversación.

### DES-01 — Integración escritorio incompleta y supervisión insuficiente

**STATIC/NOT_TESTED.** No hay bindings Qt; UI es qml6 hijo más REST/SSE. Main selecciona Wayland automáticamente (`src/main.rs:323`), pero atajos rdev documentan X11 (`src/backend/hotkey_listener.rs:3`), sin integración nativa KGlobalAccel. Señales DBus se emiten (`:295`) sin propietario/receptor implementado del servicio org.kde.assistant; la entrega práctica también escribe un archivo (`:314`). Blur son métodos que sólo loguean (`src/backend/kde_integration.rs:86`, `:93`).

Overlay layer-shell real existe (`qml/agent/AgentOverlay.qml:12`, `:47`), pero main hornea flag de overlay antes de demostrar que vive (`src/main.rs:343`), y sólo supervisa el hijo Main (`:427`). Si overlay falla, avatar standalone sigue deshabilitado (`qml/Main.qml:1334`). Rutas QML dependen de CWD (`src/main.rs:371`, `:399`); resolver probado de `src/app_paths.rs:1` no se usa en main. **Recuperación:** capacidades KDE verificadas por sesión, rutas de instalación usadas, handshake/supervisión y fallback de overlay; no certificar Wayland por un smoke offscreen.

### PERF-01 — Trabajo síncrono en caminos async y de audio

**STATIC.** SQLite y std::Mutex en handlers async (`src/backend/http_server.rs:1655`) con busy timeout 5 s (`src/backend/session_manager.rs:86`); lectura/hash completo síncrono (`src/backend/model_downloader.rs:184`); inferencia wake dentro de task Tokio (`src/backend/hotword.rs:201`); generación de preview pdftoppm con `.status()` síncrono (`src/backend/tool_executor.rs:1896`). Callback de cpal llama voice.push_audio (`src/backend/mod.rs:219`); emit_level hace create_dir/write síncronos (`src/backend/voice_pipeline.rs:425`) además de locks (`:388`). Por tanto Tokio no garantiza ausencia de bloqueo del callback ni de workers.

Piper se inicia por síntesis (`src/backend/tts/piper.rs:201`); warmup (`src/main.rs:96`) no mantiene ese proceso/modelo caliente. Whisper prewarm se agenda aun sin uso local (`main.rs:80`). TTS comparte stop flag (`src/backend/speech_service.rs:159`); productor abortado no garantiza matar el proceso, y la espera de voz ignora `Ok(Err(...))` de reproducción (`voice_pipeline.rs:945`). **Impacto:** jitter, dropouts, latencias y errores de audio ocultos; no se cuantificaron en hardware. **Recuperación:** callback limitado a copia/canal acotado, workers dedicados y supervisados, recursos lazy, trazas de latencia por etapa.

## Hallazgos P2

### MAINT-01 — Concentración de responsabilidades y código experimental

**STATIC.** `tool_executor.rs` tiene 3.088 líneas, `http_server.rs` 2.110, `voice_pipeline.rs` 1.294, `session_manager.rs` 962, `qml/Main.qml` 1.830 y SettingsDialog 1.655. Son conteos del árbol auditado, no métricas de calidad por sí solos. HTTP mezcla transporte, memoria, voz, permisos y persistencia; herramientas mezclan políticas de paths, documentos, procesos y KDE. Instancias KdeIntegration se crean en backend y main (`src/backend/mod.rs:92`, `src/main.rs:246`). Canales SSE y archivos de estado mantienen caminos paralelos.

Hay sidecar/spike no integrados y bytecode Python versionado. `ndarray` y `once_cell` son dependencias directas sin usos encontrados en src (`Cargo.toml:52`, `:62`); candidatos a revisar, no se declaró que todo el árbol transitivo sea innecesario. Todos los modelos/audio compilan como dependencias obligatorias (`Cargo.toml:33`, `:45`). **Recuperación:** límites por servicio, feature/capacidad de voz, retiro documentado de experimentos después de asegurar regresiones.

### MAINT-02 — Pruebas que aparentan cobertura o requieren infraestructura

**CONFIRMED/STATIC.** Unit tests mezclan puro dominio con Piper obligatorio, 12 fallos de infraestructura. Test `download_and_extract_onnx_runtime` no está ignored y puede descargar en cargo test (`src/backend/wakeword_ml.rs:520`); aquí retornó por bundle existente, no verificó descarga. `tests/agent_smoke.rs:25` retorna si no hay key y Cargo imprime passed. Smokes TTS/wake están ignored (`tests/tts_stream_smoke.rs:18`, `tests/wake_resample_sim.rs:68`). Wrappers valida_qml/valida_agent consideran vivo a los 3 s como éxito y dejan stderr en pipe (`src/bin/valida_qml.rs:14`, `src/bin/valida_agent.rs:14`); valida_agent_ui sí usa harness con assertions (`src/bin/valida_agent_ui.rs:8`).

**Recuperación:** separar tests offline deterministas de pruebas de hardware/red opt-in, indicar skips explícitos; no usar smokes de carga como verificación de flujo real.

### MAINT-03 — Documentación mezcla intención y realidad

**STATIC.** AGENTS sigue siendo regla de trabajo, pero sus afirmaciones no acreditan implementación:

| Documento/evidencia | Contradicción observable |
| --- | --- |
| `AGENTS.md:94`, `KDE-ASSISTANT-V2-PROMPT.md:46` | Q_INVOKABLE/signals frente a HTTP/SSE y qml6 separado (`src/main.rs:371`, `http_server.rs:1640`). |
| `AGENTS.md:18` | WebRTC VAD frente a VAD RMS/adaptativo propio (`src/backend/vad.rs:1`); no dependencia WebRTC en Cargo. |
| `AGENTS.md:21`, `KDE-ASSISTANT-V2-PROMPT.md:191` | .qrc embebido frente a SVG/rutas de disco; README lo llama futuro (`README.md:67`, `qml/Octicon.qml:31`). |
| `AGENTS.md:111` y `:321`, `README.md:141` | Defaults wake 0.8 y 0.5 incompatibles; código Rust 0.5 (`src/models/config.rs:299`), Python 0.25 (`voice_agent/main.py:84`). |
| `AGENTS.md:336`, `README.md:126` | Voz local frente a auto→Groq y sidecar remoto (`speech_service.rs:235`, `voice_agent/main.py:463`). |
| `README.md:115` | Describe UI usando /chat/complete; QML usa streaming /api/chat (`qml/Main.qml:188`). |
| `voice_agent/main.py:2` | Sidecar gestionado por main frente a None permanente (`src/main.rs:152`). |
| `README.md:223`, `DESKTOP-AGENT-DESIGN.md:5` | Checkmarks/“auditoría completa” históricos no acreditan el flujo de esta revisión; fallan validaciones y quedan hardware/Plasma NOT_TESTED. |

Se leyeron AGENTS, PLAN, PROMPT, apple-DESIGN y documentos de avatar/voz relevantes. No se reemplazaron contratos de diseño ni licencias de blobatar. La épica de recuperación ya exige baselines y evidencia: [seguimiento #1](https://github.com/Eirom16/KDE-ASSISTANT-2/issues/1). Estos informes no marcan sus tareas como terminadas.

## Rendimiento y disponibilidad: límites de la conclusión

Se midió sólo QML offscreen sin backend: Main ≈1,499% de un core y RSS 102,14–102,21 MiB; DevPreview ≈2,832% y 62,88–62,92 MiB, ventanas de 6,003 s. No son cifras del producto en reposo ni del compositor. Backend completo, tiempo de carga de modelos, STT/TTS real y procesos después de cierre del producto: **NOT_TESTED/BLOCKED**. Los logs y método exacto están en RESULTS.

Fortalezas verificables para reutilizar: herramientas usan arrays de argumentos, servidor se liga a 127.0.0.1 (`http_server.rs:1642`), SQL parametrizado, parser/chunker/VAD/normalización tienen tests puros aprobados, harness de avatar pasa, y el happy path REST+SSE+SQLite funciona con proveedor simulado. Ninguna de estas fortalezas compensa los P0.

No se recomienda distribuir una versión con herramientas antes de cerrar SEC-01/02 y verificar el límite completo de autorización. Las decisiones pendientes de alcance, voz, IPC y escritorio se enumeran en RECOVERY-PLAN; no se implementaron cambios de producción durante la auditoría.
