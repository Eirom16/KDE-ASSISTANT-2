// build.rs - KDE Assistant v2
//
// En Fase 1 este script es un placeholder. En fases siguientes:
// - Compilara resources.qrc con rcc (Qt Resource Compiler) para embeber
//   QML files y SVGs de Octicons en el binario.
// - Generara bindings de cxx-qt para exponer tipos Rust a QML.
//
// Por ahora solo emite un mensaje informativo al inicio del build.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources.qrc");
    println!("cargo:rerun-if-changed=qml/");
    println!("cargo:rerun-if-changed=assets/octicons/");

    // TODO(Fase 3): integrar cxx-qt-build para generar bridges C++ <-> Rust
    // TODO(Fase 3): usar qt_build::build_resources() para compilar resources.qrc

    // Link con Qt6 (placeholder, se activara en fase de UI)
    // println!("cargo:rustc-link-lib=Qt6Core");
    // println!("cargo:rustc-link-lib=Qt6Quick");
}
