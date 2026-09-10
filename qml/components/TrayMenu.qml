// TrayMenu.qml - System Tray para KDE Plasma + popup propio posicionado.
// El Menu nativo de Qt.labs.platform se abria en la esquina superior
// izquierda bajo XWayland (bug de posicionamiento del popup). En su lugar,
// click derecho abre un popup QML colocado junto al icono (tray.geometry),
// que arranca cerrado y solo se abre a peticion.

import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import Qt.labs.platform
import qml 1.0
import qml.auth 1.0

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
            if (AuthToken.token) return String(AuthToken.token)
        } catch (e) {}
        try {
            var env = Qt.platform.environment
            var t = env ? env["KDE_ASSISTANT_TOKEN"] : null
            return t ? String(t) : ""
        } catch (err) { return "" }
    }
    function setAuth(xhr) {
        if (authToken !== "") xhr.setRequestHeader("Authorization", "Bearer " + authToken)
    }

    // Filas del menu (se reconstruyen al abrir).
    property var rows: []
    // true cuando el popup esta visible (util para tests).
    property bool menuOpen: menuWindow.visible

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

    // Reconstruye las filas y calcula la altura exacta
    // (filas 36px + separadores 9px + padding 16px).
    // Todas las filas llevan todas las claves (sin undefined).
    function newRow(o) {
        return {
            label: o.label || "", icon: o.icon || "", hint: o.hint || "",
            act: o.act || "", arg: o.arg || "",
            sep: o.sep === true,
            checkable: o.checkable === true, checked: o.checked === true
        }
    }
    function rebuildRows() {
        var r = []
        r.push(newRow({ label: root.windowVisible ? qsTr("Ocultar ventana") : qsTr("Mostrar ventana"), icon: "eye-16", act: "toggle" }))
        r.push(newRow({ label: qsTr("Dictar"), icon: "unmute-16", hint: root.pttShortcut, act: "dictate" }))
        r.push(newRow({ sep: true }))
        r.push(newRow({ label: qsTr("Nueva sesión"), icon: "plus-16", act: "new" }))
        for (var i = 0; i < Math.min(root.recentSessions.length, 5); i++) {
            var s = root.recentSessions[i] || {}
            r.push(newRow({ label: "•  " + (s.title || qsTr("Sin título")), act: "open", arg: s.id }))
        }
        r.push(newRow({ sep: true }))
        r.push(newRow({ label: qsTr("Buscar en la web…"), icon: "search-16", act: "search" }))
        r.push(newRow({ label: qsTr("Abrir aplicación…"), icon: "rocket-16", act: "app" }))
        r.push(newRow({ label: qsTr("Ver archivos de Documentos"), icon: "file-16", act: "folder" }))
        r.push(newRow({ sep: true }))
        r.push(newRow({ label: qsTr("Siempre visible"), act: "pin", checkable: true, checked: root.alwaysOnTop }))
        r.push(newRow({ label: qsTr("Configuración"), icon: "gear-16", act: "settings" }))
        r.push(newRow({ sep: true }))
        r.push(newRow({ label: qsTr("Salir"), icon: "sign-out-16", act: "quit" }))
        root.rows = r
        var h = 16
        for (var j = 0; j < r.length; j++) h += r[j].sep ? 9 : 36
        menuWindow.height = h
    }

    function handleRow(act, arg) {
        menuWindow.visible = false
        if (act === "toggle") {
            if (root.windowVisible) root.hideRequested()
            else root.showRequested()
        } else if (act === "dictate") {
            root.dictateRequested()
        } else if (act === "new") {
            root.newSessionRequested()
        } else if (act === "open") {
            root.openSessionRequested(arg)
        } else if (act === "search") {
            root.quickSearchRequested()
        } else if (act === "app") {
            root.quickOpenAppRequested()
        } else if (act === "folder") {
            root.quickOpenFolderRequested()
        } else if (act === "pin") {
            root.alwaysOnTopToggled(!root.alwaysOnTop)
        } else if (act === "settings") {
            root.settingsRequested()
        } else if (act === "quit") {
            root.quitRequested()
        }
    }

    // Abre el popup junto al icono del tray (o esquina inferior
    // derecha si el sistema no da geometria). Arranca cerrado.
    function openMenu() {
        rebuildRows()
        var g = tray.geometry
        var mw = menuWindow.width
        var mh = menuWindow.height
        var scrW = Screen.desktopAvailableWidth
        var scrH = Screen.desktopAvailableHeight
        var px, py
        if (g && g.width > 0 && scrW > 0) {
            px = g.x + g.width / 2 - mw / 2
            py = g.y - mh - 10
            if (py < 8) py = g.y + g.height + 10
        } else {
            px = scrW - mw - 12
            py = scrH - mh - 48
        }
        px = Math.max(8, Math.min(px, scrW - mw - 8))
        py = Math.max(8, Math.min(py, scrH - mh - 8))
        menuWindow.x = Math.round(px)
        menuWindow.y = Math.round(py)
        menuWindow.wasActive = false
        menuWindow.visible = true
        menuWindow.requestActivate()
    }

    SystemTrayIcon {
        id: tray
        visible: true
        tooltip: qsTr("KDE Assistant v2")

        // Icono: nombre del tema hicolor con fallback al SVG del repo.
        icon.name: "kde-assistant"
        icon.source: Qt.resolvedUrl("../../assets/icons/kde-assistant.svg")
        icon.mask: false

        // Click izquierdo: muestra/oculta. Click derecho: popup propio
        // junto al icono (el Menu nativo se iba a la esquina superior).
        onActivated: function(reason) {
            if (reason === SystemTrayIcon.Trigger) {
                if (root.windowVisible) root.hideRequested()
                else root.showRequested()
            } else if (reason === SystemTrayIcon.Context) {
                root.openMenu()
            } else if (reason === SystemTrayIcon.DoubleClick) {
                root.showRequested()
            }
        }
    }

    // === Popup propio (Tool sin marco, siempre arriba) ===
    Window {
        id: menuWindow
        width: 280
        height: 360
        minimumWidth: 280
        maximumWidth: 280
        flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
        color: "transparent"
        visible: false

        // Cierre al perder foco (click fuera) o Escape.
        property bool wasActive: false
        onActiveChanged: {
            if (active) wasActive = true
            else if (wasActive) {
                wasActive = false
                visible = false
            }
        }
        onVisibleChanged: {
            if (visible) keyGrabber.forceActiveFocus()
        }

        // Captura Escape (Keys no se puede anclar a Window).
        Item {
            id: keyGrabber
            anchors.fill: parent
            focus: true
            Keys.onEscapePressed: menuWindow.visible = false
        }

        Rectangle {
            anchors.fill: parent
            radius: Theme.radiusMd
            color: Theme.surface
            border.width: 1
            border.color: Theme.hairline

            Column {
                id: menuCol
                anchors.fill: parent
                anchors.margins: 8
                spacing: 0

                Repeater {
                    model: root.rows
                    delegate: Item {
                        required property var modelData
                        width: menuCol.width
                        height: modelData.sep ? 9 : 36

                        // Separador
                        Rectangle {
                            visible: modelData.sep
                            anchors.centerIn: parent
                            width: parent.width - 16
                            height: 1
                            color: Theme.hairline
                        }

                        // Fila clicable
                        Rectangle {
                            visible: !modelData.sep
                            anchors.fill: parent
                            radius: Theme.radiusSm
                            color: rowMouse.containsMouse ? Theme.surfaceHover : "transparent"

                            Behavior on color {
                                ColorAnimation { duration: Theme.animFast }
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 10
                                anchors.rightMargin: 10
                                spacing: 10

                                Octicon {
                                    visible: modelData.icon !== ""
                                    name: modelData.icon
                                    size: Theme.iconSizeSm
                                    color: Theme.ink
                                    Layout.alignment: Qt.AlignVCenter
                                }
                                Text {
                                    text: modelData.label
                                    font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
                                    color: Theme.ink
                                    Layout.fillWidth: true
                                    Layout.alignment: Qt.AlignVCenter
                                    elide: Text.ElideRight
                                    maximumLineCount: 1
                                }
                                Text {
                                    visible: modelData.hint !== ""
                                    text: modelData.hint
                                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                    color: Theme.inkMuted
                                    Layout.alignment: Qt.AlignVCenter
                                }
                                Rectangle {
                                    visible: modelData.checkable
                                    Layout.preferredWidth: 18
                                    Layout.preferredHeight: 18
                                    Layout.alignment: Qt.AlignVCenter
                                    radius: 9
                                    color: modelData.checked ? Theme.success : "transparent"
                                    border.width: modelData.checked ? 0 : 1
                                    border.color: Theme.inkMuted
                                    Octicon {
                                        visible: modelData.checked
                                        anchors.centerIn: parent
                                        name: "check-16"
                                        size: 11
                                        color: Theme.inkOnPrimary
                                    }
                                }
                            }

                            MouseArea {
                                id: rowMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.handleRow(modelData.act, modelData.arg)
                            }
                        }
                    }
                }
            }
        }
    }
}
