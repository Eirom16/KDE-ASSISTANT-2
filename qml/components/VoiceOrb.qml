// VoiceOrb.qml - Orbe luminoso estilo Siri
// Estados: idle | listening | processing | speaking
// Animacion spring reactiva a amplitud del microfono

import QtQuick
import qml 1.0

Item {
    id: root

    // === Props ===
    property string state: "idle"        // "idle" | "listening" | "processing" | "speaking"
    property real amplitude: 0.0          // 0.0 a 1.0 (RMS del microfono)
    property int size: Theme.voiceOrbSize

    signal clicked()

    width: size
    height: size

    // F0-6: pulso para speaking (Date.now() en binding no es reactivo).
    property real speakPulse: 0.0
    SequentialAnimation on speakPulse {
        running: root.state === "speaking"
        loops: Animation.Infinite
        NumberAnimation { from: 0.0; to: 1.0; duration: 450; easing.type: Easing.InOutSine }
        NumberAnimation { from: 1.0; to: 0.0; duration: 450; easing.type: Easing.InOutSine }
    }

    // === Capas de ondas expansivas (estados activos) ===
    Repeater {
        model: state === "listening" || state === "speaking" ? 3 : 0
        delegate: Rectangle {
            required property int index
            anchors.centerIn: parent
            width: root.size
            height: width
            radius: width / 2
            color: "transparent"
            border.width: 1.5
            border.color: Theme.orbStart
            opacity: 0.4

            SequentialAnimation on scale {
                running: root.state === "listening" || root.state === "speaking"
                loops: Animation.Infinite
                PauseAnimation { duration: index * 400 }
                NumberAnimation { from: 0.6; to: 1.4; duration: 1200; easing.type: Easing.OutCubic }
            }
            SequentialAnimation on opacity {
                running: root.state === "listening" || root.state === "speaking"
                loops: Animation.Infinite
                PauseAnimation { duration: index * 400 }
                NumberAnimation { from: 0.4; to: 0.0; duration: 1200; easing.type: Easing.OutCubic }
            }
        }
    }

    // === Orbe central con gradiente ===
    Rectangle {
        id: orb
        anchors.centerIn: parent
        width: root.size * 0.6
        height: width
        radius: width / 2

        gradient: Gradient {
            GradientStop { position: 0.0; color: Theme.orbStart }
            GradientStop { position: 0.55; color: Theme.orbMid }
            GradientStop { position: 1.0; color: Theme.orbEnd }
        }

        // Glow
        Rectangle {
            anchors.fill: parent
            anchors.margins: -20
            z: -1
            color: "transparent"
            radius: parent.radius + 20
            border.width: 0

            // Glow radial simulado con un gradiente encima (no soportado nativamente, usamos opacity de un drop-shadow manual)
        }

        // Estado interno: anillos pequenos
        Rectangle {
            visible: root.state === "processing"
            anchors.centerIn: parent
            width: parent.width * 0.5
            height: width
            radius: width / 2
            color: "transparent"
            border.width: 2
            border.color: Qt.rgba(1, 1, 1, 0.5)
            RotationAnimation on rotation {
                from: 0; to: 360
                loops: Animation.Infinite
                duration: 2000
            }
        }

        // Icono central segun estado
        Octicon {
            anchors.centerIn: parent
            size: 28
            name: {
                if (root.state === "processing") return "sync-16"
                if (root.state === "speaking") return "play-16"
                return "unmute-16"
            }
            color: "#ffffff"
        }
    }

    // === Animacion de escala (reactive a amplitud) ===
    // Listening: orb crece con la amplitud
    // Speaking: orb se mantiene estable
    // Processing: orb contraido
    // Idle: orb pequeno
    scale: {
        if (root.state === "listening") {
            return 0.85 + root.amplitude * 0.45  // 0.85 a 1.3
        } else if (root.state === "speaking") {
            return 1.0 + root.speakPulse * 0.07
        } else if (root.state === "processing") {
            return 0.85
        }
        return 0.7  // idle
    }

    Behavior on scale {
        SpringAnimation {
            spring: Theme.springStiffness
            damping: Theme.springDamping
            mass: 0.5
        }
    }

    // === Click handler (wake word / dismiss) ===
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
