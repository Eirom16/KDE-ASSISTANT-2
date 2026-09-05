// snap_ui.sh - Captura screenshots de la UI en distintos estados para QA visual
// Requiere: Xvfb, ffmpeg, qml6
//
// Uso: ./snap_ui.sh /tmp/snap1.png /tmp/snap2.png

use anyhow::Result;
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Uso: snap_ui <output1.png> [output2.png ...]");
        std::process::exit(1);
    }

    // Iniciar Xvfb
    let _xvfb = Command::new("Xvfb")
        .args([":99", "-screen", "0", "1280x800x24", "-ac"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::sleep(Duration::from_secs(1));

    // Lanzar QML
    let mut child = Command::new("qml6")
        .args(["-I", ".", "qml/Main.qml"])
        .env("DISPLAY", ":99")
        .env("QT_QPA_PLATFORM", "xcb")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    std::thread::sleep(Duration::from_secs(4));

    // Capturar
    for (i, out) in args.iter().enumerate() {
        let status = Command::new("ffmpeg")
            .args([
                "-f",
                "x11grab",
                "-video_size",
                "1280x800",
                "-i",
                ":99",
                "-frames:v",
                "1",
                "-update",
                "1",
                "-y",
                out,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if !status.success() {
            eprintln!("ffmpeg fallo en frame {}", i);
        } else {
            println!("Captura {} guardada en {}", i, out);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Limpiar
    let _ = child.kill();
    let _ = child.wait();
    Command::new("pkill").args(["-f", "Xvfb"]).status().ok();

    Ok(())
}
