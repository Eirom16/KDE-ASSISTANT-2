// IconButton.qml - Boton circular con icono estilo Apple
// 44x44 por defecto, chip translucent, con animacion press y estados

import QtQuick
import qml 1.0

Rectangle {
    id: root

    property string iconName: ""
    property int iconSize: Theme.iconSizeMd
    property int buttonSize: Theme.buttonIconSize
    property color iconColor: Theme.ink
    property color backgroundColor: Theme.surfaceChip
    property bool active: false       // Para estado "grabando" (rojo)
    property color activeColor: Theme.error
    signal clicked()

    width: buttonSize
    height: buttonSize
    radius: width / 2
    color: {
        if (active) return activeColor
        if (mouseArea.pressed) return Qt.darker(backgroundColor, 1.15)
        if (mouseArea.containsMouse) return backgroundColor === Theme.surfaceChip
                                       ? Theme.surfaceChipHover
                                       : Qt.lighter(backgroundColor, 1.1)
        return backgroundColor
    }

    Behavior on color {
        ColorAnimation { duration: Theme.animFast }
    }

    Octicon {
        anchors.centerIn: parent
        name: root.iconName
        size: root.iconSize
        color: root.active ? Theme.inkOnPrimary : root.iconColor
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    // Pulsing ring cuando esta active
    Rectangle {
        anchors.fill: parent
        visible: root.active
        radius: width / 2
        color: "transparent"
        border.width: 2
        border.color: root.activeColor
        opacity: pulseAnim.opacity

        SequentialAnimation on opacity {
            id: pulseAnim
            loops: Animation.Infinite
            running: root.active
            NumberAnimation { from: 0.8; to: 0.0; duration: 1200 }
            NumberAnimation { from: 0.0; to: 0.8; duration: 1 }
        }

        scale: pulseAnim.opacity * 0.3 + 1.0
    }

    scale: mouseArea.pressed ? 0.92 : 1.0
    Behavior on scale {
        NumberAnimation { duration: Theme.animFast }
    }
}
