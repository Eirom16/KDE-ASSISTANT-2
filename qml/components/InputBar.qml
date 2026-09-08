// InputBar.qml - Floating capsule bar estilo Apple
// Auto-resize hasta 4 lineas, hint text, botones circulares

import QtQuick
import QtQuick.Controls
import qml 1.0

Item {
    id: root

    // === Props ===
    property alias text: input.text
    property bool recording: false
    property bool voiceEnabled: true
    property bool canSend: text.trim().length > 0 && !streaming

    property bool streaming: false   // Mientras el LLM esta respondiendo, no se puede enviar

    signal sendClicked(string text)
    signal micClicked()
    signal stopClicked()

    implicitHeight: 64
    implicitWidth: 480

    // === Floating capsule bar ===
    Rectangle {
        id: bar
        anchors.fill: parent
        anchors.margins: Theme.spacingXs
        radius: Theme.radiusPill
        color: Qt.rgba(
            Theme.surface.r,
            Theme.surface.g,
            Theme.surface.b,
            0.95
        )
        border.width: 1
        border.color: input.activeFocus ? Theme.primary : Theme.hairline

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }

        // Sombra elevada
        Rectangle {
            anchors.fill: parent
            anchors.margins: -2
            z: -1
            color: Theme.shadowSoft
            radius: parent.radius + 2
            opacity: 0.5
        }

        Row {
            anchors.fill: parent
            anchors.leftMargin: 4
            anchors.rightMargin: 4
            spacing: 0

            // === TextInput (expansible) ===
            Item {
                width: bar.width - 44 * 2 - 16
                height: bar.height - 8
                anchors.verticalCenter: parent.verticalCenter

                ScrollView {
                    id: scroll
                    anchors.fill: parent
                    anchors.margins: 4
                    clip: true

                    TextArea {
                        id: input
                        placeholderText: qsTr("Escribe un mensaje...")
                        placeholderTextColor: Theme.inkMuted
                        color: Theme.ink
                        font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                        wrapMode: TextEdit.Wrap
                        background: null
                        selectByMouse: true
                        leftPadding: Theme.spacingSm
                        rightPadding: Theme.spacingXs
                        topPadding: Theme.spacingXs
                        bottomPadding: Theme.spacingXs
                        // Auto-resize: max ~4 lineas
                        onTextChanged: {
                            var lines = (text.match(/\n/g) || []).length + 1
                            var maxLines = 4
                            // Heuristica simple: dejamos al control crecer
                        }
                        // Enter envia (el TextArea multilinea lo consumiria si no)
                        Keys.onReturnPressed: function(event) {
                            if (event.modifiers & Qt.ShiftModifier) {
                                event.accepted = false  // Shift+Enter: salto de linea
                            } else {
                                if (root.streaming) {
                                    root.stopClicked()
                                } else if (root.canSend) {
                                    root.sendClicked(input.text)
                                    input.text = ""
                                }
                                event.accepted = true
                            }
                        }
                        Keys.onEnterPressed: function(event) {
                            if (event.modifiers & Qt.ShiftModifier) {
                                event.accepted = false
                            } else {
                                if (root.streaming) {
                                    root.stopClicked()
                                } else if (root.canSend) {
                                    root.sendClicked(input.text)
                                    input.text = ""
                                }
                                event.accepted = true
                            }
                        }
                    }
                }
            }

            // === Boton microfono (circular) ===
            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: root.recording ? "stop-16" : "unmute-16"
                iconSize: Theme.iconSizeMd
                buttonSize: 40
                backgroundColor: Theme.surfaceChip
                iconColor: root.recording ? Theme.inkOnPrimary : Theme.ink
                active: root.recording
                activeColor: Theme.error
                onClicked: {
                    if (root.recording) root.stopClicked()
                    else root.micClicked()
                }
            }

            // === Boton enviar (circular) ===
            IconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: root.streaming ? "stop-16" : "paper-airplane-16"
                iconSize: Theme.iconSizeMd
                buttonSize: 40
                backgroundColor: root.canSend || root.streaming ? Theme.primary : Theme.surfaceChip
                iconColor: root.canSend || root.streaming ? Theme.inkOnPrimary : Theme.inkMuted
                active: root.streaming
                activeColor: Theme.error
                onClicked: {
                    if (root.streaming) {
                        root.stopClicked()
                    } else if (root.canSend) {
                        root.sendClicked(input.text)
                        input.text = ""
                    }
                }
            }
        }
    }

    // === Hint text (debajo) ===
    Text {
        anchors.top: bar.bottom
        anchors.topMargin: 4
        anchors.horizontalCenter: parent.horizontalCenter
        text: qsTr("Enter para enviar · Shift+Enter nueva linea")
        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0.2)
        color: Theme.inkMuted
    }

    // === Keys ===
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            if (event.modifiers & Qt.ShiftModifier) {
                // Shift+Enter: nueva linea (default TextArea)
            } else {
                if (root.streaming) {
                    root.stopClicked()
                } else if (root.canSend) {
                    root.sendClicked(input.text)
                    input.text = ""
                }
                event.accepted = true
            }
        }
    }
}
