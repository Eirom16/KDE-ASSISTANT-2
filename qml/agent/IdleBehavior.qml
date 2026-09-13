// IdleBehavior.qml - Comportamiento autonomo del personaje (§17).
//
// Reglas del prompt: vivo pero no molesto. Frecuencias bajas, siempre
// moduladas por el mood y pausadas cuando hay actividad real.
// - glance: miradas aleatorias cada ~6-14s (moduladas por el mood).
// - posture: pequenio cambio de postura cada ~45-120s.
// - sleep: tras sleepAfterSecs sin actividad -> dormido (sleepy, Zzz,
//   respiracion lenta). Cualquier actividad (voz, hover, click, escena,
//   intent) despierta: micro-sorpresa + vuelta al mood.
// - busy: voz/escenas pausan glances y postura (no interrumpen).

import QtQuick

QtObject {
    id: idle

    property var gaze: null
    property var expressionCtrl: null
    property var moodCtrl: null
    property Item character: null

    property bool enabled: true
    property bool busy: false               // voz/escena en curso
    property int sleepAfterSecs: 240
    property bool sleeping: false

    signal fellAsleep()
    signal wokeUp()

    // === Actividad (llamada por cualquier fuente de eventos) ===
    function notifyActivity(kind) {
        if (sleeping) wake()
        sleepTimer.restart()
    }

    function sleep() {
        if (sleeping || !enabled) return
        sleeping = true
        glanceTimer.stop()
        postureTimer.stop()
        if (moodCtrl) moodCtrl.setMood("sleepy", 0.6, 0)
        else if (expressionCtrl) expressionCtrl.baseExpression = "sleepy"
        if (character) character.sleeping = true
        if (gaze) { gaze.saccadesEnabled = false; gaze.rest(0) }
        postureReturn()
        fellAsleep()
    }

    function wake() {
        if (!sleeping) return
        sleeping = false
        if (gaze) gaze.saccadesEnabled = true
        if (character) character.sleeping = false
        // Micro-despertar: sorpresa corta -> el MoodController devuelve a la
        // expresion del mood actual por su baseExpression (idle aqui: neutral)
        if (moodCtrl) moodCtrl.setMood("neutral", 0.7, 0)
        if (expressionCtrl) expressionCtrl.flash("surprised", 550)
        glanceTimer.restart()
        postureTimer.restart()
        wokeUp()
    }

    // === Timers ===

    property Timer _sleep: Timer {
        id: sleepTimer
        repeat: false
        interval: Math.max(5, idle.sleepAfterSecs) * 1000
        running: idle.enabled
        onTriggered: idle.sleep()
    }

    property Timer _glance: Timer {
        id: glanceTimer
        repeat: true
        interval: 8000
        running: idle.enabled
        onTriggered: {
            var base = idle.moodCtrl ? idle.moodCtrl.glanceEveryMs : 9000
            interval = base * 0.7 + Math.random() * base * 0.6
            if (idle.busy || idle.sleeping || !idle.gaze) return
            idle.gaze.glanceRandom()
        }
    }

    property Timer _posture: Timer {
        id: postureTimer
        repeat: true
        interval: 60000
        running: idle.enabled
        onTriggered: {
            interval = 45000 + Math.random() * 75000
            if (idle.busy || idle.sleeping || !idle.character) return
            var next = (Math.random() - 0.5) * 4.4   // -2.2 .. 2.2 grados
            if (Math.abs(next - idle.character.postureRot) < 0.8)
                next = next >= 0 ? -1.6 : 1.6
            postureAnim.to = next
            postureAnim.restart()
        }
    }

    function postureReturn() {
        postureAnim.to = 0
        postureAnim.restart()
    }

    property NumberAnimation _postureAnim: NumberAnimation {
        id: postureAnim
        target: idle.character
        property: "postureRot"
        duration: 900
        easing.type: Easing.InOutSine
    }
}
