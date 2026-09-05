// Octicon.qml - Componente reutilizable para iconos de GitHub Primer Octicons
// Los SVGs estan en assets/octicons/ y se embeben en resources.qrc
// Uso: Octicon { name: "rocket-16"; size: 20; color: Theme.primary }

import QtQuick
import QtQuick.Shapes
import Qt5Compat.GraphicalEffects   // ColorOverlay para tintar SVG

Item {
    id: root

    // Nombre del icono (sin extension), ej: "rocket-16"
    property string name: "hubot-16"

    // Tamano del icono (cuadrado)
    property int size: 16

    // Color de relleno (heredar de Theme)
    property color color: Theme.ink

    // Slot del icono
    property string slot: "default"  // "default", "primary", "muted", "success", "error"

    width: size
    height: size

    // Path a los SVGs (filesystem; en produccion se embeben via resources.qrc)
    readonly property string iconPath: Qt.resolvedUrl("../assets/octicons/" + name + ".svg")

    // Carga del SVG via Image (Qt SVG renderer)
    Image {
        id: iconImage
        anchors.fill: parent
        source: root.iconPath
        sourceSize: Qt.size(root.size, root.size)
        fillMode: Image.PreserveAspectFit
        smooth: true
        antialiasing: true
        visible: status === Image.Ready

        // Tintar SVG con color (Qt 6.5+)
        layer.enabled: true
        layer.effect: ColorOverlay {
            color: root.color
        }
    }

    // Fallback: circulo si el icono no carga
    Rectangle {
        anchors.centerIn: parent
        width: root.size * 0.6
        height: width
        radius: width / 2
        color: root.color
        opacity: 0.3
        visible: iconImage.status !== Image.Ready
    }
}
