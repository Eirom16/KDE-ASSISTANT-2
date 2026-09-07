// TrayMenu.qml - System Tray para KDE Plasma (Qt.labs.platform)
// Icono en la bandeja del sistema con menu contextual multifuncion.

import QtQuick
import Qt.labs.platform

SystemTrayIcon {
    id: tray

    // === Props ===
    property bool windowVisible: true

    signal showRequested()
    signal hideRequested()
    signal newSessionRequested()
    signal settingsRequested()
    signal quitRequested()

    visible: true
    tooltip: qsTr("KDE Assistant v2")

    // Icono: usamos el SVG de hubot desde el filesystem (no qrc aun)
    icon.source: Qt.resolvedUrl("../assets/octicons/hubot-16.svg")
    icon.mask: false

    // Menu contextual
    menu: Menu {
        MenuItem {
            text: tray.windowVisible ? qsTr("Ocultar") : qsTr("Mostrar")
            onTriggered: {
                if (tray.windowVisible) tray.hideRequested()
                else tray.showRequested()
            }
        }
        MenuSeparator { }
        MenuItem {
            text: qsTr("Nueva sesión")
            onTriggered: tray.newSessionRequested()
        }
        MenuItem {
            text: qsTr("Dictar (Push-to-Talk)")
            onTriggered: tray.showRequested()
        }
        MenuSeparator { }
        MenuItem {
            text: qsTr("Configuración")
            onTriggered: tray.settingsRequested()
        }
        MenuSeparator { }
        MenuItem {
            text: qsTr("Salir")
            onTriggered: tray.quitRequested()
        }
    }

    // Click en el icono: muestra/oculta la ventana
    onActivated: function(reason) {
        if (reason === SystemTrayIcon.Trigger) {
            if (tray.windowVisible) tray.hideRequested()
            else tray.showRequested()
        }
    }
}