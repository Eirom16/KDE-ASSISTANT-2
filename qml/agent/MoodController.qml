// MoodController.qml - Estado emocional interno del personaje (§6).
//
// Un mood = expresion base asociada + perfil de movimiento (ritmo de
// respiracion, amplitud) + perfil de mirada (frecuencia de glances).
// Reglas:
// - minDwellMs: un mood no puede cambiar antes de ese tiempo (la ultima
//   peticion queda pendiente; evita que cada respuesta del LLM lo mueva).
// - ttlSecs>0: el mood caduca y vuelve a "neutral" solo.
// - intensity 0..1 modula amplitud de respiracion y frecuencias.

import QtQuick

QtObject {
    id: mood

    // Subsistemas a los que aplica el perfil
    property var expressionCtrl: null

    property string currentMood: "neutral"
    property real intensity: 0.7          // 0..1
    property int minDwellMs: 2600

    // Perfil resultante (lo leen AgentAnimationController / IdleBehavior)
    readonly property int breathePeriodMs: _profile.breathePeriodMs * (1.0 - 0.25 * (intensity - 0.5) * 2)
    readonly property real breatheAmp: _profile.breatheAmp * (0.6 + 0.8 * intensity)
    readonly property int glanceEveryMs: Math.max(3000, _profile.glanceEveryMs / (0.6 + 0.8 * intensity))
    readonly property string expression: _profile.expression

    signal moodDropped(string name)   // peticion pendiente reemplazada

    property int _ttl: 0
    property string _pending: ""
    property real _pendingIntensity: 0.7
    property int _pendingTtl: 0
    property double _lastChange: 0

    // Tabla de moods (§6). expression valida contra CharacterData.
    readonly property var profiles: ({
        "neutral":   { expression: "idle",      breathePeriodMs: 2400, breatheAmp: 1.0,  glanceEveryMs: 9000 },
        "happy":     { expression: "happy",     breathePeriodMs: 2100, breatheAmp: 1.1,  glanceEveryMs: 7000 },
        "curious":   { expression: "idle",      breathePeriodMs: 2100, breatheAmp: 1.0,  glanceEveryMs: 5000 },
        "focused":   { expression: "thinking",  breathePeriodMs: 2600, breatheAmp: 0.8,  glanceEveryMs: 14000 },
        "excited":   { expression: "happy",     breathePeriodMs: 1700, breatheAmp: 1.35, glanceEveryMs: 6000 },
        "confused":  { expression: "unsure",    breathePeriodMs: 2300, breatheAmp: 0.9,  glanceEveryMs: 8000 },
        "concerned": { expression: "sad",       breathePeriodMs: 2800, breatheAmp: 0.8,  glanceEveryMs: 11000 },
        "sleepy":    { expression: "sleepy",    breathePeriodMs: 3600, breatheAmp: 0.65, glanceEveryMs: 22000 }
    })

    property var _profile: profiles["neutral"]

    function isValid(name) {
        return profiles[name] !== undefined
    }

    // Peticion de mood. ttlSecs=0 -> persistente hasta nueva orden.
    function setMood(name, intensityValue, ttlSecs) {
        if (!isValid(name)) {
            console.warn("MoodController: mood desconocido '" + name + "'")
            return false
        }
        var now = Date.now()
        var elapsed = now - _lastChange
        if (elapsed < minDwellMs && name !== currentMood) {
            if (_pending !== "" && _pending !== name) moodDropped(_pending)
            _pending = name
            _pendingIntensity = intensityValue > 0 ? intensityValue : 0.7
            _pendingTtl = ttlSecs > 0 ? ttlSecs : 0
            dwellTimer.interval = minDwellMs - elapsed
            dwellTimer.restart()
            return true
        }
        _apply(name, intensityValue > 0 ? intensityValue : 0.7, ttlSecs > 0 ? ttlSecs : 0)
        return true
    }

    function _apply(name, intensityValue, ttlSecs) {
        _profile = profiles[name]
        _pending = ""
        currentMood = name
        intensity = Math.max(0.1, Math.min(1.0, intensityValue))
        _ttl = ttlSecs
        _lastChange = Date.now()
        if (expressionCtrl)
            expressionCtrl.baseExpression = _profile.expression
        if (ttlSecs > 0) {
            ttlTimer.interval = ttlSecs * 1000
            ttlTimer.restart()
        } else {
            ttlTimer.stop()
        }
        // (moodChanged llega via la signal automatica de la property)
    }

    property Timer _dwell: Timer {
        id: dwellTimer
        repeat: false
        onTriggered: {
            if (mood._pending !== "") {
                var m = mood._pending
                mood._pending = ""
                mood._apply(m, mood._pendingIntensity, mood._pendingTtl)
            }
        }
    }

    property Timer _ttlT: Timer {
        id: ttlTimer
        repeat: false
        onTriggered: mood._apply("neutral", 0.7, 0)
    }
}
