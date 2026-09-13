// DevPreview.qml - Harness de desarrollo del Assistant Character.
// NO va a produccion: lo cubre DebugPanel (Fase 8 / prompt §31).
//
//   qml6 -I . qml/agent/DevPreview.qml
//
// Cubre el stack completo de Fases 1-2: personaje + expresiones +
// animaciones + mood + gaze + idle (dormir/despertar).

import QtQuick
import QtQuick.Controls
import QtQuick.Window

Window {
    id: root
    width: 620
    height: 720
    visible: true
    color: "#1d1d1f"
    title: "Agent Dev Preview"

    // === Personaje ===
    AssistantCharacter {
        id: character
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 40
        size: 180
        pose: exprCtrl.pose
        breatheAmp: moodCtrl.breatheAmp
    }

    ExpressionController { id: exprCtrl }
    AgentAnimationController {
        id: animCtrl
        character: character
        breathePeriodMs: moodCtrl.breathePeriodMs
    }
    MoodController { id: moodCtrl; expressionCtrl: exprCtrl }
    GazeController { id: gazeCtrl; character: character; window: root }
    IdleBehavior {
        id: idle
        gaze: gazeCtrl
        expressionCtrl: exprCtrl
        moodCtrl: moodCtrl
        character: character
        sleepAfterSecs: 60
    }
    CharacterController {
        id: cc
        expressionCtrl: exprCtrl
        animationCtrl: animCtrl
        moodCtrl: moodCtrl
        gazeCtrl: gazeCtrl
        idleBehavior: idle
    }

    Component.onCompleted: {
        animCtrl.play("breathe")
        animCtrl.play("blink")
    }

    // Mirada al cursor sobre el area del personaje
    MouseArea {
        anchors.fill: character
        hoverEnabled: true
        onPositionChanged: function(m) { gazeCtrl.lookAtLocal(m.x, m.y, width, height) }
        onExited: gazeCtrl.rest(600)
    }

    // === Panel de control ===
    Column {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 14
        spacing: 8

        Text {
            color: "#ffffff"
            font.pixelSize: 12
            text: "expr: " + exprCtrl.expression
                + "  |  mood: " + moodCtrl.mood + " (" + moodCtrl.intensity.toFixed(2) + ")"
                + "  |  sleep: " + idle.sleeping
                + "  |  gaze: " + character.gazeOffset.x.toFixed(1) + "," + character.gazeOffset.y.toFixed(1)
                + "  |  posture: " + character.postureRot.toFixed(1)
        }

        // Expresiones (14)
        Grid {
            columns: 7
            spacing: 6
            Repeater {
                model: ["idle","happy","sad","mad","surprised","wink","sleepy","smug","unsure","scared","love","shy","sick","thinking"]
                delegate: Button {
                    required property string modelData
                    text: modelData
                    font.pixelSize: 10
                    onClicked: exprCtrl.force(modelData)
                }
            }
        }

        // Eventos del asistente
        Grid {
            columns: 5
            spacing: 6
            Repeater {
                model: ["wake", "listening", "processing", "speaking", "done", "error", "happy", "wink", "idle"]
                delegate: Button {
                    required property string modelData
                    text: modelData
                    font.pixelSize: 10
                    onClicked: cc.handleEvent(modelData, "NORMAL")
                }
            }
        }

        // Moods + gaze + sleep/wake + anim
        Grid {
            columns: 4
            spacing: 6
            Repeater {
                model: [
                    {label: "mood focused", act: function() { moodCtrl.setMood("focused", 0.9, 0) }},
                    {label: "mood happy", act: function() { moodCtrl.setMood("happy", 0.8, 0) }},
                    {label: "mood neutral", act: function() { moodCtrl.setMood("neutral", 0.7, 0) }},
                    {label: "mood sleepy", act: function() { moodCtrl.setMood("sleepy", 0.7, 0) }},
                    {label: "gaze random", act: function() { gazeCtrl.glanceRandom() }},
                    {label: "gaze zone files", act: function() { gazeCtrl.lookAtZone("files", 3000) }},
                    {label: "sleep ahora", act: function() { cc.handleEvent("sleep", "NORMAL") }},
                    {label: "despertar", act: function() { cc.handleEvent("wakeIdle", "NORMAL") }},
                    {label: "appear", act: function() { animCtrl.play("appear") }},
                    {label: "disappear", act: function() { animCtrl.play("disappear") }}
                ]
                delegate: Button {
                    required property var modelData
                    text: modelData.label
                    font.pixelSize: 10
                    onClicked: modelData.act()
                }
            }
        }
    }
}
