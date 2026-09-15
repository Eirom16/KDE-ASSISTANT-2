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
    /// Etiqueta que anuncia el control al lector de pantalla. Los llamadores
    /// deben dar una frase humana; el icono queda como respaldo.
    property string accessibleName: iconName
    signal clicked()

    focus: true
    activeFocusOnTab: true
    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
    Accessible.description: root.active ? qsTr("Activo") : ""

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

    Keys.onReturnPressed: function(event) {
        if (root.enabled !== false) root.clicked()
        event.accepted = true
    }
    Keys.onSpacePressed: function(event) {
        if (root.enabled !== false) root.clicked()
        event.accepted = true
    }

    // Pulsing ring cuando esta active
    Rectangle {
        anchors.fill: parent
        visible: root.active
        radius: width / 2
        color: "transparent"
        border.width: 2
        border.color: root.activeColor

        SequentialAnimation on opacity {
            id: pulseAnim
            loops: Animation.Infinite
            running: root.active
            NumberAnimation { from: 0.8; to: 0.0; duration: 1200 }
            NumberAnimation { from: 0.0; to: 0.8; duration: 1 }
        }

        SequentialAnimation on scale {
            loops: Animation.Infinite
            running: root.active
            NumberAnimation { from: 1.0; to: 1.3; duration: 1200; easing.type: Easing.OutCubic }
            NumberAnimation { from: 1.3; to: 1.0; duration: 1 }
        }
    }

    scale: mouseArea.pressed ? 0.92 : 1.0
    Behavior on scale {
        NumberAnimation { duration: Theme.animFast }
    }
}
