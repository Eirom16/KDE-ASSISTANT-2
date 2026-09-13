// DownloadScene.qml - Escena de descarga/progreso.
// Secuencia: gaze notificaciones → focused waiting → progress pulse → success

import QtQuick

QtObject {
    id: scene

    property var characterCtrl: null
    property var expressionCtrl: null
    property var animationCtrl: null
    property var moodCtrl: null
    property var gazeCtrl: null
    property var movementCtrl: null

    property string _phase: "idle"
    property int _phaseIndex: 0

    readonly property var _phases: [
        { name: "gaze", dur: 400, fn: _phaseGaze },
        { name: "wait", dur: 2000, fn: _phaseWait },
        { name: "complete", dur: 1000, fn: _phaseComplete }
    ]

    signal started()
    signal finished()
    signal cancelled()

    function start(params) {
        _phaseIndex = 0
        _runPhase()
    }

    function _runPhase() {
        if (_phaseIndex >= _phases.length) { _finish(); return }
        var phase = _phases[_phaseIndex]
        _phase = phase.name
        phase.fn()
        Qt.callLater(function() {
            var timer = Qt.createQmlObject('import QtQuick; Timer { interval: ' + phase.dur + '; repeat: false; running: true; onTriggered: _advancePhase() }', scene)
        })
    }

    function _advancePhase() { _phaseIndex++; _runPhase() }

    function _phaseGaze() {
        if (gazeCtrl) gazeCtrl.lookAtZone("notifications", 3000)
        if (expressionCtrl) expressionCtrl.request("focused")
        if (moodCtrl) moodCtrl.setMood("focused", 0.8, 0)
    }

    function _phaseWait() {
        if (animationCtrl) animationCtrl.play("rock", 2000) // balanceo thinking
        // Podría pulsar breathe más rápido para mostrar espera
    }

    function _phaseComplete() {
        if (expressionCtrl) expressionCtrl.flash("happy", 1000)
        if (moodCtrl) moodCtrl.setMood("happy", 0.8, 10)
        if (animationCtrl) animationCtrl.play("bounce", 800)
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
