# F4-R5 — Pendientes y límites

Sin consolidación ni pruebas físicas nuevas. F4 global permanece PARTIAL.

| ID | Clase | Situación / tratamiento |
|---|---|---|
| R5-COLD | Limitación de CPU | El modo preparado no elimina la primera carga ONNX. La primera lectura manual puede costar más que la ruta fría por la verificación antes de reutilizar el canal; preparación durante LLM y lecturas consecutivas son los beneficios buscados. No se amplían timeouts. |
| R5-MEMORY | Riesgo de RAM | Una sesión preparada retiene cientos de MiB. Default OFF, TTL acotado, vida máxima y monitor. Los umbrales derivan de la envolvente medida, pero no constituyen un RLIMIT ni certificación ante OOM real. No se fuerza presión del sistema. |
| R5-PRESSURE | Política corregida con evidencia | El criterio inicial PSI positivo aislado rechazó preparación con PSI 0,17% y MemAvailable 4.997.476 KiB. Ahora PSI complementa un margen de memoria reducido; se conserva rechazo por capacidad insuficiente o métricas inválidas. No se atribuyen todos los fallos iniciales a PSI: faltaban snapshots. Umbrales y casos están documentados y cubiertos por UNIT; presión física extrema no probada. |
| R5-REPRO | Incertidumbre de rendimiento | Host no dedicado, frecuencia/carga/page cache/temperatura variables; una compilación ajena retrasó el inicio de tests. No inferencia concurrente con compiladores. Comparación agrupada separada de la alternada. No modificar governor, afinidad ni aplicaciones ajenas. |
| R5-HUMAN | Pruebas físicas pendientes | R5 audible NOT_TESTED. T1 R3 conserva PASS histórico; no certifica esta nueva ruta. T2–T5 humanos NOT_TESTED. Sin reutilizar recibos, altavoces o micrófono. |
| R5-SAME-UID | Límite de integridad | Hashes repetidos y watches complementarios, inodos modelo/config fijados. No sandbox completo contra otro proceso malicioso del mismo UID ni transacción atómica sobre módulos importados por path. |
| R5-LONG-TEXT | Límite del motor | Texto 16 KB y PCM 16 MiB acotados; monitor y deadlines revocan exceso. El mayor consumo de una frase extrema no se certifica con frases cortas. |
| R5-DISTRIBUTION | Deuda fuera de alcance | Instalación privada exacta Piper 1.8.0/ORT1.31.0/CPython3.14 y voz es_ES-sharvard-medium. Licencias y avisos de R3 conservados; no empaquetado F6 ni instalación general de entornos arbitrarios. |
| R5-KWIN | Incidente heredado | F3 «Active own window did not hide» fue intermitente en R4. Gate actual PASS; no se reprodujo ni se declara corregido. Sin cambios F3. |
| R5-INC-01 | Incidente histórico protegido | Dispatch interno estricto antes de Backend. Perfiles privados nuevos; sin inspección ni rollback del perfil personal. |
| R5-WAKE | Fuera de alcance | Hey Jarvis continúa bloqueado; no se implementa ni activa. PTT humano R2 se conserva sin repetir micrófono. |

Resultados definitivos, cobertura no ejecutada y limpieza: RESULTS. Cifras de retención, latencias y dispersión: BENCHMARKS.

## Correcciones durante la validación

- Recuperación: una cancelación de salud/integridad dejaba un token cancelado para solicitudes posteriores. Rotación de época/token idempotente, sin revocar el token de una configuración concurrente; recuperación de la primera nueva lectura tras restaurar la copia privada: PASS automatizado.
- Señal de terminal LLM: Completed también despierta el futuro del registry; queda excluido de la revocación para conservar lectura final legítima.
- Reposo: el primitivo heredado sondeaba cada 10 ms. Con varios futuros del manager producía aproximadamente 0,29–0,36 s de CPU Backend en ventanas de 7 s, aunque Piper marcaba cero ticks. Se sustituye por Notify más flag persistente, registro anterior a la comprobación y tests de múltiples waiters/cancelación temprana. La repetición final midió 0,00–0,02 s de CPU Backend por ventana de 7 s (n=5), frente al sondeo previo; Piper cero ticks. Los datos previos se separan de la repetición final.
- Expiración: un hijo ya terminado podía aparecer brevemente como zombie sin VmRSS; el monitor lo interpretaba como fallo de métricas y revocaba la siguiente generación. Se distingue State Z/X y desaparición de /proc de métricas inválidas de un proceso vivo; Child::wait conserva ownership de la salida. UNIT cubre esta distinción y se repiten las expiraciones reales.
- Clippy: la primera ejecución F2 detectó `while_let_loop` en el recorrido de padres del watcher, en default y legacy. Se expresa como `while let` conservando el mismo recorrido, filtros y watches; no se desactiva el lint. El FAIL inicial permanece en el ledger, con repetición focalizada y de gates afectados.
- Arnés extendido: la primera ruta de socket medía 119 bytes. Nombres privados cortos y validación del límite AF_UNIX corrigen el lanzamiento sin usar el runtime habitual ni ampliar esperas. Batería extendida repetida: 14 PASS.

## Estado al cierre

Sin defectos R5 pendientes reproducidos por las pruebas finales: lifecycle y gates PASS después de las correcciones documentadas. Permanecen las limitaciones de cold start (primera lectura ~7,9 s; recarga variable), RAM retenida hasta TTL, presión/OOM no probada físicamente, aislamiento sin sandbox completo y distribución/licencias pendiente. Las dos fallas AF_UNIX de los arneses y el FAIL inicial Clippy se conservan junto con sus repeticiones PASS.

Aceptación audible R5 y T2–T5 humanos NOT_TESTED. Se requiere autorización independiente para altavoces; no se solicita ni consume en esta entrega. T1 R3 y PTT R2 conservan exclusivamente su evidencia histórica. F4 global PARTIAL; revisión técnica pendiente.
