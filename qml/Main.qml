// Main.qml - Ventana principal KDE Assistant v2
// Estetica Apple Design (Cupertino), translucida, frosted glass

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts
import qml 1.0
import qml.components 1.0

ApplicationWindow {
    id: root

    title: "KDE Assistant v2"
    width: Theme.windowDefaultWidth
    height: Theme.windowDefaultHeight
    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    maximumWidth: Theme.windowMaxWidth
    maximumHeight: Theme.windowMaxHeight

    color: "transparent"   // El fondo lo da el Rectangle root

    flags: Qt.Window | Qt.WindowStaysOnTopHint
    visible: true

    // === Modelo de datos real (conectado al backend Rust) ===
    property string backendUrl: "http://127.0.0.1:8765"

    property var sessions: []          // [{id, title, updated_at, message_count}]
    property string currentSessionId: ""

    property var messages: []          // [{role, authorLabel, content, timestamp, isStreaming, toolCalls}]

    // === Helpers ===
    function nowTime() {
        return Qt.formatTime(new Date(), "hh:mm")
    }

    function appendMessage(msg) {
        var arr = messages.slice()
        arr.push(msg)
        messages = arr
    }

    function formatServerResponse(text) {
        return markdownToHtml(text || "")
    }

    // Convierte markdown basico a HTML para renderizar en el chat
    function markdownToHtml(md) {
        if (!md) return ""
        var html = md

        // Escapar HTML primero para evitar inyeccion
        html = html.replace(/&/g, "&amp;")
                   .replace(/</g, "&lt;")
                   .replace(/>/g, "&gt;")

        // Code blocks: ```lang \n ... \n ``` — monoespaciado con wrap
        html = html.replace(/```[\s\S]*?```/g, function(m) {
            var code = m.replace(/```[a-zA-Z]*\n?/g, "").replace(/```/g, "")
            code = code.replace(/\n/g, "<br>")
            return "<font face=\"monospace\" size=\"2\">" + code + "</font>"
        })

        // Inline code: `code`
        html = html.replace(/`([^`]+)`/g, "<code>$1</code>")

        // Bold: **text**
        html = html.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")

        // Italic: *text*
        html = html.replace(/\*([^*]+)\*/g, "<i>$1</i>")

        // Headers: ### text, ## text, # text
        html = html.replace(/^###\s+(.+)$/gm, "<b>$1</b>")
        html = html.replace(/^##\s+(.+)$/gm, "<b>$1</b>")
        html = html.replace(/^#\s+(.+)$/gm, "<b>$1</b>")

        // List items: - item  or  * item
        html = html.replace(/^[-*]\s+(.+)$/gm, "• $1")

        // Line breaks
        html = html.replace(/\n/g, "<br>")

        return html
    }

    // Envia un mensaje al backend con streaming SSE y agrega la respuesta al chat
    function sendMessage(text) {
        if (!text || text.trim().length === 0) return
        if (streaming) return

        // Si no hay sesion, crearla primero y luego enviar
        if (!currentSessionId) {
            var cxhr = new XMLHttpRequest()
            cxhr.open("POST", backendUrl + "/api/session")
            cxhr.setRequestHeader("Content-Type", "application/json")
            cxhr.onreadystatechange = function() {
                if (cxhr.readyState === XMLHttpRequest.DONE && (cxhr.status === 200 || cxhr.status === 201)) {
                    try {
                        var s = JSON.parse(cxhr.responseText)
                        currentSessionId = s.id
                        loadSessions()
                    } catch (e) {}
                    sendMessageStream(text)
                }
            }
            cxhr.send(JSON.stringify({ title: "Nueva conversación" }))
            return
        }
        sendMessageStream(text)
    }

    function sendMessageStream(text) {
        // Mensaje del usuario
        appendMessage({
            role: "user",
            authorLabel: "Tu",
            content: text,
            raw: text,
            timestamp: nowTime(),
            isStreaming: false,
            toolCalls: []
        })

        // Placeholder del asistente en modo streaming
        appendMessage({
            role: "assistant",
            authorLabel: "KDE Assistant",
            content: "",
            raw: "",
            timestamp: nowTime(),
            isStreaming: true,
            toolCalls: []
        })

        streaming = true
        var assistantIdx = messages.length - 1
        var rawText = ""
        var toolArr = []
        var processedLen = 0
        var pending = ""
        var sawError = ""

        function refreshUI(stillStreaming) {
            var arr = messages.slice()
            if (assistantIdx >= 0 && assistantIdx < arr.length) {
                var old = arr[assistantIdx]
                arr[assistantIdx] = {
                    role: "assistant",
                    authorLabel: "KDE Assistant",
                    content: rawText ? markdownToHtml(rawText) : (stillStreaming ? "" : "⚠ Sin respuesta del asistente"),
                    raw: rawText,
                    timestamp: old.timestamp,
                    isStreaming: stillStreaming,
                    toolCalls: toolArr.slice()
                }
                messages = arr
            }
        }

        function processBlock(block) {
            var lines = block.split("\n")
            var ev = ""
            var dataStr = ""
            for (var i = 0; i < lines.length; i++) {
                var ln = lines[i].trim()
                if (ln.indexOf("event:") === 0) ev = ln.substring(6).trim()
                else if (ln.indexOf("data:") === 0) dataStr += ln.substring(5).trim()
            }
            if (!ev || !dataStr) return
            var obj = null
            try { obj = JSON.parse(dataStr) } catch (e) { return }
            if (ev === "token") {
                if (obj && obj.content) rawText += obj.content
            } else if (ev === "tool_call") {
                if (obj) toolArr.push({ id: obj.id || "", name: obj.name || "tool", status: "running", result: "", imageUrl: "", caption: "" })
            } else if (ev === "tool_result") {
                if (obj) {
                    for (var j = 0; j < toolArr.length; j++) {
                        if (toolArr[j].id === obj.tool_call_id) {
                            toolArr[j].status = "success"
                            toolArr[j].result = obj.content || ""
                            if (obj.image_url) toolArr[j].imageUrl = obj.image_url
                            break
                        }
                    }
                }
            } else if (ev === "done") {
                if (obj && obj.full_content && !rawText) rawText = obj.full_content
            } else if (ev === "error") {
                if (obj && obj.message) sawError = obj.message
            }
        }

        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/chat")
        xhr.setRequestHeader("Content-Type", "application/json")
        xhr.onreadystatechange = function() {
            if (xhr.readyState === 3 || xhr.readyState === 4) {
                var full = xhr.responseText || ""
                var newPart = full.substring(processedLen)
                processedLen = full.length
                pending += newPart
                var parts = pending.split("\n\n")
                pending = parts.pop()
                for (var k = 0; k < parts.length; k++) {
                    processBlock(parts[k])
                }
                refreshUI(true)
                if (xhr.readyState === 4) {
                    if (pending && pending.indexOf("event:") >= 0) {
                        processBlock(pending)
                        pending = ""
                    }
                    if (sawError && !rawText) rawText = "⚠ " + sawError
                    streaming = false
                    refreshUI(false)
                }
            }
        }
        var body = JSON.stringify({
            message: text,
            session_id: currentSessionId ? currentSessionId : null
        })
        xhr.send(body)
    }

    // Carga las sesiones del backend (o crea una si no hay)
    function loadSessions() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/sessions")
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                sessions = JSON.parse(xhr.responseText)
                if (sessions.length > 0) {
                    currentSessionId = sessions[0].id
                }
            }
        }
        xhr.send()
    }

    function newSession() {
        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/session")
        xhr.setRequestHeader("Content-Type", "application/json")
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                var s = JSON.parse(xhr.responseText)
                currentSessionId = s.id
                messages = []
                loadSessions()
                drawerOpen = false
            }
        }
        xhr.send(JSON.stringify({ title: "Nueva conversación" }))
    }

    // Elimina una sesion en el backend y refresca la lista
    function deleteSession(sessionId) {
        var xhr = new XMLHttpRequest()
        xhr.open("DELETE", backendUrl + "/api/session?session_id=" + encodeURIComponent(sessionId))
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                if (currentSessionId === sessionId) {
                    currentSessionId = ""
                    messages = []
                }
                loadSessions()
            }
        }
        xhr.send()
    }

    // Carga los mensajes de una sesion desde el backend
    function loadMessages(sessionId) {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/messages?session_id=" + encodeURIComponent(sessionId))
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                var list = JSON.parse(xhr.responseText)
                var arr = []
                for (var i = 0; i < list.length; i++) {
                    var m = list[i]
                    // Las imagenes (role tool con image_url) se adjuntan como
                    // toolCalls del assistant anterior para que se vean en el chat
                    if (m.role === "tool" && m.image_url) {
                        for (var k = arr.length - 1; k >= 0; k--) {
                            if (arr[k].role === "assistant") {
                                arr[k].toolCalls.push({
                                    id: "", name: "show_image", status: "success",
                                    result: m.content || "", imageUrl: m.image_url, caption: ""
                                })
                                break
                            }
                        }
                        continue
                    }
                    if (m.role !== "user" && m.role !== "assistant") continue
                    arr.push({
                        role: m.role,
                        authorLabel: m.role === "user" ? "Tu" : "KDE Assistant",
                        content: m.role === "assistant" ? markdownToHtml(m.content) : m.content,
                        raw: m.content,
                        timestamp: "",
                        isStreaming: false,
                        toolCalls: []
                    })
                }
                messages = arr
            }
        }
        xhr.send()
    }

    // === Estado UI ===
    property bool drawerOpen: false
    property bool streaming: false
    property string voiceState: "idle"  // "idle" | "listening" | "processing" | "speaking"

    Component.onCompleted: loadSessions()

    // === Background frosted ===
    // Esquinas exteriores cuadradas para asentar en la decoracion Breeze;
    // los radios viven en cards, burbujas e InputBar.
    Rectangle {
        id: bgRect
        anchors.fill: parent
        color: Theme.canvas
        radius: 0
        border.width: 1
        border.color: Theme.hairline
    }

    // === Layout principal ===
    Item {
        anchors.fill: parent
        anchors.margins: 1

        // === Drawer lateral (colapsable) ===
        SessionDrawer {
            id: drawer
            x: root.drawerOpen ? 0 : -drawer.drawerWidth
            y: 0
            width: drawer.drawerWidth
            height: parent.height
            drawerOpen: root.drawerOpen
            sessions: root.sessions
            currentId: root.currentSessionId
            isDark: Theme.isDark
            onNewSessionClicked: {
                root.newSession()
            }
            onSessionSelected: function(id) {
                root.currentSessionId = id
                root.loadMessages(id)
                root.drawerOpen = false
            }
            onSessionDeleteRequested: function(id) {
                root.deleteSession(id)
            }
            onSettingsClicked: {
                root.drawerOpen = false
                settings.show()
            }
        }

        // === Main content area ===
        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: root.drawerOpen ? drawer.drawerWidth : 0
            anchors.rightMargin: 0
            anchors.topMargin: 0
            anchors.bottomMargin: 0
            spacing: 0

            Behavior on anchors.leftMargin {
                NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
            }

            // === Title bar ===
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: 40

                // Toggle drawer
                IconButton {
                    id: toggleBtn
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.leftMargin: Theme.spacingXs
                    iconName: root.drawerOpen ? "sidebar-collapse-16" : "sidebar-expand-16"
                    iconSize: 18
                    buttonSize: 32
                    backgroundColor: "transparent"
                    iconColor: Theme.ink
                    onClicked: root.drawerOpen = !root.drawerOpen
                }

                // Title
                Text {
                    anchors.centerIn: parent
                    text: qsTr("KDE Assistant")
                    font: Theme.font(Theme.fontSizeBodyStrong, Theme.weightBold, -0.1)
                    color: Theme.ink
                }

                // Status indicator (conectado a backend)
                Row {
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.rightMargin: Theme.spacingMd
                    spacing: 6

                    Rectangle {
                        width: 6; height: 6; radius: 3
                        color: Theme.success
                        anchors.verticalCenter: parent.verticalCenter
                        SequentialAnimation on opacity {
                            loops: Animation.Infinite
                            NumberAnimation { from: 1.0; to: 0.4; duration: 1200 }
                            NumberAnimation { from: 0.4; to: 1.0; duration: 1200 }
                        }
                    }
                    Text {
                        text: qsTr("Online")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
            }

            // === Voice Orb (centrado, visible cuando no es idle) ===
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: root.voiceState === "idle" ? 0 : 180
                visible: root.voiceState !== "idle"

                Behavior on Layout.preferredHeight {
                    NumberAnimation { duration: Theme.animNormal; easing.type: Easing.OutCubic }
                }

                VoiceOrb {
                    anchors.centerIn: parent
                    state: root.voiceState
                    amplitude: root.voiceLevel
                    size: 120
                    onClicked: {
                        if (root.voiceState === "speaking") {
                            bargeIn()
                            root.voiceState = "listening"
                        } else if (root.voiceState !== "idle") {
                            root.voiceState = "idle"
                        }
                    }
                }
            }

            // === ChatView ===
            ChatView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                messages: root.messages
                onSuggestionClicked: function(text) {
                    root.sendMessage(text)
                }
                onImageClicked: function(url, caption) {
                    imagePreview.show(url, caption)
                }
                onCopyRequested: function(text) {
                    root.copyToClipboard(text)
                }
                onPlayRequested: function(text) {
                    root.speakText(text)
                }
            }

            // Buffer oculto para copiar al portapapeles
            // (QML no expone QClipboard; se usa selectAll+copy).
            TextEdit {
                id: clipBuffer
                visible: false
                width: 0
                height: 0
            }

            // === Input bar (flotante) ===
            // Altura sigue al InputBar (auto-crece hasta 4 lineas)
            InputBar {
                id: inputBar
                Layout.fillWidth: true
                Layout.margins: Theme.spacingSm
                Layout.preferredHeight: implicitHeight
                streaming: root.streaming
                recording: root.voiceState === "listening"
                onSendClicked: function(text) {
                    root.sendMessage(text)
                }
                onMicClicked: {
                    root.voiceState = root.voiceState === "listening" ? "idle" : "listening"
                }
                onStopClicked: {
                    root.streaming = false
                    root.voiceState = "idle"
                }
            }
        }
    }

    // === Image preview dialog (overlay) ===
    ImagePreviewDialog {
        id: imagePreview
        anchors.fill: parent
        open_: false
    }

    // === Settings dialog (overlay) ===
    SettingsDialog {
        id: settings
        anchors.fill: parent
        open_: false
        backendUrl: root.backendUrl
        onClosed: console.log("Settings closed")
        onSaved: console.log("Settings saved")
    }

    // === System Tray (KDE Plasma) ===
    TrayMenu {
        id: tray
        windowVisible: root.visible
        backendUrl: root.backendUrl
        onShowRequested: {
            root.show()
            root.raise()
            root.requestActivate()
        }
        onHideRequested: root.hide()
        onNewSessionRequested: {
            root.newSession()
            root.show()
            root.raise()
        }
        onOpenSessionRequested: function(sessionId) {
            root.currentSessionId = sessionId
            root.loadMessages(sessionId)
            root.show()
            root.raise()
            root.requestActivate()
        }
        onSettingsRequested: {
            settings.show()
            root.show()
            root.raise()
        }
        onQuitRequested: Qt.quit()
    }

    // === Burbuja flotante (ventana independiente, visible aunque Main este oculta) ===
    FloatingOrb {
        id: floatingOrb
        voiceState: root.voiceState
        amplitude: root.voiceLevel
    }

    // === Hotkey polling: lee ~/.cache/kde-assistant/hotkey.state ===
    // El backend Rust (rdev) escribe a este archivo cuando detecta
    // Super+Shift+A (toggle), Super+Shift+V mantenido (PTT start/end),
    // Ctrl+Shift+K (new session). El backend ya graba/procesa el audio;
    // aqui solo se refleja el estado visual (VoiceOrb).
    property string hotkeyStamp: ""
    property string lastHotkeyStamp: ""
    property string lastVoiceStamp: ""
    property string cacheBase: {
        // Qt6: intentar Qt.platform.environment, fallback a HOME hardcodeada
        var env = Qt.platform.environment
        var home = env ? env["HOME"] : null
        if (!home) home = "/root"
        return "file://" + home + "/.cache/kde-assistant/"
    }
    property string homePath: cacheBase + "hotkey.state"
    property string voicePath: cacheBase + "voice.state"
    property string voiceLevelPath: cacheBase + "voice.level"
    property real voiceLevel: 0.0
    Timer {
        id: hotkeyTimer
        interval: 300
        running: true
        repeat: true
        onTriggered: {
            pollHotkeys()
            pollVoiceState()
        }
    }
    Timer {
        id: levelTimer
        interval: 80
        running: root.voiceState === "listening"
        repeat: true
        onTriggered: pollVoiceLevel()
    }

    function bargeIn() {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/voice/barge_in")
        req.setRequestHeader("Content-Type", "application/json")
        req.send(JSON.stringify({}))
        console.log("Barge-in solicitado")
    }

    function pollHotkeys() {
        var req = new XMLHttpRequest()
        req.open("GET", root.homePath + "?t=" + Date.now())
        req.onreadystatechange = function() {
            if (req.readyState === 4) {
                if (req.status === 200 || req.status === 0) {
                    var content = req.responseText.trim()
                    if (content && content !== root.lastHotkeyStamp) {
                        root.lastHotkeyStamp = content
                        var parts = content.split("|")
                        var action = parts[0]
                        console.log("Hotkey recibido:", action)
                        if (action === "toggle_window") {
                            root.visible = !root.visible
                        } else if (action === "push_to_talk_start") {
                            if (root.voiceState === "speaking") bargeIn()
                            root.voiceState = "listening"
                        } else if (action === "push_to_talk_end") {
                            root.voiceState = "idle"
                        } else if (action === "push_to_talk") {
                            if (root.voiceState === "speaking") {
                                bargeIn()
                                root.voiceState = "listening"
                            } else {
                                root.voiceState = root.voiceState === "listening" ? "idle" : "listening"
                            }
                        } else if (action === "new_session") {
                            root.currentSessionId = ""
                        }
                    }
                }
            }
        }
        req.send()
    }

    function pollVoiceLevel() {
        var req = new XMLHttpRequest()
        req.open("GET", root.voiceLevelPath + "?t=" + Date.now())
        req.onreadystatechange = function() {
            if (req.readyState === 4 && (req.status === 200 || req.status === 0)) {
                var v = parseFloat(req.responseText.trim())
                if (!isNaN(v)) root.voiceLevel = Math.max(0, Math.min(1, v))
            }
        }
        req.send()
    }

    // El backend escribe listening|processing|speaking|idle con timestamp.
    // Solo los estados nuevos pisan el voiceState local.
    property string lastVoiceCycle: "0"   // timestamp_ms ya mostrado en el chat
    function pollVoiceState() {
        var req = new XMLHttpRequest()
        req.open("GET", root.voicePath + "?t=" + Date.now())
        req.onreadystatechange = function() {
            if (req.readyState === 4) {
                if (req.status === 200 || req.status === 0) {
                    var content = req.responseText.trim()
                    if (content && content !== root.lastVoiceStamp) {
                        root.lastVoiceStamp = content
                        var state = content.split("|")[0]
                        if (state === "listening" || state === "processing"
                                || state === "speaking" || state === "idle") {
                            var was = root.voiceState
                            root.voiceState = state
                            // Al invocar por voz la app se abre aunque este minimizada
                            if (state === "listening" && was !== "listening") {
                                root.show()
                                root.raise()
                                root.requestActivate()
                            }
                            // Al terminar un ciclo de voz, traer el intercambio al chat
                            if (state === "idle" && was !== "idle") {
                                fetchVoiceExchange()
                            }
                        }
                    }
                }
            }
        }
        req.send()
    }

    // Trae el ultimo intercambio por voz y lo agrega al chat actual.
    function fetchVoiceExchange() {
        var req = new XMLHttpRequest()
        req.open("GET", backendUrl + "/api/voice/last")
        req.onreadystatechange = function() {
            if (req.readyState === XMLHttpRequest.DONE && xhr_ok(req)) {
                try {
                    var ex = JSON.parse(req.responseText)
                    var stamp = String(ex.timestamp_ms || 0)
                    if (!ex.transcript || stamp === "0" || stamp === root.lastVoiceCycle) return
                    root.lastVoiceCycle = stamp
                    appendMessage({
                        role: "user",
                        authorLabel: "Tu (voz)",
                        content: ex.transcript,
                        raw: ex.transcript,
                        timestamp: nowTime(),
                        isStreaming: false,
                        toolCalls: []
                    })
                    if (ex.response) {
                        appendMessage({
                            role: "assistant",
                            authorLabel: "KDE Assistant",
                            content: markdownToHtml(ex.response),
                            raw: ex.response,
                            timestamp: nowTime(),
                            isStreaming: false,
                            toolCalls: []
                        })
                    }
                    logVoiceExchange(ex.transcript, ex.response || "")
                } catch (e) {}
            }
        }
        req.send()
    }

    function xhr_ok(req) {
        return req.status === 200 || req.status === 201 || req.status === 0
    }

    // Persiste el intercambio por voz en la sesion actual (la crea si no hay).
    function logVoiceExchange(transcript, response) {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/voice/log")
        req.setRequestHeader("Content-Type", "application/json")
        req.onreadystatechange = function() {
            if (req.readyState === XMLHttpRequest.DONE && xhr_ok(req)) {
                try {
                    var resp = JSON.parse(req.responseText)
                    if (resp.session_id) {
                        if (!currentSessionId) currentSessionId = resp.session_id
                        loadSessions()
                    }
                } catch (e) {}
            }
        }
        req.send(JSON.stringify({
            transcript: transcript,
            response: response,
            session_id: currentSessionId ? currentSessionId : null
        }))
    }

    // Texto plano apto para TTS (sin marcas markdown/HTML).
    function stripMarkdownForSpeech(md) {
        if (!md) return ""
        var s = md
        s = s.replace(/```[\s\S]*?```/g, " ")
        s = s.replace(/`([^`]*)`/g, "$1")
        s = s.replace(/\*\*([^*]*)\*\*/g, "$1")
        s = s.replace(/\*([^*]*)\*/g, "$1")
        s = s.replace(/^#{1,6}\s+/gm, "")
        s = s.replace(/^[-*]\s+/gm, "")
        s = s.replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
        s = s.replace(/<[^>]*>/g, " ")
        s = s.replace(/\s+/g, " ").trim()
        return s
    }

    // Reproduce un texto via el backend (boton reproducir).
    function speakText(rawText) {
        var text = stripMarkdownForSpeech(rawText)
        if (!text) return
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/speak")
        req.setRequestHeader("Content-Type", "application/json")
        req.send(JSON.stringify({ text: text }))
    }

    // Copia al portapapeles via buffer oculto.
    function copyToClipboard(text) {
        clipBuffer.text = text || ""
        clipBuffer.selectAll()
        clipBuffer.copy()
        clipBuffer.deselect()
    }
}
