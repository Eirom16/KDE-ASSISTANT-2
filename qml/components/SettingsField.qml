// SettingsField.qml - Campo de input con label para SettingsDialog
// Wrapper sobre TextField con estilo Apple

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0
import "."

Column {
    id: root

    property string label: ""
    property alias value: input.text
    property string placeholder: ""
    property bool isPassword: false

    // Ocupa todo el ancho cuando esta dentro de un layout
    // (sin width explicito para no pelear con el layout)
    Layout.fillWidth: true

    spacing: Theme.spacingXs

    Text {
        text: root.label
        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
        color: Theme.inkMuted
    }

    Rectangle {
        width: parent.width
        height: 36
        radius: Theme.radiusMd
        color: input.activeFocus ? Theme.surface : Theme.surfacePearl
        border.width: 1
        border.color: input.activeFocus ? Theme.primary : Theme.hairline

        Behavior on border.color {
            ColorAnimation { duration: Theme.animFast }
        }

        TextField {
            id: input
            anchors.fill: parent
            anchors.leftMargin: Theme.spacingMd
            anchors.rightMargin: Theme.spacingMd
            verticalAlignment: TextInput.AlignVCenter
            placeholderText: root.placeholder
            placeholderTextColor: Theme.inkMuted
            color: Theme.ink
            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
            echoMode: root.isPassword ? TextInput.Password : TextInput.Normal
            background: null
            selectByMouse: true
        }
    }
}
