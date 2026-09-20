// AgentOverlay.qml - Ventana layer-shell del personaje (Wayland nativo).
// Proceso separado (lo lanza main.rs como segundo qml6). Independiente del
// chat: minimizar la ventana principal NO oculta el personaje.
//
// LayerShell API (layer-shell-qt 6.7): PROPIEDAD ASOCIADA en Window:
//   LayerShell.Window.anchors, LayerShell.Window.margins, LayerShell.Window.layer, ...
// La ventana es pequeña (cuerpo del personaje + margen para saltos) y
// anclada abajo-derecha: no roba foco ni clics fuera de su rectángulo.

import QtQuick
import QtQuick.Window
import org.kde.layershell 1.0 as LayerShell
import qml.agent 1.0
import qml.auth 1.0

Window {
    id: overlay

    // === Config (se leen de /api/config al arrancar) ===
    property int characterSize: 140
    property string characterAppearance: "capsule"
    property color characterAccentColor: "#0094bb"
    property string presenceMode: "companion"
    property bool reducedMotion: false
    property int sleepAfterSecs: 240
    property bool clickThrough: false
    property bool interactive: true

    // === Estado asíncrono del asistente (SSE /api/voice/stream) ===
    property string assistantState: "idle"
    property string voiceState: "idle"
    property string chatState: "idle"
    property real voiceLevel: 0.0
    property string backendUrl: "http://127.0.0.1:8765"
    property bool keepVisible: false
    property bool fadingOut: false
    property int idlePresenceMs: 7000

    title: "KDE Assistant Agent"
    color: "transparent"
    flags: Qt.Tool | Qt.FramelessWindowHint
    width: characterSize + 16
    height: characterSize * 2   // espacio vertical para saltos
    visible: voiceState !== "idle" || agent.activityVisible || keepVisible || fadingOut

    // Wayland: convertir esta ventana en layer-shell surface
    LayerShell.Window.anchors: LayerShell.Window.AnchorBottom | LayerShell.Window.AnchorRight
    LayerShell.Window.layer: LayerShell.Window.LayerTop
    LayerShell.Window.exclusionZone: -1
    LayerShell.Window.keyboardInteractivity: LayerShell.Window.KeyboardInteractivityNone
    LayerShell.Window.margins.bottom: 20
    LayerShell.Window.margins.right: 24
    LayerShell.Window.scope: "kde-assistant-agent"

    function moveBy(dx, dy) {
        var maxRight = Math.max(0, Screen.desktopAvailableWidth - overlay.width)
        var maxBottom = Math.max(0, Screen.desktopAvailableHeight - overlay.height)
        LayerShell.Window.margins.right = Math.max(0, Math.min(maxRight, LayerShell.Window.margins.right - dx))
        LayerShell.Window.margins.bottom = Math.max(0, Math.min(maxBottom, LayerShell.Window.margins.bottom - dy))
    }

    function wakeAgentVisuals() {
        keepVisible = true
        fadingOut = false
        settleTimer.stop()
        hideAfterFadeTimer.stop()
        agent.ensureVisible()
    }

    function scheduleAgentHide() {
        if (voiceState !== "idle" || chatState !== "idle" || agent.activityVisible) return
        settleTimer.restart()
    }

    onVoiceStateChanged: {
        if (voiceState !== "idle") wakeAgentVisuals()
        else scheduleAgentHide()
    }

    onChatStateChanged: {
        if (chatState !== "idle") wakeAgentVisuals()
        else scheduleAgentHide()
    }

    // === Personaje (Item dentro de esta ventana overlay) ===
    AgentWindow {
        id: agent
        anchors.fill: parent
        agentEnabled: true
        reducedMotion: overlay.reducedMotion
        characterSize: overlay.characterSize
        characterAppearance: overlay.characterAppearance
        characterAccentColor: overlay.characterAccentColor
        presenceMode: overlay.presenceMode
        sleepAfterSecs: overlay.sleepAfterSecs
        assistantState: overlay.assistantState
        voiceLevel: overlay.voiceLevel
        moveWholeWindow: true
        hostVisible: overlay.visible
        onMoveRequested: function(dx, dy) {
            overlay.moveBy(dx, dy)
        }
    }

    Connections {
        target: agent
        function onActivityVisibleChanged() {
            if (agent.activityVisible) overlay.wakeAgentVisuals()
            else overlay.scheduleAgentHide()
        }
    }

    Timer {
        id: settleTimer
        interval: overlay.idlePresenceMs
        repeat: false
        onTriggered: {
            if (overlay.voiceState !== "idle" || overlay.chatState !== "idle" || agent.activityVisible) return
            overlay.fadingOut = true
            agent.softDisappear()
            hideAfterFadeTimer.restart()
        }
    }

    Timer {
        id: hideAfterFadeTimer
        interval: overlay.reducedMotion ? 40 : 320
        repeat: false
        onTriggered: {
            overlay.keepVisible = false
            overlay.fadingOut = false
        }
    }

    // === Carga de config ===
    property string authToken: AuthToken.token
    function setAuth(xhr) {
        if (authToken !== "") xhr.setRequestHeader("Authorization", "Bearer " + authToken)
    }

    function loadConfig() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/config")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    var cfg = JSON.parse(xhr.responseText)
                    if (cfg.character) {
                        if (cfg.character.size !== undefined)
                            characterSize = parseInt(cfg.character.size) || 140
                        if (cfg.character.appearance)
                            characterAppearance = String(cfg.character.appearance)
                        if (cfg.character.accent_color)
                            characterAccentColor = cfg.character.accent_color
                        if (cfg.character.mode)
                            presenceMode = String(cfg.character.mode)
                        if (cfg.character.reduced_motion !== undefined)
                            reducedMotion = cfg.character.reduced_motion === true
                        if (cfg.character.sleep_timeout_secs !== undefined)
                            sleepAfterSecs = parseInt(cfg.character.sleep_timeout_secs) || 240
                    }
                } catch (e) {}
            }
        }
        xhr.send()
    }

    // === SSE: estado de voz ===
    property var _sseXhr: null
    property int _sseProcessed: 0

    function startVoiceStream() {
        if (_sseXhr) { try { _sseXhr.abort() } catch(e) {} }
        var xhr = new XMLHttpRequest()
        _sseXhr = xhr
        _sseProcessed = 0
        var pendingVoice = ""
        xhr.open("GET", backendUrl + "/api/voice/stream")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === 3 || xhr.readyState === 4) {
                var full = xhr.responseText || ""
                var newPart = full.substring(_sseProcessed)
                _sseProcessed = full.length
                pendingVoice += newPart
                var parts = pendingVoice.split("\n\n")
                pendingVoice = parts.pop()
                for (var i = 0; i < parts.length; i++) {
                    var block = parts[i].trim()
                    if (!block) continue
                    var ev = "", data = ""
                    var lines = block.split("\n")
                    for (var j = 0; j < lines.length; j++) {
                        var ln = lines[j].trim()
                        if (ln.indexOf("event:") === 0) ev = ln.substring(6).trim()
                        else if (ln.indexOf("data:") === 0) data = ln.substring(5).trim()
                    }
                    if (!ev || !data) continue
                    if (ev === "state") {
                        try {
                            var statePayload = JSON.parse(data)
                            voiceState = statePayload.state || "idle"
                            assistantState = voiceState !== "idle" ? voiceState : chatState
                        } catch(e) {}
                    } else if (ev === "level") {
                        try { voiceLevel = Number(JSON.parse(data).level) || 0 } catch(e) {}
                    }
                }
                if (xhr.readyState === 4) {
                    _sseXhr = null
                    // Reconectar tras pequeño delay
                    reconnectTimer.running = true
                }
            }
        }
        xhr.send()
    }

    Timer {
        id: reconnectTimer
        interval: 2000
        repeat: false
        onTriggered: startVoiceStream()
    }

    // El avatar también reacciona a respuestas escritas. La voz tiene
    // prioridad visual cuando ambos flujos coinciden.
    property var _agentXhr: null
    function startAgentStream() {
        if (_agentXhr) { try { _agentXhr.abort() } catch(e) {} }
        var xhr = new XMLHttpRequest()
        _agentXhr = xhr
        var processed = 0
        var pending = ""
        xhr.open("GET", backendUrl + "/api/agent/stream")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== 3 && xhr.readyState !== 4) return
            var full = xhr.responseText || ""
            pending += full.substring(processed)
            processed = full.length
            var parts = pending.split("\n\n")
            pending = parts.pop()
            for (var i = 0; i < parts.length; ++i) {
                var lines = parts[i].split("\n")
                var data = ""
                for (var j = 0; j < lines.length; ++j) {
                    if (lines[j].trim().indexOf("data:") === 0)
                        data += lines[j].trim().substring(5).trim()
                }
                      try {
                          var event = JSON.parse(data)
                          if (event.type) {
                              overlay.wakeAgentVisuals()
                              agent.handleActivity(event)
                          }
                          if (event.state === "processing" || event.state === "idle") {
                              chatState = event.state
                              assistantState = voiceState !== "idle" ? voiceState : chatState
                          }
                } catch(e) {}
            }
            if (xhr.readyState === 4) {
                _agentXhr = null
                agent.resetActivity()
                agentReconnect.running = true
            }
        }
        xhr.send()
    }

    Timer {
        id: agentReconnect
        interval: 2000
        repeat: false
        onTriggered: startAgentStream()
    }

    Component.onCompleted: {
        loadConfig()
        startVoiceStream()
        startAgentStream()
        console.log("AgentOverlay: layer-shell iniciado")
    }
}
