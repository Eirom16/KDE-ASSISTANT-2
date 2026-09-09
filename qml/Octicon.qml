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

    // Path a los SVGs: filesystem en dev; qrc:/ en instalado.
    // Si el filesystem falla, se reintenta con qrc (una vez).
    property bool triedQrc: false
    readonly property string fsPath: root.name !== ""
        ? Qt.resolvedUrl("../assets/octicons/" + name + ".svg")
        : ""
    readonly property string qrcPath: root.name !== ""
        ? "qrc:/octicons/" + name + ".svg"
        : ""
    readonly property string iconPath: fsPath

    // Carga del SVG via Image (Qt SVG renderer)
    Image {
        id: iconImage
        anchors.fill: parent
        source: root.iconPath
        sourceSize: Qt.size(root.size, root.size)
        fillMode: Image.PreserveAspectFit
        smooth: true
        antialiasing: true
        visible: root.name !== "" && status === Image.Ready
        onStatusChanged: {
            // F0-8: fallback a qrc:/ si el filesystem no está (app instalada).
            if (status === Image.Error && !root.triedQrc && root.qrcPath !== "") {
                root.triedQrc = true
                iconImage.source = root.qrcPath
            }
            if (status === Image.Error) {
                console.warn("Octicon no encontrado: " + root.name)
            }
        }

        // Tintar SVG con color (Qt 6.5+)
        layer.enabled: true
        layer.effect: ColorOverlay {
            color: root.color
        }
    }

    // Fallback: círculo tenue solo si falló la carga (no durante Loading).
    Rectangle {
        anchors.centerIn: parent
        width: root.size * 0.6
        height: width
        radius: width / 2
        color: root.color
        opacity: 0.3
        visible: root.name !== "" && iconImage.status === Image.Error
    }
}
