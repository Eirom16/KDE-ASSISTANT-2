# F3 — Cierre de auditoría y consolidación local

## Dictámenes separados

- **A. Consolidación técnica: CONSOLIDATED.** Dos commits locales, árboles staged
  archivados y validados antes de cada commit, sin defectos críticos abiertos
  identificados en el alcance revisado. No constituye una prueba de ausencia
  universal de vulnerabilidades.
- **B. Aceptación de escritorio F3: PARTIAL.** Sin aceptación humana nueva;
  tecla física y gesto real de bandeja/foco siguen NOT_TESTED por decisión del
  usuario. Ningún diálogo del portal fue abierto ni atajo real registrado.

Rama: `recovery/f3-kde-wayland-integration`. Baseline obligatorio `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835`.

| Responsabilidad | Commit | Tree | Padre |
|---|---|---|---|
| A: infraestructura/producción y tres fixtures compatibles | `e4bb565e6961c1262a93121ab88057713157c6ac` | `7c1b5d9db9f61e98f0508f109595f10a779d9229` | `8b21c7fd505c8c6b5d0e9bcb5c7ecbd7455c9835` |
| B: suites, harnesses, runners y cinco informes | `1d2186e5dae78ae9ddc1dbed01c44c9b7d67f920` | `b71420c56ced8874e3889fb31e8b6f29ea43cd80` | `e4bb565e6961c1262a93121ab88057713157c6ac` |

## Hallazgos y correcciones

Se reprodujeron seis fallos de GlobalShortcuts antes de corregir: activación
adelantada perdida, Closed temprano anunciado como éxito, handle ajeno aceptado,
resultado Bind con acciones adicionales, listener que sobrevivía al Drop del
owner y estado registrado tras desaparecer el servicio. Se validan identidad
conexión/token y única acción; listeners se suscriben antes de crear/bindear;
Closed/owner-changed invalidan; Drop aborta y main recoge el resultado incluso
si completion compite con abort. Sin rebind automático ni autoridad LLM nueva.

Se corrigieron dos defectos de harness: socket Unix demasiado largo en TMPDIR
privado y reapertura F1B2 basada en Weak vacío antes de que Drop liberara flock.
El fixture ahora espera el lock real; SessionManager y sus garantías no cambian.
Regresiones y reproducciones FAIL/PASS: F3-ISSUES.md y F3-RESULTS.md.

## Gates y evidencia

Los cinco runners solicitados y fmt, Clippy default/legacy-fixtures y build
PASS en el workspace. A archivado: build y tres gates heredados PASS.
B archivado: build, F3 completo (incluye los tres gates) y F3 Plasma PASS.
Detalles exactos, manifests y tiempos en F3-COMMIT-VALIDATION.md.

Selección final específica: 21 PASS desktop/portal, 1 parent-death por hilo,
1 HTTP/SSE, 3 recursos, 3 tema; 12 QtTest F3, 12 W1R y 4 consentimiento F2.
F2 conserva 17 PASS. Se reportan IGNORED/FILTERED por selección sin sumar
runners anidados como pruebas independientes. Los grupos de runtime conservan
categorías AUTOMATED_QML/AUTOMATED_KWIN; no son HUMAN_VERIFIED.

## Garantías y límites verificados

- Instancia: flock privado por perfil canonicalizado, NOFOLLOW, UID/modo/nlink
  e inode; sin PID en archivo, unlink de lock ni búsqueda por nombre. Carreras
  simultáneas tienen un solo owner; duplicado no crea otro Qt. Listener reservado
  antes de bootstrap; sin bus la activación falla explícitamente.
- Procesos: handles Child propios, kill/wait y Drop; Qt se crea en el hilo líder
  de main. PR_SET_PDEATHSIG pertenece al hilo creador: el test termina sólo ese
  hilo manteniendo vivos proceso/hermano. PPID no prueba identidad de hilo;
  los launch sites deben conservar ese invariante. No se certifican nietos/F4.
- DBus Desktop1: sólo Activate/Toggle, sin argumentos de modelo, bearer, paths
  personales ni herramientas. DBusMenu conserva etiquetas/recientes existentes.
  SSE desktop es GET autenticado, Host/Origin exactos por puerto, cola limitada,
  cierre por lag y nueva suscripción sin replay. AuthToken es caché privada
  persistente 0700/0600, no exportación por entorno ni borrado seguro prometido.
- Bandeja/ventanas: restauración en una activación, maximizado, cierre a tray,
  nueva sesión y salida; callback diferido invalidado por generación. Foco
  solicitado con API pública, sin force stealing. Incidente Red histórico
  sigue sin causa demostrada; diagnóstico sólo metadatos durante pending.
- F2: system_info Green, read_file/create_file Yellow, edit_file Red. Las otras
  19 siguen bloqueadas, aunque flags legacy estén true. Sin notify LLM,
  ejecución arbitraria, permisos universales, cambios a UnknownOutcome o replay.
- Rendimiento: sin polling nuevo de teclas ni reconstrucción por token;
  supervisión heredada 200/100 ms conservada. Las muestras únicas originales
  12,27%/1,99% Qt visible/oculta no demuestran regresión cuantitativa. F5 no iniciado.

Blur y pin superior de esta implementación Wayland: NOT_SUPPORTED. HiDPI real,
varias salidas, X11 y overlay legado: NOT_TESTED. F2 mantiene confianza local
por UID/bearer, sin sandbox entre aplicaciones de la misma cuenta. El coste y
la asignación física del atajo no están medidos. Ninguna prueba offscreen se
presenta como certificación Wayland.

## Parche y estado final

`F3-CHANGES.patch`: baseline F2 → HEAD F3, 44 rutas modificadas.
SHA-256: `5ce8ae62f8dd16064b24e63dfb0d386820147eb7ff2ece2627be38082732f774`.
`git apply --check` PASS; aplicado a baseline limpio; 267 archivos resultantes
coinciden exactamente en contenido/modos con tree `b71420c56ced8874e3889fb31e8b6f29ea43cd80`.
Los 41 históricos sin seguimiento permanecen idénticos y excluidos.
Índice y archivos rastreados limpios; sólo históricos y estos tres entregables
posteriores permanecen sin seguimiento, evitando autorreferencia.

## Recomendación GATE-F3-W1

**READY para revisión/aceptación humana separada.** Verificar click real de
bandeja desde otra app, minimizar/restaurar/maximizar, consentimiento Red visible
mientras pending (denegar sin efectos), nueva sesión/revocación y salir sin hijos.
El atajo real seguirá NOT_TESTED hasta una autorización explícita posterior;
no se solicita ni se registra durante esta auditoría. No declarar F3 ACCEPTED
basándose sólo en estos gates automatizados.

Sin push, merge, release, root, cambios KDE personales ni inicio de F4/F5.
