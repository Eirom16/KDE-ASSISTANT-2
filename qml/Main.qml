// Main.qml - Ventana principal KDE Assistant v2
// Estetica Apple Design (Cupertino), translucida, frosted glass

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts
import qml 1.0
import qml.components 1.0
import qml.auth 1.0
import qml.agent 1.0

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

    // Cerrar la ventana NO apaga el asistente: se oculta y sigue en la
    // bandeja (wake word / hotkeys / recordatorios siguen vivos). La salida
    // real es tray "Salir" (quitRequested -> Qt.quit()).
    onClosing: function(close) {
        close.accepted = false
        root.hide()
    }


    // === Modelo de datos real (conectado al backend Rust) ===
    property string backendUrl: "http://127.0.0.1:8765"
    // Token local: módulo generado por main.rs (qml.auth) y, como
    // respaldo, env KDE_ASSISTANT_TOKEN (Qt.platform.environment es null
    // en algunas sesiones qml6: sin token todo da 401).
    property string authToken: {
        try {
            if (AuthToken.token) return String(AuthToken.token)
        } catch (e) {}
        try {
            var env = Qt.platform.environment
            var t = env ? env["KDE_ASSISTANT_TOKEN"] : null
            return t ? String(t) : ""
        } catch (err) { return "" }
    }
    function setAuth(xhr) {
        if (authToken !== "") xhr.setRequestHeader("Authorization", "Bearer " + authToken)
    }

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

    // Maneja la subida de archivos (texto/imagen) desde el InputBar
    function handleFileUpload(filePath, fileType) {
        console.log("Archivo seleccionado:", filePath, "tipo:", fileType)
        if (fileType === "image") {
            // Para imagenes, usamos la tool show_image
            var xhr = new XMLHttpRequest()
            xhr.open("POST", backendUrl + "/api/tools/execute")
            xhr.setRequestHeader("Content-Type", "application/json")
            setAuth(xhr)
            xhr.onreadystatechange = function() {
                if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                    try {
                        var result = JSON.parse(xhr.responseText)
                        if (result.image_url) {
                            // La tool show_image ya inyecta la imagen en el chat
                            root.sendMessage("[Imagen subida: " + filePath + "]")
                        }
                    } catch (e) {
                        console.error("Error subiendo imagen:", e)
                    }
                }
            }
            xhr.send(JSON.stringify({
                name: "show_image",
                arguments: JSON.stringify({ source: filePath, caption: "" }),
                session_id: currentSessionId ? currentSessionId : null
            }))
        } else {
            // Para archivos de texto, leemos el contenido y lo enviamos como mensaje
            var file = new XMLHttpRequest()
            file.open("GET", "file://" + filePath)
            file.onreadystatechange = function() {
                if (file.readyState === XMLHttpRequest.DONE && file.status === 200) {
                    var content = file.responseText
                    var preview = content.length > 500 ? content.substring(0, 500) + "..." : content
                    root.sendMessage("[Archivo: " + filePath + "]\n```\n" + preview + "\n```")
                }
            }
            file.send()
        }
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
            setAuth(cxhr)
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
        streamAgent(backendUrl + "/api/chat", {
            message: text,
            session_id: currentSessionId ? currentSessionId : null
        }, text)
    }

    // F1-1: regenera la última respuesta (también sirve de "reintentar"
    // tras un tool en error). No duplica el mensaje del usuario: quita los
    // assistants locales en cola y el backend borra el trailing en DB.
    function regenerate() {
        if (!currentSessionId || streaming) return
        var arr = messages.slice()
        while (arr.length > 0 && arr[arr.length - 1].role === "assistant") arr.pop()
        messages = arr
        streamAgent(backendUrl + "/api/chat/regenerate", {
            session_id: currentSessionId
        }, null)
    }

    // Motor SSE genérico: opcionalmente agrega el mensaje del usuario,
    // siempre agrega el placeholder del asistente y streamea tokens/tools.
    function streamAgent(url, bodyObj, userText) {
        if (userText !== null && userText !== undefined) {
            // Mensaje del usuario
            appendMessage({
                role: "user",
                authorLabel: "Tu",
                content: userText,
                raw: userText,
                timestamp: nowTime(),
                isStreaming: false,
                toolCalls: []
            })
        }

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
                            var denied = obj.content && (obj.content.indexOf("El usuario denegó") === 0 || obj.content.indexOf("Acción sensible denegada") === 0)
                            toolArr[j].status = denied ? "error" : "success"
                            toolArr[j].result = denied ? qsTr("Denegada por ti") : (obj.content || "")
                            if (!denied && obj.image_url) toolArr[j].imageUrl = obj.image_url
                            break
                        }
                    }
                    // Sin entrada previa (p.ej. regenerate): crearla como éxito.
                    var found = false
                    for (var m = 0; m < toolArr.length; m++) {
                        if (toolArr[m].id === obj.tool_call_id) { found = true; break }
                    }
                    if (!found && obj.tool_call_id) {
                        toolArr.push({ id: obj.tool_call_id, name: "tool", status: "success", result: obj.content || "", imageUrl: obj.image_url || "", caption: "" })
                    }
                }
            } else if (ev === "approval_needed") {
                // F4-1: la tool espera tu decisión (botones en el badge).
                if (obj) {
                    for (var n = 0; n < toolArr.length; n++) {
                        if (toolArr[n].id === obj.tool_call_id) {
                            toolArr[n].status = "confirm"
                            toolArr[n].result = qsTr("Requiere confirmación") + (obj.permission ? " (" + obj.permission + ")" : "")
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
        xhr.open("POST", url)
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        activeChatXhr = xhr
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
                    // F0-3: el middleware devuelve 401 JSON (no SSE) si falta el Bearer.
                    if (xhr.status === 401) {
                        rawText = qsTr("⚠ No autorizado (token local). Reinicia la app.")
                        lastError = qsTr("No autorizado")
                        lastErrorDetail = qsTr("Falta el token local. Reinicia kde-assistant (main.rs lo inyecta vía KDE_ASSISTANT_TOKEN).")
                        showErrorBanner = true
                        streaming = false
                        activeChatXhr = null
                        refreshUI(false)
                        return
                    }
                    if (pending && pending.indexOf("event:") >= 0) {
                        processBlock(pending)
                        pending = ""
                    }
                    if (sawError && !rawText) {
                        rawText = "⚠ " + sawError
                        lastError = qsTr("Error del asistente")
                        lastErrorDetail = sawError
                        showErrorBanner = true
                    }
                    streaming = false
                    // Re-chequear salud tras error (puede ser 401/429).
                    if (sawError) checkBackend()
                    activeChatXhr = null
                    refreshUI(false)
                }
            }
        }
        var body = JSON.stringify(bodyObj)
        xhr.send(body)
    }

    // Carga las sesiones del backend (o crea una si no hay)
    function loadSessions() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/sessions")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.status === 401 && xhr.readyState === XMLHttpRequest.DONE) {
                lastError = qsTr("No autorizado")
                lastErrorDetail = qsTr("Token local inválido. Reinicia la app.")
                showErrorBanner = true
                return
            }
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                try {
                    sessions = JSON.parse(xhr.responseText)
                } catch (e) { sessions = [] }
                // Solo auto-seleccionar si no hay selección o ya no existe.
                var exists = false
                for (var i = 0; i < sessions.length; i++) {
                    if (sessions[i].id === currentSessionId) { exists = true; break }
                }
                if (!exists) {
                    if (sessions.length > 0) {
                        currentSessionId = sessions[0].id
                        loadMessages(currentSessionId)
                    } else {
                        currentSessionId = ""
                    }
                }
            } else if (xhr.readyState === XMLHttpRequest.DONE) {
                backendOnline = false
                backendStatus = "offline"
            }
        }
        xhr.send()
    }

    function newSession() {
        // F0-7: cancelar streaming en curso antes de cambiar de sesión.
        cancelChat()
        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/session")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
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
        setAuth(xhr)
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

    // Hora real de un timestamp RFC3339 ("2026-09-09T12:34:56+...") → "hh:mm".
    function fmtTime(iso) {
        if (!iso) return ""
        try {
            var d = new Date(iso)
            if (isNaN(d.getTime())) return ""
            return Qt.formatTime(d, "hh:mm")
        } catch (e) { return "" }
    }

    // Carga los mensajes de una sesion desde el backend
    function loadMessages(sessionId) {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/messages?session_id=" + encodeURIComponent(sessionId))
        setAuth(xhr)
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
                        timestamp: fmtTime(m.timestamp),
                        isStreaming: false,
                        toolCalls: []
                    })
                }
                messages = arr
            }
        }
        xhr.send()
    }

    // F1-2: renombra una sesión (PATCH /api/session).
    function renameSession(sessionId, title) {
        var t = (title || "").trim()
        if (!t) return
        var xhr = new XMLHttpRequest()
        xhr.open("PATCH", backendUrl + "/api/session")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && (xhr.status === 200 || xhr.status === 201)) {
                loadSessions()
            }
        }
        xhr.send(JSON.stringify({ id: sessionId, title: t }))
    }

    // === Estado UI ===
    property bool drawerOpen: false
    property bool streaming: false
    property string voiceState: "idle"  // "idle" | "listening" | "processing" | "speaking"
    // XHR del chat en curso (F0-7: para abortar en Stop / nueva sesión).
    property var activeChatXhr: null
    // Estado backend (F0-5): se actualiza con /api/health + /api/config.
    property bool backendOnline: false
    property string backendStatus: "offline" // offline | online | nokey
    property string lastError: ""
    property string lastErrorDetail: ""
    property bool showErrorBanner: false
    property string lastModel: ""
    property string lastProvider: ""
    // F2-1: ventana siempre visible (sale de config ui.always_on_top).
    property bool alwaysOnTop: true
    property string pttShortcut: "Super+Shift+V"
    // Desktop Agent (personaje): cfg.character.* — opt-in (enabled=false por
    // defecto). Ver DESKTOP-AGENT-DESIGN.md.
    property bool characterEnabled: false
    property string characterMode: "companion"
    property bool characterReducedMotion: false
    property int characterSleepSecs: 240
    property int characterSize: 140

    // Prepara el input con un texto y abre la ventana (acciones rápidas del tray).
    function prefill(text) {
        inputBar.text = text
        root.show()
        root.raise()
        root.requestActivate()
        inputBar.focusInput()
    }

    // F2-1: alterna siempre-visible y lo persiste en config.
    function toggleAlwaysOnTop(on) {
        root.alwaysOnTop = on
        root.flags = on ? (Qt.Window | Qt.WindowStaysOnTopHint) : Qt.Window
        persistUiFlag("always_on_top", on)
    }

    function persistUiFlag(key, value) {
        var g = new XMLHttpRequest()
        g.open("GET", backendUrl + "/api/config")
        setAuth(g)
        g.onreadystatechange = function() {
            if (g.readyState === XMLHttpRequest.DONE && g.status === 200) {
                try {
                    var cfg = JSON.parse(g.responseText)
                    if (!cfg.ui) cfg.ui = {}
                    cfg.ui[key] = value
                    var p = new XMLHttpRequest()
                    p.open("POST", backendUrl + "/api/config")
                    p.setRequestHeader("Content-Type", "application/json")
                    setAuth(p)
                    p.send(JSON.stringify(cfg))
                } catch (e) {}
            }
        }
        g.send()
    }

    // F4-1: resuelve una confirmación pendiente (botones del badge).
    function approveTool(toolCallId, approved) {
        if (!toolCallId) return
        // Optimista: marcar el badge mientras el backend reanuda al agente.
        var arr = messages.slice()
        for (var i = 0; i < arr.length; i++) {
            var tcs = arr[i].toolCalls || []
            for (var j = 0; j < tcs.length; j++) {
                if (tcs[j].id === toolCallId && tcs[j].status === "confirm") {
                    tcs[j].status = approved ? "running" : "error"
                    tcs[j].result = approved ? qsTr("Aprobada, ejecutando…") : qsTr("Denegada por ti")
                }
            }
        }
        messages = arr
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/tools/approve")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.send(JSON.stringify({ tool_call_id: toolCallId, approved: approved }))
    }

    // F0-7: cancela el streaming local (xhr.abort) + backend (/api/chat/cancel).
    function cancelChat() {
        if (activeChatXhr) {
            try { activeChatXhr.abort() } catch (e) {}
            activeChatXhr = null
        }
        if (streaming) {
            streaming = false
            // Marcar el placeholder como cancelado si sigue en streaming.
            var arr = messages.slice()
            for (var i = arr.length - 1; i >= 0; i--) {
                if (arr[i].isStreaming) {
                    arr[i] = {
                        role: "assistant",
                        authorLabel: "KDE Assistant",
                        content: qsTr("Cancelado por el usuario."),
                        raw: "Cancelado por el usuario.",
                        timestamp: arr[i].timestamp,
                        isStreaming: false,
                        toolCalls: arr[i].toolCalls || []
                    }
                    break
                }
            }
            messages = arr
        }
        var c = new XMLHttpRequest()
        c.open("POST", backendUrl + "/api/chat/cancel")
        c.setRequestHeader("Content-Type", "application/json")
        setAuth(c)
        c.send(JSON.stringify({ session_id: currentSessionId ? currentSessionId : null }))
    }

    Component.onCompleted: {
        root.startupTime = Date.now()
        // Centrar al arrancar: sin x/y el WM la deja arriba-izquierda.
        // Asignacion unica (no binding) para no pelear con el usuario.
        root.x = Math.round((Screen.desktopAvailableWidth - root.width) / 2)
        root.y = Math.round((Screen.desktopAvailableHeight - root.height) / 2)
        console.log("auth token len:", authToken.length, "| cache:", cacheBase)
        loadSessions()
        checkBackend()
        applyTheme()
        startVoiceStream()
        // F0-6: si cacheBase quedó vacío (sin HOME), desactivar polling.
        if (!cacheBase) {
            console.warn("HOME no disponible: polling de hotkey/voz desactivado")
            filePollingEnabled = false
        }
        // Rescate: si la ventana quedó oculta (recreación por flags),
        // forzar visible + diagnóstico de geometría.
        rescueTimer.start()
    }
    Timer {
        id: rescueTimer
        interval: 1500
        repeat: false
        onTriggered: {
            if (!root.visible) {
                console.warn("ventana oculta al arrancar: forzando show")
                root.show()
                root.raise()
                root.requestActivate()
            }
            console.log("ventana:", root.visible, root.width + "x" + root.height, "en", root.x + "," + root.y)
        }
    }

    function backendStatusText() {
        if (backendStatus === "online") return qsTr("Online")
        if (backendStatus === "nokey") return qsTr("Sin key")
        return qsTr("Offline")
    }

    function providerLabel() {
        var p = (lastProvider || "").toLowerCase()
        if (p === "groq") return "Groq"
        if (p === "openai") return "OpenAI"
        if (p === "custom") return qsTr("Custom")
        if (p) return "OpenRouter"
        return ""
    }

    function modelShort() {
        if (!lastModel) return ""
        var parts = String(lastModel).split("/")
        return parts[parts.length - 1]
    }

    function backendStatusColor() {
        if (backendStatus === "online") return Theme.success
        if (backendStatus === "nokey") return Theme.warn
        return Theme.error
    }

    // Comprueba salud del backend y si hay API key configurada.
    function checkBackend() {
        var h = new XMLHttpRequest()
        h.open("GET", backendUrl + "/api/health")
        // /health no requiere auth, pero si hay token lo enviamos igual.
        setAuth(h)
        h.onreadystatechange = function() {
            if (h.readyState === XMLHttpRequest.DONE) {
                if (h.status === 200) {
                    backendOnline = true
                    // Ver si hay key: GET config (no muestra la key, solo si vacía).
                    var c = new XMLHttpRequest()
                    c.open("GET", backendUrl + "/api/config")
                    setAuth(c)
                    c.onreadystatechange = function() {
                        if (c.readyState === XMLHttpRequest.DONE) {
                            if (c.status === 401) {
                                // El token del QML no coincide con el del backend
                                // (p.ej. otra instancia vieja ocupa el puerto).
                                backendStatus = "offline"
                                lastError = qsTr("No autorizado")
                                lastErrorDetail = qsTr("Token local inválido. Cierra otras instancias (`pkill -f kde-assistant`) y reinicia.")
                                showErrorBanner = true
                                return
                            }
                            if (c.status !== 200) return
                            try {
                                var cfg = JSON.parse(c.responseText)
                                var key = (cfg.ai && cfg.ai.api_key) ? String(cfg.ai.api_key).trim() : ""
                                var model = (cfg.ai && cfg.ai.model) ? String(cfg.ai.model) : ""
                                lastModel = model
                                lastProvider = (cfg.ai && cfg.ai.provider) ? String(cfg.ai.provider) : ""
                                backendStatus = key !== "" ? "online" : "nokey"
                                if (key === "") {
                                    lastError = qsTr("Falta API key")
                                    lastErrorDetail = qsTr("Abre Configuración y pon tu key de Groq/OpenRouter/OpenAI.")
                                    showErrorBanner = true
                                } else if (showErrorBanner && lastError === qsTr("Falta API key")) {
                                    showErrorBanner = false
                                }
                            } catch (e) {
                                backendStatus = "online"
                            }
                        }
                    }
                    c.send()
                } else if (h.status === 0) {
                    backendOnline = false
                    backendStatus = "offline"
                    lastError = qsTr("Sin ruta al backend")
                    lastErrorDetail = qsTr("El QML no llega a 127.0.0.1:8765. Revisa el log del backend.")
                    showErrorBanner = true
                } else {
                    backendOnline = false
                    backendStatus = "offline"
                    lastError = qsTr("Backend no disponible")
                    lastErrorDetail = qsTr("No se pudo contactar 127.0.0.1:8765. ¿Está corriendo kde-assistant?")
                    showErrorBanner = true
                }
            }
        }
        // Timeout manual: si no responde en 4s, marcar offline.
        h.send()
    }

    // Aplica tema + flags de ventana desde backend (ui.theme, ui.always_on_top).
    // system → respeta el actual de Theme (por defecto dark) para no parpadear;
    // dark/light fuerzan. Se re-aplica al guardar Settings.
    function applyTheme() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/config")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    var cfg = JSON.parse(xhr.responseText)
                    var t = cfg.ui && cfg.ui.theme ? String(cfg.ui.theme) : "system"
                    if (t === "dark") Theme.isDark = true
                    else if (t === "light") Theme.isDark = false
                    // system: no forzar; el usuario puede alternar desde Breeze
                    // (fase 2: leer portal color-scheme vía backend y exponerlo aquí).
                    if (cfg.ui && cfg.ui.always_on_top !== undefined) {
                        var wantTop = cfg.ui.always_on_top !== false
                        // Cambiar flags recrea la ventana nativa y pierde la
                        // posicion (salta arriba-izquierda): solo tocar si
                        // cambia, re-mostrar y re-centrar despues.
                        if (root.alwaysOnTop !== wantTop) {
                            root.alwaysOnTop = wantTop
                            root.flags = wantTop ? (Qt.Window | Qt.WindowStaysOnTopHint) : Qt.Window
                            root.show()
                            root.raise()
                            root.x = Math.round((Screen.desktopAvailableWidth - root.width) / 2)
                            root.y = Math.round((Screen.desktopAvailableHeight - root.height) / 2)
                            root.requestActivate()
                        }
                    }
                    if (cfg.shortcuts && cfg.shortcuts.push_to_talk) {
                        root.pttShortcut = String(cfg.shortcuts.push_to_talk)
                    }
                    // Desktop Agent (character.*): opt-in; campos con
                    // fallback al default si la seccion no existe.
                    if (cfg.character) {
                        if (cfg.character.enabled !== undefined)
                            root.characterEnabled = cfg.character.enabled === true
                        if (cfg.character.mode)
                            root.characterMode = String(cfg.character.mode)
                        if (cfg.character.reduced_motion !== undefined)
                            root.characterReducedMotion = cfg.character.reduced_motion === true
                        if (cfg.character.sleep_timeout_secs !== undefined)
                            root.characterSleepSecs = parseInt(cfg.character.sleep_timeout_secs) || 240
                        if (cfg.character.size !== undefined)
                            root.characterSize = parseInt(cfg.character.size) || 140
                    }
                } catch (e) {}
            }
        }
        xhr.send()
    }

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
            onSessionRenameRequested: function(id, title) {
                root.renameSession(id, title)
            }
            onAuditRequested: {
                auditDialog.show()
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

                // Status indicator (refleja /api/health + key).
                // Item con ancho acotado + clip: el texto largo (proveedor +
                // modelo) nunca se sale del campo visible.
                Item {
                    anchors.right: menuBtn.left
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.rightMargin: Theme.spacingXs
                    width: Math.min(statusRow.implicitWidth, parent.width * 0.5)
                    height: 20
                    clip: true

                    Row {
                        id: statusRow
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6

                        Rectangle {
                            width: 6; height: 6; radius: 3
                            color: backendStatusColor()
                            anchors.verticalCenter: parent.verticalCenter
                            SequentialAnimation on opacity {
                                running: root.backendStatus === "online"
                                loops: Animation.Infinite
                                NumberAnimation { from: 1.0; to: 0.4; duration: 1200 }
                                NumberAnimation { from: 0.4; to: 1.0; duration: 1200 }
                            }
                        }
                        Text {
                            text: {
                                var t = backendStatusText()
                                var prov = providerLabel()
                                var mod = modelShort()
                                if (prov && mod) return t + " · " + prov + " · " + mod
                                if (mod) return t + " · " + mod
                                return t
                            }
                            font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            anchors.verticalCenter: parent.verticalCenter
                            elide: Text.ElideRight
                            maximumLineCount: 1
                        }
                    }

                    // Reintentar health al hacer click
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.checkBackend()
                    }
                }

                // Menu del tray (kebab): abre el mismo popup que el click
                // derecho en el icono, por si la sesion no lo entrega.
                IconButton {
                    id: menuBtn
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.rightMargin: Theme.spacingXs
                    iconName: "kebab-horizontal-16"
                    iconSize: 16
                    buttonSize: 28
                    backgroundColor: "transparent"
                    iconColor: Theme.inkMuted
                    onClicked: tray.openMenu()
                }
            }

            // === Error banner (F0-5: antes muerto, ahora cableado) ===
            ErrorBanner {
                Layout.fillWidth: true
                Layout.leftMargin: Theme.spacingMd
                Layout.rightMargin: Theme.spacingMd
                Layout.topMargin: Theme.spacingXs
                Layout.bottomMargin: 0
                visible: root.showErrorBanner
                message: root.lastError
                detail: root.lastErrorDetail
                retryable: true
                onRetryClicked: {
                    root.showErrorBanner = false
                    root.checkBackend()
                }
                onDismissed: root.showErrorBanner = false
            }

            // === Voice Orb (centrado, visible cuando no es idle) ===
            // El orb del chat solo cubre el dictado al input y la voz SIN
            // avatar: cuando el personaje está activo, él es quien escucha
            // y habla (duplicar el orb aquí era la "interferencia" visual).
            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: orbVisible ? 180 : 0
                visible: orbVisible
                property bool orbVisible: root.dictating
                    || (root.voiceState !== "idle" && !root.characterEnabled)

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
                onRegenerateRequested: {
                    root.regenerate()
                }
                onToolApproveRequested: function(id) {
                    root.approveTool(id, true)
                }
                onToolDenyRequested: function(id) {
                    root.approveTool(id, false)
                }
                onToolDetailRequested: function(name, result) {
                    toolDetail.show(name, result)
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
                // El mic del chat refleja SOLO el dictado al input; la
                // conversación por voz (wake word / PTT) es del avatar y
                // no debe pintar este botón como si estuviera dictando.
                recording: root.dictating
                onSendClicked: function(text) {
                    root.sendMessage(text)
                }
                onMicClicked: {
                    root.toggleDictation()
                }
                onStopClicked: {
                    root.cancelChat()
                    root.voiceState = "idle"
                }
                onFileSelected: function(filePath, fileType) {
                    root.handleFileUpload(filePath, fileType)
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

    // === Tool detail dialog (F4-3) ===
    ToolDetailDialog {
        id: toolDetail
        anchors.fill: parent
        open_: false
    }

    // === Audit dialog (F4-3) ===
    AuditDialog {
        id: auditDialog
        anchors.fill: parent
        open_: false
        backendUrl: root.backendUrl
    }

    // === Settings dialog (overlay) ===
    SettingsDialog {
        id: settings
        anchors.fill: parent
        open_: false
        backendUrl: root.backendUrl
        micLevel: root.voiceLevel
        onClosed: console.log("Settings closed")
        onSaved: {
            console.log("Settings saved")
            root.applyTheme()
            root.checkBackend()
        }
    }

    // === System Tray (KDE Plasma) ===
    TrayMenu {
        id: tray
        windowVisible: root.visible
        alwaysOnTop: root.alwaysOnTop
        pttShortcut: root.pttShortcut
        backendUrl: root.backendUrl
        onShowRequested: {
            root.show()
            root.raise()
            root.requestActivate()
        }
        onHideRequested: root.hide()
        onDictateRequested: {
            // Abre la ventana con el input listo; el dictado es el PTT global.
            root.prefill("")
        }
        onQuickSearchRequested: {
            root.prefill(qsTr("Busca en la web: "))
        }
        onQuickOpenAppRequested: {
            root.prefill(qsTr("Abre "))
        }
        onQuickOpenFolderRequested: {
            root.prefill(qsTr("Muéstrame los archivos de ~/Documentos"))
        }
        onAlwaysOnTopToggled: function(on) {
            root.toggleAlwaysOnTop(on)
        }
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

    // === Desktop Agent (personaje): character.enabled en config.json ===
    // Por defecto desactivado (opt-in). Ver DESKTOP-AGENT-DESIGN.md.
    // El personaje refleja el estado del asistente.
    // Fase 2-3: AgentWindowStandalone (Window XWayland).
    // Fase 4+: AgentMain.qml proceso nativo Wayland (layer-shell).
    // main.rs fija KDE_ASSISTANT_AGENT_OVERLAY=1 cuando el overlay corre:
    // en ese caso la ventana standalone queda OFF (evita personaje duplicado).
    // Qt.platform.environment puede estar null al inicio en sesiones Wayland,
    // así que por defecto asumimos overlay activo (conservador: ocultamos el
    // standalone hasta que se confirme). Un Timer relee tras 200ms.
    function envOr(name, fallback) {
        try {
            var env = Qt.platform.environment
            if (!env) return fallback
            var v = env[name]
            return v === undefined || v === null ? fallback : String(v)
        } catch (err) { return fallback }
    }
    // Default true (conservador): standalone oculto hasta relectura del env.
    property bool agentOverlayRunning: true
    Timer {
        id: overlayCheckTimer
        interval: 200
        repeat: false
        running: true
        onTriggered: {
            var val = envOr("KDE_ASSISTANT_AGENT_OVERLAY", "0")
            root.agentOverlayRunning = (val === "1")
        }
    }
    AgentWindowStandalone {
        id: agentWindow
        agentEnabled: root.characterEnabled && !root.agentOverlayRunning
        reducedMotion: root.characterReducedMotion
        presenceMode: root.characterMode
        sleepAfterSecs: root.characterSleepSecs
        characterSize: root.characterSize
        assistantState: root.voiceState
        voiceLevel: root.voiceLevel
    }

    // === Hotkey polling: lee ~/.cache/kde-assistant/hotkey.state ===
    // El backend Rust (rdev) escribe a este archivo cuando detecta
    // Super+Shift+A (toggle), Super+Shift+V mantenido (PTT start/end),
    // Ctrl+Shift+K (new session). El backend ya graba/procesa el audio;
    // aqui solo se refleja el estado visual (VoiceOrb).
    property string hotkeyStamp: ""
    property string lastHotkeyStamp: ""
    // FIX-sesión-real: ignorar hotkeys más viejos que el arranque. El archivo
    // hotkey.state persiste entre sesiones; sin este filtro, un toggle_window
    // rancio ocultaba la ventana sola 300ms después de abrir (proceso sano,
    // sin errores en el log).
    property double startupTime: 0
    // F0-6: si no hay HOME, no leer /root ajeno: desactivar polling con aviso.
    // (Sin side-effects dentro del binding: el flag se calcula en onCompleted.)
    property bool filePollingEnabled: true
    property string cacheBase: {
        try {
            if (AuthToken.cacheDir) return AuthToken.cacheDir
        } catch (e) {}
        try {
            var env = Qt.platform.environment
            var home = env ? env["HOME"] : null
            if (home && String(home).trim() !== "") {
                return "file://" + home + "/.cache/kde-assistant/"
            }
        } catch (err) {}
        return ""
    }
    property string homePath: cacheBase + "hotkey.state"
    property real voiceLevel: 0.0
    Timer {
        id: hotkeyTimer
        interval: 300
        running: true
        repeat: true
        onTriggered: {
            pollHotkeys()
        }
    }
    // F3-1: reintento del stream de voz si se cae (el backend lo mantiene vivo).
    Timer {
        id: voiceRetryTimer
        interval: 2000
        repeat: false
        onTriggered: startVoiceStream()
    }

    function bargeIn() {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/voice/barge_in")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.send(JSON.stringify({}))
        console.log("Barge-in solicitado")
    }

    // === Dictado al input (boton mic del chat) ===
    // Graba con el micro del sistema y, al parar, transcribe con Whisper y
    // vuelca el texto en el InputBar (sin LLM ni TTS, a diferencia del asistente por voz).
    property bool dictating: false

    function toggleDictation() {
        if (root.dictating) root.stopDictation()
        else root.startDictation()
    }

    function startDictation() {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/dictate/start")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            if (xhr_ok(req)) {
                root.dictating = true
                root.voiceState = "listening"
            } else {
                var msg = qsTr("No se pudo iniciar el dictado")
                try {
                    var r = JSON.parse(req.responseText)
                    if (r.error) msg = r.error
                } catch (e) {}
                lastError = msg
                lastErrorDetail = qsTr("Verifica el micrófono en Configuración.")
                showErrorBanner = true
            }
        }
        req.send(JSON.stringify({}))
    }

    function stopDictation() {
        root.dictating = false
        root.voiceState = "processing"
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/dictate/stop")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            root.voiceState = "idle"
            if (xhr_ok(req)) {
                try {
                    var r = JSON.parse(req.responseText)
                    var t = (r.transcript || "").trim()
                    if (t.length > 0) {
                        var cur = inputBar.text.trim()
                        inputBar.text = cur.length > 0 ? cur + " " + t : t
                        inputBar.focusInput()
                    } else {
                        lastError = qsTr("No se escuchó nada")
                        lastErrorDetail = qsTr("Acércate al micrófono e inténtalo de nuevo.")
                        showErrorBanner = true
                    }
                } catch (e) {}
            } else {
                var msg = qsTr("Falló la transcripción")
                try {
                    var r2 = JSON.parse(req.responseText)
                    if (r2.error) msg = r2.error
                } catch (e) {}
                lastError = msg
                lastErrorDetail = ""
                showErrorBanner = true
            }
        }
        req.send(JSON.stringify({}))
    }

    function pollHotkeys() {
        if (!filePollingEnabled || !root.homePath) return
        var req = new XMLHttpRequest()
        req.open("GET", root.homePath + "?t=" + Date.now())
        req.onreadystatechange = function() {
            if (req.readyState === 4) {
                if (req.status === 200 || req.status === 0) {
                    var content = req.responseText.trim()
                    if (!content || content === root.lastHotkeyStamp) return
                    root.lastHotkeyStamp = content
                    // Solo acciones más nuevas que el arranque (ver startupTime).
                    var parts = content.split("|")
                    var ts = parseInt(parts[1] || "0", 10)
                    if (ts <= root.startupTime) return
                    var action = parts[0]
                    console.log("Hotkey recibido:", action)
                    if (action === "toggle_window") {
                        root.visible = !root.visible
                        if (root.visible) {
                            root.show()
                            root.raise()
                            root.requestActivate()
                        }
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
                    } else if (action === "open_menu") {
                        root.show()
                        root.raise()
                        root.requestActivate()
                        tray.openMenu()
                    }
                }
            }
        }
        req.send()
    }

    // F3-1: push de estado/nivel por SSE (adiós polling de voice.state/level).
    // Un solo XHR persistente; si se cae, reintenta a los 2s.
    property var voiceStreamXhr: null
    function startVoiceStream() {
        if (voiceStreamXhr) {
            try { voiceStreamXhr.abort() } catch (e) {}
            voiceStreamXhr = null
        }
        var xhr = new XMLHttpRequest()
        voiceStreamXhr = xhr
        var processedLen = 0
        var pending = ""
        xhr.open("GET", backendUrl + "/api/voice/stream")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === 3 || xhr.readyState === 4) {
                if (xhr.status === 401) {
                    lastError = qsTr("No autorizado")
                    lastErrorDetail = qsTr("Token local inválido. Reinicia la app.")
                    showErrorBanner = true
                    voiceStreamXhr = null
                    return
                }
                var full = xhr.responseText || ""
                var newPart = full.substring(processedLen)
                processedLen = full.length
                pending += newPart
                var parts = pending.split("\n\n")
                pending = parts.pop()
                for (var k = 0; k < parts.length; k++) {
                    processVoiceBlock(parts[k])
                }
                if (xhr.readyState === 4) {
                    // El backend no cierra el stream salvo error: reintentar.
                    voiceStreamXhr = null
                    if (pending && pending.indexOf("event:") >= 0) {
                        processVoiceBlock(pending)
                        pending = ""
                    }
                    voiceRetryTimer.restart()
                }
            }
        }
        xhr.send()
    }

    function processVoiceBlock(block) {
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
        if (ev === "state" && obj && obj.state) {
            handleVoiceState(obj.state)
        } else if (ev === "level" && obj && obj.level !== undefined) {
            var v = parseFloat(obj.level)
            if (!isNaN(v)) root.voiceLevel = Math.max(0, Math.min(1, v))
        }
    }

    // Aplica un estado de voz (llega por SSE; antes era pollVoiceState).
    function handleVoiceState(state) {
        if (state !== "listening" && state !== "processing"
                && state !== "speaking" && state !== "idle") return
        var was = root.voiceState
        if (was === state) return
        root.voiceState = state
        // Solo abrir la ventana al invocar por voz si NO hay avatar:
        // con el personaje activo, la conversación ocurre en el overlay
        // y no debe saltar la ventana del chat.
        if (state === "listening" && was !== "listening" && !root.characterEnabled) {
            root.show()
            root.raise()
            root.requestActivate()
        }
        // Al terminar un ciclo de voz, traer el intercambio al chat
        if (state === "idle" && was !== "idle") {
            fetchVoiceExchange()
        }
    }

    // El backend escribe listening|processing|speaking|idle con timestamp.
    // Solo los estados nuevos pisan el voiceState local.
    property string lastVoiceCycle: "0"   // timestamp_ms ya mostrado en el chat

    // Trae el ultimo intercambio por voz y lo agrega al chat actual.
    function fetchVoiceExchange() {
        var req = new XMLHttpRequest()
        req.open("GET", backendUrl + "/api/voice/last")
        setAuth(req)
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
        setAuth(req)
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
        setAuth(req)
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
