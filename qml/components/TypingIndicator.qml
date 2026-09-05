// TypingIndicator.qml - Tres puntos pulsantes estilo Apple
// Timing escalonado: 0ms, 150ms, 300ms. Ciclo 1200ms total.

import QtQuick
import qml 1.0

Item {
    id: root

    property color color: Theme.inkMuted
    property int dotSize: 8
    property int spacing: 6
    property bool active: true

    implicitWidth: dotSize * 3 + spacing * 2
    implicitHeight: dotSize

    Row {
        anchors.centerIn: parent
        spacing: root.spacing

        Repeater {
            model: 3
            delegate: Rectangle {
                required property int index
                width: root.dotSize
                height: width
                radius: width / 2
                color: root.color
                opacity: active ? opacity1.value : 0.0

                SequentialAnimation on opacity {
                    id: opacity1
                    running: root.active
                    loops: Animation.Infinite
                    PauseAnimation { duration: index * 150 }
                    NumberAnimation { from: 0.3; to: 1.0; duration: 400; easing.type: Easing.InOutQuad }
                    NumberAnimation { from: 1.0; to: 0.3; duration: 400; easing.type: Easing.InOutQuad }
                }
            }
        }
    }
}
