// snap_ui - Captura screenshot de la UI para QA visual
// Uso: snap_ui [ancho alto] [archivo_qml] [output.png]

use anyhow::Result;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let width: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(1280);
    let height: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(800);
    let qml_file: String = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "qml/Main.qml".to_string());
    let outputs: Vec<String> = if args.len() > 3 {
        args[3..].to_vec()
    } else {
        vec!["/tmp/snap.png".to_string()]
    };

    // Limpiar locks previos
    let _ = Command::new("pkill").args(["-f", "qml6|Xvfb"]).status();
    let _ = std::fs::remove_file("/tmp/.X99-lock");
    std::thread::sleep(Duration::from_millis(500));

    // Iniciar Xvfb
    let _xvfb = Command::new("Xvfb")
        .args([
            ":99",
            "-screen",
            "0",
            &format!("{width}x{height}x24"),
            "-ac",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::sleep(Duration::from_secs(2));

    if !Path::new(&qml_file).exists() {
        eprintln!("QML no encontrado: {qml_file}");
        let _ = Command::new("pkill").args(["-f", "Xvfb"]).status();
        std::process::exit(1);
    }

    // Lanzar QML
    let mut child = Command::new("qml6")
        .args(["-I", ".", &qml_file])
        .env("DISPLAY", ":99")
        .env("QT_QPA_PLATFORM", "xcb")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::sleep(Duration::from_secs(4));

    // Capturar
    for (i, out) in outputs.iter().enumerate() {
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "x11grab",
                "-video_size",
                &format!("{width}x{height}"),
                "-framerate",
                "1",
                "-i",
                ":99",
                "-frames:v",
                "1",
                "-update",
                "1",
                out,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            println!("Captura {i} guardada en {out}");
        } else {
            eprintln!("ffmpeg fallo en frame {i}");
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Limpiar
    let _ = child.kill();
    let _ = child.wait();
    let _ = Command::new("pkill").args(["-f", "Xvfb"]).status();
    let _ = std::fs::remove_file("/tmp/.X99-lock");

    Ok(())
}
