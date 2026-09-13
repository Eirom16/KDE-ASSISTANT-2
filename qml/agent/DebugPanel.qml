// DebugPanel.qml - Panel de desarrollo/debug opcional.
// Muestra estado interno del personaje en tiempo real.
// Solo disponible en modo debug (se puede desactivar completamente en producción).
// Nota: sin QtQuick.Timers (no disponible en qml6 runtime): refresh via setInterval JS.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0

Item {
    id: panel

    property bool open_: false
    property var characterCtrl: null
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var idleBehavior: null
    property var movementCtrl: null
    property var sceneCtrl: null
    property var intentRouter: null
    property var visualGuide: null

    visible: open_
    z: 1000

    property int _refreshTimerId: 0

    function show() {
        open_ = true
        _startRefresh()
    }
    function hide() {
        open_ = false
        _stopRefresh()
    }
    function toggle() {
        if (open_) hide()
        else show()
    }

    function _startRefresh() {
        if (_refreshTimerId !== 0) return
        _refreshTimerId = setInterval(refresh, 100)
        refresh()
    }

    function _stopRefresh() {
        if (_refreshTimerId !== 0) {
            clearInterval(_refreshTimerId)
            _refreshTimerId = 0
        }
    }

    Component.onDestruction: {
        if (_refreshTimerId !== 0) clearInterval(_refreshTimerId)
    }

    function _prop(obj, name) { return obj ? obj[name] : undefined }
    function _num(v, d) { return (typeof v === "number") ? v.toFixed(d) : "?" }

    function refresh() {
        if (!open_) return
        exprText.text   = "Expression: " + (_prop(expressionCtrl, "currentExpression") || _prop(expressionCtrl, "_expression") || "?")
        moodText.text   = "Mood: " + (_prop(moodCtrl, "mood") || "?") + " (int: " + _num(_prop(moodCtrl, "intensity"), 2) + ")"
        gazeText.text   = "Gaze: " + _num(_prop(gazeCtrl, "curX"), 2) + ", " + _num(_prop(gazeCtrl, "curY"), 2) + " (zone: " + (_prop(gazeCtrl, "_zone") || "none") + ")"
        animText.text   = "Anim: " + (_prop(animationCtrl, "_currentAnimation") || "none")
        moveText.text   = "Move: x=" + _num(_prop(movementCtrl, "x"), 1) + " y=" + _num(_prop(movementCtrl, "y"), 1) + " vx=" + _num(_prop(movementCtrl, "vx"), 1) + " vy=" + _num(_prop(movementCtrl, "vy"), 1) + " ground=" + (_prop(movementCtrl, "onGround") ? "Y" : "N")
        sceneText.text  = "Scene: " + (_prop(sceneCtrl, "_currentScene") || "none") + " (prio: " + (_prop(sceneCtrl, "_scenePriority") || 0) + ")"
        intentText.text = "Intent: " + (_prop(intentRouter, "_currentIntentTool") || "none")
        idleText.text   = "Idle: enabled=" + !!_prop(idleBehavior, "enabled") + " busy=" + !!_prop(idleBehavior, "busy")
        priorityText.text = "Priority: " + (_prop(characterCtrl, "_activePriority") === 0 ? "0" : (_prop(characterCtrl, "_activePriority") || "?"))
    }

    Rectangle {
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 60
        anchors.rightMargin: 20
        width: 380
        height: parent.height - 100
        radius: 12
        color: "#e61d1d1f"
        border.width: 1
        border.color: "#3a3a3c"

        Column {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 6

            Text { text: "Character Debug"; color: "#ffffff"; font.pixelSize: 14; font.bold: true }

            Flow {
                width: parent.width
                spacing: 4
                Repeater {
                    model: ["idle","happy","sad","mad","surprised","wink","sleepy","smug","unsure","scared","love","shy","sick","thinking"]
                    delegate: Rectangle {
                        required property string modelData
                        width: lbl.width + 14; height: 24; radius: 12; color: "#2a2a2c"
                        Text { id: lbl; anchors.centerIn: parent; text: modelData; color: "#cccccc"; font.pixelSize: 10 }
                        MouseArea { anchors.fill: parent; onClicked: if (expressionCtrl) expressionCtrl.request(modelData) }
                    }
                }
            }

            Flow {
                width: parent.width
                spacing: 4
                Repeater {
                    model: ["search","openapp","file","system","error","success","download","screenshot"]
                    delegate: Rectangle {
                        required property string modelData
                        width: lbl2.width + 14; height: 24; radius: 12; color: "#173a4a"
                        Text { id: lbl2; anchors.centerIn: parent; text: modelData; color: "#7ec8e3"; font.pixelSize: 10 }
                        MouseArea { anchors.fill: parent; onClicked: if (sceneCtrl) sceneCtrl.play(modelData) }
                    }
                }
            }

            Flow {
                width: parent.width
                spacing: 4
                Repeater {
                    model: ["walk","run","jump","bounce","return"]
                    delegate: Rectangle {
                        required property string modelData
                        width: lbl3.width + 14; height: 24; radius: 12; color: "#2a4a17"
                        Text { id: lbl3; anchors.centerIn: parent; text: modelData; color: "#a5e37e"; font.pixelSize: 10 }
                        MouseArea { anchors.fill: parent; onClicked: {
                            if (!movementCtrl) return
                            if (modelData === "walk") movementCtrl.walkTo(80, 100)
                            else if (modelData === "run") movementCtrl.runTo(250, 100)
                            else if (modelData === "jump") movementCtrl.jump()
                            else if (modelData === "bounce") movementCtrl.bounce(1.0)
                            else if (modelData === "return") movementCtrl.returnHome()
                        } }
                    }
                }
            }

            Rectangle { width: parent.width; height: 1; color: "#3a3a3c" }

            Text { id: exprText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: moodText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: gazeText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: animText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: moveText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: sceneText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: intentText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: idleText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
            Text { id: priorityText; color: "#aaaaaa"; font.pixelSize: 10; font.family: "monospace" }
        }
    }
}
