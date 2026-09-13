// ScreenshotScene.qml - Escena de captura de pantalla.
// Secuencia: surprised flash → click → success

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
        { name: "surprise", dur: 400, fn: _phaseSurprise },
        { name: "capture", dur: 300, fn: _phaseCapture },
        { name: "success", dur: 1000, fn: _phaseSuccess }
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

    function _phaseSurprise() {
        if (expressionCtrl) expressionCtrl.flash("surprised", 400)
        if (animationCtrl) animationCtrl.play("bounce", 300)
    }

    function _phaseCapture() {
        if (expressionCtrl) expressionCtrl.flash("happy", 300)
    }

    function _phaseSuccess() {
        if (expressionCtrl) expressionCtrl.flash("happy", 1000)
        if (moodCtrl) moodCtrl.setMood("happy", 0.7, 8)
        if (animationCtrl) animationCtrl.play("bounce", 600)
    }

    function _finish() { finished() }
    function cancel() { _phaseIndex = _phases.length; _finish(); cancelled() }
}
