// SuccessScene.qml - Escena de éxito/confirmación.
// Secuencia: flash happy → bounce → mood happy → settle

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
        { name: "flash", dur: 400, fn: _phaseFlash },
        { name: "celebrate", dur: 1000, fn: _phaseCelebrate },
        { name: "settle", dur: 1200, fn: _phaseSettle }
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
        var phase = _phases[_phases.length > _phaseIndex ? _phaseIndex : 0]
        _phase = phase.name
        phase.fn()
        Qt.callLater(function() {
            var timer = Qt.createQmlObject('import QtQuick; Timer { interval: ' + phase.dur + '; repeat: false; running: true; onTriggered: _advancePhase() }', scene)
        })
    }

    function _advancePhase() { _phaseIndex++; _runPhase() }

    function _phaseFlash() {
        if (expressionCtrl) expressionCtrl.flash("happy", 400)
        if (animationCtrl) animationCtrl.play("bounce", 600)
    }

    function _phaseCelebrate() {
        if (moodCtrl) moodCtrl.setMood("happy", 0.9, 12)
        if (animationCtrl) animationCtrl.play("bounce", 800)
    }

    function _phaseSettle() {
        if (gazeCtrl) gazeCtrl.rest(500)
        // Mood auto-decay via TTL
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
