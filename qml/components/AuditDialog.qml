// AuditDialog.qml - Historial de herramientas ejecutadas (F4-3)
// Lee GET /api/tools/audit y muestra qué hizo el asistente, cuándo y con qué resultado.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0
import qml.auth 1.0
import "."

Rectangle {
    id: root

    property bool open_: false
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

    property var entries: []

    signal closed()

    function show() {
        root.open_ = true
        root.load()
    }

    function hide() {
        root.open_ = false
        root.closed()
    }

    function fmtTs(iso) {
        if (!iso) return ""
        try {
            var d = new Date(iso)
            if (isNaN(d.getTime())) return ""
            var dd = ("0" + d.getDate()).slice(-2)
            var mm = ("0" + (d.getMonth() + 1)).slice(-2)
            return dd + "/" + mm + " " + Qt.formatTime(d, "hh:mm")
        } catch (e) { return "" }
    }

    function load() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/tools/audit?limit=50")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    root.entries = JSON.parse(xhr.responseText)
                } catch (e) { root.entries = [] }
            }
        }
        xhr.send()
    }

    visible: open_
    color: Theme.overlayBackdrop
    z: 999

    MouseArea {
        anchors.fill: parent
        onClicked: root.hide()
    }

    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 520)
        height: Math.min(parent.height - 80, 520)
        radius: Theme.radiusLg + 2
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacingLg
            spacing: Theme.spacingMd

            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.spacingXs

                Text {
                    text: qsTr("Historial de acciones")
                    font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, -0.1)
                    color: Theme.ink
                    Layout.fillWidth: true
                }

                IconButton {
                    iconName: "x-16"
                    iconSize: 18
                    buttonSize: 32
                    backgroundColor: "transparent"
                    iconColor: Theme.ink
                    onClicked: root.hide()
                }
            }

            Text {
                visible: root.entries.length === 0
                text: qsTr("Aún no se ha ejecutado ninguna herramienta.")
                font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                color: Theme.inkMuted
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: root.entries.length > 0
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                Column {
                    width: parent.width
                    spacing: Theme.spacingXs

                    Repeater {
                        model: root.entries
                        delegate: Item {
                            required property var modelData
                            width: parent.width
                            height: 44

                            RowLayout {
                                anchors.fill: parent
                                spacing: Theme.spacingXs

                                // Punto de permiso: verde/amarillo/rojo
                                Rectangle {
                                    Layout.preferredWidth: 8
                                    Layout.preferredHeight: 8
                                    Layout.alignment: Qt.AlignVCenter
                                    radius: 4
                                    color: {
                                        if (modelData.permission === "red") return Theme.error
                                        if (modelData.permission === "yellow") return Theme.warn
                                        return Theme.success
                                    }
                                }

                                Column {
                                    Layout.fillWidth: true
                                    spacing: 0

                                    Text {
                                        text: (modelData.tool || "tool") + (modelData.success ? "" : qsTr(" (falló)"))
                                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, -0.05)
                                        color: modelData.success ? Theme.ink : Theme.error
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                    Text {
                                        text: root.fmtTs(modelData.timestamp) + "  ·  " + (modelData.decided || "auto") + "  ·  " + (modelData.duration_ms || 0) + "ms"
                                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                                        color: Theme.inkMuted
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }
                                }
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                PillButton {
                    text: qsTr("Cerrar")
                    onClicked: root.hide()
                }
            }
        }
    }
}
