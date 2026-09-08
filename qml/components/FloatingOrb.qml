// FloatingOrb.qml - Burbuja flotante estilo Siri
// Ventana pequena sin marco, siempre visible, que aparece al invocar
// al asistente por voz (wake word o push-to-talk). Independiente de la
// ventana principal: se muestra aunque esta este oculta.

import QtQuick
import QtQuick.Window
import qml 1.0
import qml.components 1.0

Window {
    id: root

    property string voiceState: "idle"   // idle | listening | processing | speaking
    property real amplitude: 0.0

    // Oscilacion sintetica para dar vida al orbe mientras escucha
    // (la amplitud real del micro no se propaga a QML).
    property real wobble: 0.0
    SequentialAnimation on wobble {
        running: root.voiceState === "listening"
        loops: Animation.Infinite
        NumberAnimation { from: 0.2; to: 0.9; duration: 700; easing.type: Easing.InOutSine }
        NumberAnimation { from: 0.9; to: 0.2; duration: 700; easing.type: Easing.InOutSine }
    }

    title: "KDE Assistant"
    width: 190
    height: 190
    minimumWidth: width
    minimumHeight: height
    maximumWidth: width
    maximumHeight: height

    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowTransparentForInput
    visible: voiceState !== "idle"

    // Esquina inferior derecha de la pantalla
    x: Screen.desktopAvailableWidth - width - 28
    y: Screen.desktopAvailableHeight - height - 64

    // Aparicion suave
    opacity: visible ? 1.0 : 0.0
    Behavior on opacity {
        NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
    }

    VoiceOrb {
        anchors.centerIn: parent
        state: root.voiceState
        amplitude: root.voiceState === "listening" ? root.wobble : 0.0
        size: 120
    }

    // Etiqueta de estado bajo el orbe
    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 6
        text: {
            if (root.voiceState === "listening") return qsTr("Escuchando…")
            if (root.voiceState === "processing") return qsTr("Procesando…")
            if (root.voiceState === "speaking") return qsTr("Hablando…")
            return ""
        }
        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0)
        color: Theme.ink
        style: Text.Outline
        styleColor: Qt.rgba(0, 0, 0, 0.6)
    }
}
