// ErrorScene.qml - Escena de error.
// Secuencia: flash unsure → mood concerned → shake → gaze down → recover

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
        { name: "flash", dur: 500, fn: _phaseFlash },
        { name: "concerned", dur: 1500, fn: _phaseConcerned },
        { name: "shake", dur: 800, fn: _phaseShake },
        { name: "recover", dur: 1200, fn: _phaseRecover }
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

    function _phaseFlash() {
        if (expressionCtrl) expressionCtrl.flash("unsure", 500)
        if (animationCtrl) animationCtrl.play("shake", 500)
    }

    function _phaseConcerned() {
        if (moodCtrl) moodCtrl.setMood("concerned", 0.9, 30)
        if (gazeCtrl) gazeCtrl.lookAtZone("files", 2000) // mirar hacia abajo
    }

    function _phaseShake() {
        if (animationCtrl) animationCtrl.play("shake", 800)
    }

    function _phaseRecover() {
        if (expressionCtrl) expressionCtrl.flash("curious", 1000)
        if (moodCtrl) moodCtrl.setMood("neutral", 0.7, 0)
        if (gazeCtrl) gazeCtrl.rest(500)
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
