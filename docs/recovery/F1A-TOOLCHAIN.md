# F1A — Toolchain y validación local

Toolchain comprobada el 2026-10-10: **Rust 1.99.0** (`b940084d7`, 2026-09-28), **Cargo 1.99.0** (`5f94df478`, 2026-08-27), Linux x86_64 `7.2.7-1-cachyos`. `rust-toolchain.toml` fija 1.99.0 con rustfmt y Clippy. No se afirma compatibilidad con Rust 1.75 ni con ningún MSRV no probado.

`Cargo.lock` se versiona como lock de esta aplicación. La excepción `!/Cargo.lock` mantiene las demás exclusiones de `*.lock`. Las validaciones usan `--locked`, que rechaza cambios necesarios en el lock. El lock fija dependencias Rust; no fija paquetes nativos, kernel, Qt ni modelos externos. Se comprobó con la caché Cargo local, sin descargar dependencias ni modelos. No equivale a una instalación desde cero ni a un build bit a bit reproducible.

Se necesitan Python 3, toolchain Rust indicada y dependencias nativas de compilación del proyecto: compilador C/C++, CMake, libclang, pkg-config, ALSA y X11/XInput/XTest para los módulos existentes. Qt/QML, Piper, modelos y una sesión de escritorio no se ejecutan en esta selección. El archivo de toolchain puede solicitar su instalación mediante rustup en otra máquina; eso no es una descarga de modelos ni parte de las pruebas aquí registradas.

Validación básica automatizada, sin servicio externo ni CI remoto:

```bash
python3 scripts/validate_f1a.py
```

Ejecuta formato, check y Clippy de todos los targets con lock; integración F1A, unitarios seguros y app_paths. Crea XDG/TMPDIR privados 0700, retira variables `*_API_KEY`, fuerza Cargo offline, limpia fixtures propios y conserva logs/resultados JSON fuera del repositorio. Cargo offline sólo restringe a Cargo: la selección excluye explícitamente los tests de red/descarga.

La selección unitaria excluye los 12 tests conocidos que necesitan Piper, el test que puede descargar ONNX y el test de imagen que usa loopback. Se enumeran en el script y se anuncian al ejecutarlo. No se simula Piper y no se presenta la selección como suite completa. Reproducción separada de los fallos heredados, con salida no cero si fallan:

```bash
python3 scripts/validate_f1a.py --piper-inherited
```

El test de montajes está `ignored` por defecto porque requiere namespaces de usuario/montaje, unshare, mount y umount. Se ejecuta en un namespace hijo privado con fixtures sintéticos, sin sudo ni montajes en la sesión original:

```bash
python3 scripts/validate_f1a.py --mounts
```

Si no están disponibles las primitivas o namespaces necesarios, la selección solicitada falla; el script no introduce un fallback permisivo. Los restantes tests F1A comprueban la contención del producto sin necesitar permisos de montaje. No ejecutar smokes de voz/proveedor, snap_ui, main ni descarga de modelos para sustituir esta selección.
