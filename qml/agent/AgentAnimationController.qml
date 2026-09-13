// AgentAnimationController.qml - Registro de animaciones con nombre del personaje.
//
// Fase 1 (prompt §33): blink + breathe (vida base) + appear/disappear.
// Disenado para crecer (walk, run, jump, think, search... en fases 2-3):
//   play(name) / stop(name) / cancelAll(), con prioridad basica.
//
// No anima posicion en pantalla (eso es MovementController, Fase 3).
// Respeta reducedMotion: blink se mantiene (casi imperceptible), breathe y
// appear se atenian a fundidos cortos.

import QtQuick

QtObject {
    id: ctrl

    // El personaje sobre el que actuan las animaciones (AssistantCharacter)
    property Item character: null

    property bool reducedMotion: false

    // Intervalo de parpadeo (blobatar motion: 3.5-6.5 s)
    property int blinkMinMs: 3200
    property int blinkMaxMs: 6500

    // Periodo de respiracion (ms por ciclo completo; lo modula MoodController)
    property int breathePeriodMs: 2400

    // Registro de nombres validos (crece por fases)
    readonly property var known: ["blink", "breathe", "appear", "disappear"]

    // Prioridad basica de las animaciones actuales (0 = ambiente)
    property string active: ""

    signal played(string name)
    signal finished(string name)

    function play(name) {
        if (known.indexOf(name) === -1) {
            console.warn("AnimationController: animacion desconocida '" + name + "'")
            return false
        }
        if (!character) return false
        if (name === "blink") { startBlinking(); }
        else if (name === "breathe") { startBreathing(); }
        else if (name === "appear") { appearAnim.restart(); }
        else if (name === "disappear") { disappearAnim.restart(); }
        active = name
        played(name)
        return true
    }

    function stop(name) {
        if (name === "blink") { blinkTimer.stop(); blinkAnim.stop(); character.blinkScale = 1 }
        else if (name === "breathe") { breathAnim.stop(); character.breathePhase = 0 }
        if (active === name) active = ""
    }

    function cancelAll() {
        stop("blink"); stop("breathe")
        appearAnim.stop(); disappearAnim.stop()
        active = ""
    }

    // === blink: timer aleatorio + cierre/apertura rapido ===
    property Timer _blinkTimer: Timer {
        id: blinkTimer
        repeat: false
        onTriggered: {
            blinkAnim.restart()
            restart()
        }
        Component.onCompleted: interval = 4000
    }

    function _scheduleBlink() {
        blinkTimer.interval = ctrl.blinkMinMs
            + Math.random() * (ctrl.blinkMaxMs - ctrl.blinkMinMs)
        blinkTimer.restart()
    }

    function startBlinking() {
        if (!blinkTimer.running && !ctrl.reducedMotion) _scheduleBlink()
    }

    property SequentialAnimation _blinkAnim: SequentialAnimation {
        id: blinkAnim
        alwaysRunToEnd: true
        NumberAnimation { target: ctrl.character; property: "blinkScale"; from: 1.0; to: 0.06; duration: 75; easing.type: Easing.InQuad }
        NumberAnimation { target: ctrl.character; property: "blinkScale"; from: 0.06; to: 1.0; duration: 130; easing.type: Easing.OutQuad }
        onFinished: ctrl.finished("blink")
    }

    // === breathe: ciclo eterno suave (breathePhase 0->1->0) ===
    function startBreathing() {
        breathAnim.loops = Animation.Infinite
        breathAnim.start()
    }

    property SequentialAnimation _breathAnim: SequentialAnimation {
        id: breathAnim
        NumberAnimation {
            target: ctrl.character; property: "breathePhase"
            from: 0.0; to: 1.0; duration: ctrl.breathePeriodMs / 2
            easing.type: Easing.InOutSine
        }
        NumberAnimation {
            target: ctrl.character; property: "breathePhase"
            from: 1.0; to: 0.0; duration: ctrl.breathePeriodMs / 2
            easing.type: Easing.InOutSine
        }
    }

    // === appear / disappear: pops suaves (fondo para entradas/salidas) ===
    property SequentialAnimation _appear: SequentialAnimation {
        id: appearAnim
        ScriptAction { script: { ctrl.character.visible = true } }
        NumberAnimation {
            target: ctrl.character; property: "opacity"
            from: 0.0; to: 1.0; duration: ctrl.reducedMotion ? 120 : 240
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: ctrl.character; properties: "squashY"
            from: 0.55; to: 1.0; duration: ctrl.reducedMotion ? 0 : 300
            easing.type: Easing.OutBack; easing.overshoot: 1.4
        }
        onFinished: ctrl.finished("appear")
    }

    property SequentialAnimation _disappear: SequentialAnimation {
        id: disappearAnim
        NumberAnimation {
            target: ctrl.character; property: "opacity"
            to: 0.0; duration: ctrl.reducedMotion ? 100 : 200
            easing.type: Easing.InCubic
        }
        PropertyAction { target: ctrl.character; property: "visible"; value: false }
        PropertyAction { target: ctrl.character; property: "opacity"; value: 1.0 }
        onFinished: ctrl.finished("disappear")
    }
}
