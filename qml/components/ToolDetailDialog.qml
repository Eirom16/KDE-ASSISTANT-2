// ToolDetailDialog.qml - Detalle completo del resultado de una herramienta (F4-3)
// Se abre al pulsar un badge con resultado (sin imagen).

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0
import "."

Rectangle {
    id: root

    property bool open_: false
    property string toolName: ""
    property string resultText: ""

    signal closed()

    function show(name, result) {
        root.toolName = name || ""
        root.resultText = result || ""
        root.open_ = true
    }

    function hide() {
        root.open_ = false
        root.closed()
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
        height: Math.min(parent.height - 80, 480)
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
                    text: root.toolName !== "" ? root.toolName : qsTr("Acción")
                    font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, -0.1)
                    color: Theme.ink
                    Layout.fillWidth: true
                    elide: Text.ElideRight
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

            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                TextArea {
                    width: parent.width
                    text: root.resultText
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.WrapAnywhere
                    color: Theme.ink
                    font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                    background: null
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.spacingXs

                PillButton {
                    text: qsTr("Cerrar")
                    onClicked: root.hide()
                }
                Item { Layout.fillWidth: true }
                AppleButton {
                    text: qsTr("Copiar")
                    iconName: "copy-16"
                    variant: "primary"
                    onClicked: {
                        detailBuffer.text = root.resultText
                        detailBuffer.selectAll()
                        detailBuffer.copy()
                        detailBuffer.deselect()
                    }
                }
            }
        }
    }

    TextEdit {
        id: detailBuffer
        visible: false
        width: 0
        height: 0
    }
}
