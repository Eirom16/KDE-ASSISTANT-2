// InputBar.qml - Floating capsule bar estilo Apple
// Auto-resize hasta 4 lineas, hint text, botones circulares

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Qt.labs.platform as Labs
import qml 1.0

Item {
    id: root

    // === Props ===
    property alias text: input.text
    property bool recording: false
    property bool voiceEnabled: true
    property bool canSend: text.trim().length > 0 && !streaming

    property bool streaming: false   // Mientras el LLM esta respondiendo, no se puede enviar

    // Auto-crecimiento: hasta 4 lineas, luego scroll interno
    property int maxInputLines: 4
    property real lineH: input.contentHeight > 0 ? input.contentHeight / Math.max(1, input.lineCount) : 22
    property real maxTextH: 22 * 4

    signal sendClicked(string text)
    signal micClicked()
    signal stopClicked()
    signal fileSelected(string filePath, string fileType)  // fileType: "text" | "image"

    function focusInput() {
        input.forceActiveFocus()
    }

    // Hook de prueba: abre el menú adjuntar (usado por el harness visual).
    function debugOpenAttachMenu() {
        attachMenu.open()
    }

    implicitWidth: 480
    // Altura barre + hint; la barre crece con el contenido
    implicitHeight: bar.implicitHeight + hintText.implicitHeight + 8

    // === Floating capsule bar ===
    Rectangle {
        id: bar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: Theme.spacingXs
        anchors.rightMargin: Theme.spacingXs
        anchors.topMargin: Theme.spacingXs
        implicitHeight: inputBox.implicitHeight + 24
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

        RowLayout {
            id: inputBox
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.margins: 6
            anchors.leftMargin: 10
            spacing: 6

            // === TextInput (auto-creciente hasta maxTextH) ===
            ScrollView {
                id: scroll
                Layout.fillWidth: true
                // Altura del scroll incluye padding del TextArea
                Layout.preferredHeight: Math.min(input.implicitHeight, root.maxTextH + input.topPadding + input.bottomPadding)
                Layout.alignment: Qt.AlignVCenter
                clip: true
                ScrollBar.vertical.policy: input.implicitHeight > root.maxTextH + input.topPadding + input.bottomPadding ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                TextArea {
                    id: input
                    placeholderText: qsTr("Escribe un mensaje...")
                    placeholderTextColor: Theme.inkMuted
                    color: Theme.ink
                    font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                    wrapMode: TextEdit.Wrap
                    background: null
                    selectByMouse: true
                    verticalAlignment: TextEdit.AlignVCenter
                    leftPadding: Theme.spacingSm
                    rightPadding: Theme.spacingXs
                    topPadding: 6
                    bottomPadding: 6
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

            // === Boton adjuntar archivo (popup custom estilo Apple) ===
            IconButton {
                id: attachButton
                Layout.alignment: Qt.AlignVCenter
                iconName: "upload-16"
                iconSize: Theme.iconSizeMd
                buttonSize: Theme.buttonIconSize
                backgroundColor: Theme.surfaceChip
                iconColor: Theme.ink
                opacity: root.streaming ? 0.4 : 1.0
                enabled: !root.streaming
                onClicked: {
                    if (!root.streaming) {
                        attachMenu.open()
                    }
                }
            }

            // === Boton microfono (circular) ===
            // Durante streaming se deshabilita: solo el boton enviar hace stop.
            IconButton {
                Layout.alignment: Qt.AlignVCenter
                iconName: root.recording ? "stop-16" : "microphone-16"
                iconSize: Theme.iconSizeMd
                buttonSize: Theme.buttonIconSize
                backgroundColor: Theme.surfaceChip
                iconColor: root.recording ? Theme.inkOnPrimary : Theme.ink
                active: root.recording && !root.streaming
                activeColor: Theme.error
                opacity: root.streaming ? 0.4 : 1.0
                enabled: !root.streaming
                onClicked: {
                    // Toggle: el padre (Main) decide si empezar o parar el dictado.
                    root.micClicked()
                }
            }

            // === Boton enviar (circular) ===
            IconButton {
                Layout.alignment: Qt.AlignVCenter
                iconName: root.streaming ? "stop-16" : "paper-airplane-16"
                iconSize: Theme.iconSizeMd
                buttonSize: Theme.buttonIconSize
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

    // === Menu adjuntar (popup propio con tokens Theme, no Menu nativo) ===
    // Parentado al boton: se posiciona relativo a el, justo encima.
    Popup {
        id: attachMenu
        parent: attachButton
        y: -implicitHeight - 8
        x: attachButton.width / 2 - implicitWidth / 2
        implicitWidth: 220
        implicitHeight: attachCol.implicitHeight + topPadding + bottomPadding
        padding: 6
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        background: Rectangle {
            radius: Theme.radiusMd
            color: Theme.surface
            border.width: 1
            border.color: Theme.hairline

            // Reusable shadow token, igual que la barra
            Rectangle {
                anchors.fill: parent
                anchors.margins: -2
                z: -1
                radius: parent.radius + 2
                color: Theme.shadowSoft
                opacity: 0.5
            }
        }

        contentItem: Column {
            id: attachCol
            spacing: 2

            Repeater {
                model: [
                    { icon: "file-16", label: qsTr("Subir archivo de texto") },
                    { icon: "image-16", label: qsTr("Subir imagen") }
                ]
                delegate: Rectangle {
                    id: attachRow
                    required property var modelData
                    width: attachCol.width
                    height: 36
                    radius: Theme.radiusSm
                    color: rowHover.containsMouse ? Theme.surfaceHover : "transparent"

                    Behavior on color {
                        ColorAnimation { duration: Theme.animFast }
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 10

                        Octicon {
                            name: attachRow.modelData.icon
                            size: Theme.iconSizeSm
                            color: Theme.ink
                            Layout.alignment: Qt.AlignVCenter
                        }
                        Text {
                            text: attachRow.modelData.label
                            font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
                            color: Theme.ink
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignVCenter
                            elide: Text.ElideRight
                            maximumLineCount: 1
                        }
                    }

                    MouseArea {
                        id: rowHover
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            attachMenu.close()
                            fileDialog.open()
                        }
                    }
                }
            }
        }
    }

    // === Hint text (debajo) ===
    Text {
        id: hintText
        anchors.top: bar.bottom
        anchors.topMargin: 6
        anchors.horizontalCenter: parent.horizontalCenter
        // Envuelve a 360px en vez de elidirse: dos lineas maximo.
        width: Math.min(implicitWidth, parent.width - 16)
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        maximumLineCount: 2
        elide: Text.ElideRight
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

    // === File dialog (Qt.labs.platform FileDialog, API Qt 6.11) ===
    // Alias `Labs`: el import sin alias hacía que `Menu` resolviera a
    // labs.platform (sin popup()) y rompía el menú de adjuntar (QML TypeError).
    // Props: fileMode, file, files, currentFile(s), folder, options, nameFilters,
    // selectedNameFilter, defaultSuffix, acceptLabel, rejectLabel.
    Labs.FileDialog {
        id: fileDialog
        title: qsTr("Seleccionar archivo")
        fileMode: Labs.FileDialog.OpenFile
        nameFilters: ["Archivos de texto (*.txt *.md *.json *.js *.ts *.py *.rs *.html *.css)",
                      "Imagenes (*.png *.jpg *.jpeg *.webp *.gif *.bmp)",
                      "Todos los archivos (*)"]
        onAccepted: {
            if (file.toString().length > 0) {
                var path = file.toString()
                if (path.startsWith("file://")) path = path.substring(7)
                var isImage = /\.(png|jpe?g|webp|gif|bmp)$/i.test(path)
                root.fileSelected(path, isImage ? "image" : "text")
            }
        }
        onRejected: console.log("File dialog cancelled")
    }
}
