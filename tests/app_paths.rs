use kde_assistant_lib::app_paths::resolve_data_dir_from;
use std::fs;
use std::path::Path;

fn create_main_qml(root: &Path) {
    let qml_dir = root.join("qml");
    fs::create_dir_all(&qml_dir).expect("crear directorio QML de prueba");
    fs::write(qml_dir.join("Main.qml"), "// fixture").expect("crear Main.qml de prueba");
}

#[test]
fn installed_binary_resolves_sibling_share_directory() {
    // Given
    let sandbox = std::env::temp_dir().join(format!(
        "kde-assistant-paths-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let executable = sandbox.join("usr/bin/kde-assistant");
    let installed_data = sandbox.join("usr/share/kde-assistant");
    create_main_qml(&installed_data);

    // When
    let resolved = resolve_data_dir_from(
        &executable,
        &sandbox.join("source"),
        &sandbox.join("unrelated-working-directory"),
        None,
    )
    .expect("resolver datos instalados");

    // Then
    assert_eq!(resolved, installed_data);
    fs::remove_dir_all(sandbox).expect("limpiar fixture");
}

#[test]
fn explicit_data_directory_has_priority() {
    // Given
    let sandbox = std::env::temp_dir().join(format!(
        "kde-assistant-paths-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let explicit_data = sandbox.join("custom-data");
    create_main_qml(&explicit_data);

    // When
    let resolved = resolve_data_dir_from(
        &sandbox.join("bin/kde-assistant"),
        &sandbox.join("source"),
        &sandbox.join("cwd"),
        Some(&explicit_data),
    )
    .expect("resolver override de datos");

    // Then
    assert_eq!(resolved, explicit_data);
    fs::remove_dir_all(sandbox).expect("limpiar fixture");
}
