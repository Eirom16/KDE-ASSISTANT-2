// PillButton.qml - Boton capsula compacto para uso frecuente
// Variante mas simple y pequena que AppleButton, ideal para chips y sugerencias

import QtQuick
import qml 1.0

Rectangle {
    id: root

    property string text: ""
    property string iconName: ""
    property bool active: false
    property color activeColor: Theme.primary
    signal clicked()

    property int iconSize: Theme.iconSizeSm
    property int textSize: Theme.fontSizeCaption
    property int paddingH: Theme.spacingSm + 4
    property int paddingV: Theme.spacingXs

    implicitHeight: row.implicitHeight + paddingV * 2
    implicitWidth: row.implicitWidth + paddingH * 2
    radius: Theme.radiusPill
    clip: true
    color: {
        if (active) return activeColor
        if (mouseArea.pressed) return Theme.surfacePressed
        if (mouseArea.containsMouse) return Theme.surfaceHover
        return "transparent"
    }
    border.width: active ? 0 : 1
    border.color: Theme.hairline

    Behavior on color {
        ColorAnimation { duration: Theme.animFast }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.spacingXs

        Octicon {
            visible: root.iconName !== ""
            name: root.iconName
            size: root.iconSize
            color: root.active ? Theme.inkOnPrimary : Theme.ink
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: root.text
            font: Theme.font(root.textSize, Theme.weightBold, -0.12)
            color: root.active ? Theme.inkOnPrimary : Theme.ink
            anchors.verticalCenter: parent.verticalCenter
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    scale: mouseArea.pressed ? 0.97 : 1.0
    Behavior on scale {
        NumberAnimation { duration: Theme.animFast }
    }
}
