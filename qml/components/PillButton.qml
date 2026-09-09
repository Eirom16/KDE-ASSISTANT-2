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
    // Foco por teclado (Fase 1 a11y minima): anillo primary.
    focusPolicy: Qt.StrongFocus
    Accessible.role: Accessible.Button
    Accessible.name: root.text
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()
    Keys.onSpacePressed: root.clicked()
    color: {
        if (active) return activeColor
        if (mouseArea.pressed) return Theme.surfacePressed
        if (mouseArea.containsMouse) return Theme.surfaceHover
        return "transparent"
    }
    border.width: active || activeFocus ? 0 : 1
    border.color: activeFocus ? Theme.primary : Theme.hairline

    Behavior on color {
        ColorAnimation { duration: Theme.animFast }
    }

    Row {
        id: row
        anchors.centerIn: parent
        // No desbordar al padre (drawer 220px): recorta el texto con elide.
        width: Math.min(implicitWidth, parent.width - paddingH * 2)
        clip: true
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
            width: Math.min(implicitWidth, row.width - (root.iconName !== "" ? root.iconSize + Theme.spacingXs : 0))
            elide: Text.ElideRight
            maximumLineCount: 1
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
