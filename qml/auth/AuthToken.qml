// Fallback de desarrollo para el módulo qml.auth.
// En ejecución real, main.rs genera ~/.cache/kde-assistant/qml/AuthToken.qml
// con el token y lo pasa con -I ANTES que este directorio, así que este
// archivo solo se usa al lanzar qml6 a mano o en valida_qml (sin token:
// la UI mostrará "No autorizado", que es el comportamiento honesto).
pragma Singleton
import QtQuick

QtObject {
    readonly property string token: ""
    readonly property string cacheDir: ""
}
