// TrayMenu.qml - System Tray para KDE Plasma (Qt.labs.platform)
// Icono en la bandeja del sistema con menu contextual multifuncion.

import QtQuick
import Qt.labs.platform

Item {
    id: root

    // === Props (reexpuestas para Main) ===
    property bool windowVisible: true
    property bool alwaysOnTop: true
    property string pttShortcut: "Super+Shift+V"
    property var recentSessions: []   // [{id, title}]
    property string backendUrl: "http://127.0.0.1:8765"
    property string authToken: {
        try {
            var e = Qt.platform.environment
            var t = e ? e["KDE_ASSISTANT_TOKEN"] : null
            return t ? String(t) : ""
        } catch (err) { return "" }
    }
    function setAuth(xhr) {
        if (authToken !== "") xhr.setRequestHeader("Authorization", "Bearer " + authToken)
    }

    signal showRequested()
    signal hideRequested()
    signal newSessionRequested()
    signal openSessionRequested(string sessionId)
    signal dictateRequested()
    signal quickSearchRequested()
    signal quickOpenAppRequested()
    signal quickOpenFolderRequested()
    signal alwaysOnTopToggled(bool on)
    signal settingsRequested()
    signal quitRequested()

    function refreshRecents() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/sessions")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === 4 && (xhr.status === 200 || xhr.status === 0)) {
                try {
                    var list = JSON.parse(xhr.responseText)
                    recentSessions = list.slice(0, 5)
                } catch (e) {}
            }
        }
        xhr.send()
    }

    Component.onCompleted: refreshRecents()
    Timer {
        interval: 15000
        running: true
        repeat: true
        onTriggered: refreshRecents()
    }

    SystemTrayIcon {
        id: tray
        visible: true
        tooltip: qsTr("KDE Assistant v2")

        // Icono: nombre del tema hicolor con fallback al SVG del repo.
        icon.name: "kde-assistant"
        icon.source: Qt.resolvedUrl("../../assets/icons/kde-assistant.svg")
        icon.mask: false

        // Menu contextual
        menu: Menu {
            MenuItem {
                text: root.windowVisible ? qsTr("Ocultar") : qsTr("Mostrar")
                onTriggered: {
                    if (root.windowVisible) root.hideRequested()
                    else root.showRequested()
                }
            }
            MenuItem {
                // F2-1: abre la ventana y deja el input listo; el dictado real
                // es el PTT global (ver atajo entre paréntesis).
                text: qsTr("Dictar") + "  (" + root.pttShortcut + ")"
                onTriggered: root.dictateRequested()
            }
            MenuSeparator { }
            MenuItem {
                text: qsTr("Nueva sesión")
                onTriggered: root.newSessionRequested()
            }
            Menu {
                title: qsTr("Sesiones recientes")
                MenuItem {
                    visible: root.recentSessions.length > 0
                    text: root.recentSessions.length > 0 ? "• " + root.recentSessions[0].title : ""
                    onTriggered: root.openSessionRequested(root.recentSessions[0].id)
                }
                MenuItem {
                    visible: root.recentSessions.length > 1
                    text: root.recentSessions.length > 1 ? "• " + root.recentSessions[1].title : ""
                    onTriggered: root.openSessionRequested(root.recentSessions[1].id)
                }
                MenuItem {
                    visible: root.recentSessions.length > 2
                    text: root.recentSessions.length > 2 ? "• " + root.recentSessions[2].title : ""
                    onTriggered: root.openSessionRequested(root.recentSessions[2].id)
                }
                MenuItem {
                    visible: root.recentSessions.length > 3
                    text: root.recentSessions.length > 3 ? "• " + root.recentSessions[3].title : ""
                    onTriggered: root.openSessionRequested(root.recentSessions[3].id)
                }
                MenuItem {
                    visible: root.recentSessions.length > 4
                    text: root.recentSessions.length > 4 ? "• " + root.recentSessions[4].title : ""
                    onTriggered: root.openSessionRequested(root.recentSessions[4].id)
                }
            }
            Menu {
                title: qsTr("Acciones rápidas")
                MenuItem {
                    text: qsTr("Buscar en la web…")
                    onTriggered: root.quickSearchRequested()
                }
                MenuItem {
                    text: qsTr("Abrir aplicación…")
                    onTriggered: root.quickOpenAppRequested()
                }
                MenuItem {
                    text: qsTr("Ver archivos de Documentos")
                    onTriggered: root.quickOpenFolderRequested()
                }
            }
            MenuSeparator { }
            MenuItem {
                text: qsTr("Siempre visible")
                checkable: true
                checked: root.alwaysOnTop
                onTriggered: root.alwaysOnTopToggled(!root.alwaysOnTop)
            }
            MenuItem {
                text: qsTr("Configuración")
                onTriggered: root.settingsRequested()
            }
            MenuSeparator { }
            MenuItem {
                text: qsTr("Salir")
                onTriggered: root.quitRequested()
            }
        }

        // Click en el icono: muestra/oculta la ventana
        onActivated: function(reason) {
            if (reason === SystemTrayIcon.Trigger) {
                if (root.windowVisible) root.hideRequested()
                else root.showRequested()
            }
        }
    }
}
