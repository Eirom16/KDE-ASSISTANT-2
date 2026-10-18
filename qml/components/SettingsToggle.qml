// SettingsToggle.qml - Toggle on/off estilo Apple para SettingsDialog

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0
import "."

Item {
    id: root

    property string label: ""
    property string description: ""
    property bool active: false

    signal toggled()

    // Ocupa todo el ancho cuando esta dentro de un layout
    Layout.fillWidth: true

    implicitHeight: Math.max(Theme.buttonIconSize, column.implicitHeight + Theme.spacingXs * 2)

    Row {
        id: row
        anchors.fill: parent
        anchors.leftMargin: Theme.spacingXs
        anchors.rightMargin: Theme.spacingXs
        spacing: Theme.spacingMd

        Column {
            id: column
            width: parent.width - 60
            spacing: 2
            anchors.verticalCenter: parent.verticalCenter

            Text {
                text: root.label
                font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, -0.1)
                color: Theme.ink
                wrapMode: Text.Wrap
                width: parent.width
            }
            Text {
                visible: root.description !== ""
                text: root.description
                font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
                color: Theme.inkMuted80
                wrapMode: Text.Wrap
                width: parent.width
            }
        }

        Switch {
            objectName: root.objectName ? root.objectName + "Control" : ""
            width: 48
            height: Theme.buttonIconSize
            anchors.verticalCenter: parent.verticalCenter
            checked: root.active
            Accessible.name: root.label
            onClicked: root.toggled()
            indicator: Rectangle {
                width: 48; height: 28
                y: (parent.height - height) / 2
                radius: height / 2
                color: root.active ? Theme.primary : Theme.surfaceChip
                border.width: parent.activeFocus ? 2 : 0
                border.color: Theme.ink
                Rectangle {
                    width: 24; height: 24; radius: 12; y: 2
                    x: root.active ? 22 : 2
                    color: Theme.inkOnPrimary
                }
            }
        }
    }
}
