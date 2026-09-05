// valida_qml.sh - Smoke test: valida que el QML se carga sin errores
// usando qml6 con la plataforma offscreen.
//
// Requisitos: qml6 instalado (apt install qml6-module-qtquick qml6-module-qtquick-controls)

use anyhow::Result;
use std::process::Command;

fn main() -> Result<()> {
    let output = Command::new("qml6")
        .args(["-I", ".", "qml/Main.qml"])
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_ASSUME_STDERR_HAS_CONSOLE", "1")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Si el proceso sigue vivo despues de 3s, el QML cargo OK
    let mut child = output;
    std::thread::sleep(std::time::Duration::from_secs(3));

    match child.try_wait()? {
        Some(status) => {
            // Proceso termino. Si salio con codigo != 0 o > 1, fallo
            let stderr = child.stderr;
            // (consumimos el stderr implicitamente al finalizar)
            if !status.success() {
                eprintln!("qml6 fallo con {:?}", status);
                std::process::exit(1);
            }
        }
        None => {
            // Sigue vivo = OK, matamos
            child.kill()?;
            child.wait()?;
        }
    }

    println!("qml6 cargo qml/Main.qml sin errores criticos");
    Ok(())
}
