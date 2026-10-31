import QtQuick

QtObject {
    id: root
    property string backendUrl: ""
    property string token: ""
    property string sessionId: ""
    property string state: "idle"
    property var identity: null
    property int generation: 0
    property bool polling: false
    property bool requestPending: false
    property string deliveredGeneration: ""
    property bool backgroundMonitoring: false
    property bool backgroundEnabled: false
    property bool backgroundPaused: false
    property bool microphoneActive: false
    property string backgroundState: "off"
    property string lastActivation: ""
    property string lastBackgroundError: ""
    property var deliveredBackground: []
    property bool backgroundRequest: false
    property bool backgroundChanging: false
    property int backgroundGeneration: 0
    signal wakeDetected()
    readonly property bool busy: requestPending || ["preparing", "wake_waiting", "recording", "transcribing", "cancelling", "speaking"].indexOf(state) >= 0
    signal transcriptReady(string text)
    signal failed(string message)
    onSessionIdChanged: cancel()

    function request(path, body, callback) {
        var req = new XMLHttpRequest()
        req.open(body === null ? "GET" : "POST", backendUrl + path)
        req.setRequestHeader("Authorization", "Bearer " + token)
        req.setRequestHeader("Content-Type", "application/json")
        var finished = false
        var timer = Qt.createQmlObject('import QtQuick; Timer { interval: 5000; repeat: false }', root)
        function done(ok, value) {
            if (finished) return
            finished = true
            timer.stop()
            timer.destroy()
            callback(ok, value)
        }
        timer.triggered.connect(function() { done(false, {error: qsTr("El servicio de voz no responde")}); req.abort() })
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            var value = {}
            try { value = JSON.parse(req.responseText) } catch (e) {}
            done(req.status >= 200 && req.status < 300, value)
        }
        timer.start()
        req.send(body === null ? undefined : JSON.stringify(body))
    }
    function applySnapshot(value) {
        if (!value.identity && (value.state === "idle" || value.state === "disabled")) {
            polling = false; identity = null; state = value.state; return
        }
        if (!identity || !value.identity || value.identity.generation !== identity.generation
                || value.identity.session_id !== sessionId) return
        state = value.state
        if (state === "transcript_ready") {
            polling = false
            if (deliveredGeneration !== identity.generation) {
                deliveredGeneration = identity.generation
                transcriptReady(value.transcript || "")
            }
        } else if (state === "error") {
            polling = false
            failed(value.error || qsTr("Motor de voz detenido"))
        } else if (state === "idle" || state === "disabled") polling = false
    }
    function consumeStatus(value) {
        var bg = value.background
        if (bg) {
            backgroundEnabled = bg.enabled === true
            backgroundPaused = bg.paused === true
            microphoneActive = bg.microphone_active === true
            backgroundState = bg.state || "off"
            if (bg.activation && bg.activation.session_id === sessionId && bg.activation.generation !== lastActivation) {
                lastActivation = bg.activation.generation
                identity = bg.activation
                wakeDetected()
            }
            if (bg.error && bg.state === "error" && bg.error !== lastBackgroundError) {
                lastBackgroundError = bg.error; failed(bg.error)
            } else if (bg.state !== "error") lastBackgroundError = ""
        }
        applySnapshot(value)
        if (!bg) return
        var pending = bg.transcripts || []
        for (var i = 0; i < pending.length; i++) {
            var item = pending[i]; var id = item.identity
            if (!id || id.session_id !== sessionId) continue
            if (id.generation !== deliveredGeneration && deliveredBackground.indexOf(id.generation) < 0) {
                deliveredGeneration = id.generation
                transcriptReady(item.transcript || "")
            }
            if (deliveredBackground.indexOf(id.generation) < 0) {
                deliveredBackground = deliveredBackground.concat([id.generation]).slice(-32)
                request("/api/voice/background", {action:"ack",generation:id.generation}, function(ok,v) {})
            }
        }
    }
    function backgroundControl(action) {
        if (backgroundChanging) return
        if ((action === "enable" || action === "resume") && !sessionId) return
        backgroundChanging = true
        if (action === "pause" || action === "disable") {
            ++generation; polling=false; requestPending=false; identity=null; state="cancelling"
        }
        backgroundRequest = true
        var expectedSession = sessionId
        var bgGeneration = ++backgroundGeneration
        function control() {
            request("/api/voice/background", {action:action}, function(ok,value) {
                backgroundRequest = false
                backgroundChanging = false
                if (!ok) { failed(value.error || qsTr("No se pudo cambiar la escucha")); return }
                backgroundEnabled = value.enabled === true; backgroundPaused = value.paused === true
                microphoneActive = value.microphone_active === true; backgroundState = value.state || "off"
                if (action === "pause" || action === "disable") state="idle"
            })
        }
        if (action === "enable" || action === "resume") {
            request("/api/voice/session", {session_id:sessionId}, function(ok,value) {
                if (bgGeneration !== backgroundGeneration || expectedSession !== sessionId || !ok || value.session_id !== sessionId) { backgroundChanging=false; backgroundRequest=false; failed(qsTr("Conversación no disponible")); return }
                control()
            })
        } else control()
    }
    function backgroundPoll() {
        if (backgroundRequest || polling) return
        backgroundRequest = true
        var expected = backgroundGeneration
        request("/api/voice/status", null, function(ok,value) {
            backgroundRequest = false
            if (ok && expected === backgroundGeneration) consumeStatus(value)
        })
    }
    function start(wake) {
        if (busy || !sessionId) return
        var gen = ++generation
        state = "preparing"
        requestPending = true
        request("/api/voice/session", {session_id: sessionId}, function(ok, scope) {
            if (gen !== generation) return
            if (!ok || scope.session_id !== sessionId) { requestPending = false; state = "error"; failed(scope.error || qsTr("Selecciona una conversación")); return }
            identity = {session_id: sessionId, generation: scope.scope, turn_id: null}
            request("/api/voice/start", {session_id: sessionId, scope: scope.scope, wake: wake === true}, function(started, value) {
                if (gen !== generation) return
                requestPending = false
                if (!started) { cancel(); state = "error"; failed(value.error || qsTr("No se pudo preparar el micrófono")); return }
                applySnapshot(value)
                polling = true
                poll()
            })
        })
    }
    function stop() {
        if (!identity || (state !== "recording" && state !== "preparing")) return
        var gen = generation
        requestPending = true
        request("/api/voice/stop", identity, function(ok, value) {
            if (gen !== generation) return
            requestPending = false
            if (!ok) { cancel(); state = "error"; failed(value.error || qsTr("No se pudo detener el micrófono")) }
        })
    }
    function cancel() {
        ++generation
        polling = false
        requestPending = false
        var previous = identity
        identity = null
        state = previous ? "cancelling" : "idle"
        var gen = generation
        if (previous) request("/api/voice/cancel", previous, function(ok, value) {
            if (gen !== generation) return
            state = ok ? "idle" : "error"
            if (!ok) failed(value.error || qsTr("No se confirmó la detención de voz"))
        })
    }
    function poll() {
        if (requestPending || !polling) return
        var gen = generation
        requestPending = true
        request("/api/voice/status", null, function(ok, value) {
            if (gen !== generation) return
            requestPending = false
            if (!ok) { cancel(); failed(value.error || qsTr("Servicio de voz no disponible")); return }
            consumeStatus(value)
        })
    }
    function read(text) {
        if (!sessionId || busy) return
        var gen = ++generation
        requestPending = true
        request("/api/voice/session", {session_id: sessionId}, function(ok, selected) {
            if (gen !== generation) return
            if (!ok) { requestPending = false; state = "error"; failed(qsTr("Conversación no disponible")); return }
            identity = {session_id: sessionId, generation: selected.scope, turn_id: null}
            request("/api/speak", {session_id: sessionId, scope: selected.scope, text: text}, function(ok, value) {
                if (gen !== generation) return
                requestPending = false
                if (!ok) { cancel(); state = "error"; failed(value.error || qsTr("Piper no disponible")); return }
                identity = value.identity
                applySnapshot(value)
                polling = true
            })
        })
    }
    function detach() {
        cancel()
        request("/api/voice/session", {session_id: null}, function(ok, value) {})
    }
    function hide() { if (!backgroundEnabled) detach() }
    function observe() {
        if (busy) return
        var gen = generation
        request("/api/voice/status", null, function(ok, value) {
            if (gen !== generation || !ok || !value.identity || value.identity.session_id !== sessionId) return
            if (["preparing", "speaking", "error"].indexOf(value.state) < 0) return
            identity = value.identity
            applySnapshot(value)
            polling = state !== "error"
        })
    }
    property Timer pollTimer: Timer { interval: 200; running: root.polling; repeat: true; onTriggered: root.poll() }
    property Timer backgroundTimer: Timer { interval: 200; running: root.backgroundMonitoring; repeat: true; onTriggered: root.backgroundPoll() }
}
