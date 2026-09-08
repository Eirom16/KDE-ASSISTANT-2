// SettingsToggle.qml - Toggle on/off estilo Apple para SettingsDialog

import QtQuick
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

    implicitHeight: column.implicitHeight + Theme.spacingXs * 2

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
            }
            Text {
                visible: root.description !== ""
                text: root.description
                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                color: Theme.inkMuted
                wrapMode: Text.Wrap
                width: parent.width
            }
        }

        // Toggle switch
        Item {
            width: 48
            height: 28
            anchors.verticalCenter: parent.verticalCenter

            Rectangle {
                anchors.fill: parent
                radius: height / 2
                color: root.active ? Theme.success : Theme.surfaceChip

                Behavior on color {
                    ColorAnimation { duration: Theme.animFast }
                }

                Rectangle {
                    width: parent.height - 4
                    height: width
                    radius: width / 2
                    color: "white"
                    x: root.active ? parent.width - width - 2 : 2
                    y: 2

                    Behavior on x {
                        NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
                    }

                    // Sombra
                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: -2
                        z: -1
                        color: Theme.shadowSoft
                        opacity: 0.3
                        radius: parent.radius + 2
                    }
                }
            }

            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: root.toggled()
            }
        }
    }
}
