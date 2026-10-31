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
    property var pendingConsents: ({})

    function showConsent(toolCallId) {
        var request = pendingConsents[toolCallId]
        if (!request) return
        consentDialog.request = request
        consentDialog.openedAtMs = Date.now()
        traceConsentWindow("before_present")
        desktopWindow.present()
        consentDialog.open()
        Qt.callLater(function() { root.traceConsentWindow("after_open") })
    }

    function openNextConsent() {
        if (consentDialog.visible) return
        var ids = Object.keys(pendingConsents)
        if (ids.length > 0) showConsent(ids[0])
    }

    function forgetConsent(toolCallId) {
        if (!pendingConsents[toolCallId]) return
        var remaining = Object.assign({}, pendingConsents)
        delete remaining[toolCallId]
        pendingConsents = remaining
        if (consentDialog.request && consentDialog.request.tool_call_id === toolCallId)
            consentDialog.close()
    }

    function forgetTurnConsents(turnId) {
        var ids = Object.keys(pendingConsents)
        for (var i = 0; i < ids.length; i++) {
            if (pendingConsents[ids[i]].turn_id === turnId) forgetConsent(ids[i])
        }
    }

    function traceConsentWindow(stage) {
        if (Object.keys(pendingConsents).length === 0) return
        console.info("F3_CONSENT_WINDOW", JSON.stringify({stage: stage, at_ms: Date.now(),
            elapsed_ms: consentDialog.openedAtMs ? Date.now() - consentDialog.openedAtMs : 0,
            visible: root.visible, active: root.active, visibility: root.visibility,
            x: root.x, y: root.y, width: root.width, height: root.height,
            popup_visible: consentDialog.visible, popup_focus: consentDialog.activeFocus,
            popup_z: consentDialog.z, focus_item_present: root.activeFocusItem !== null,
            compositor_stacking: "not_observable_from_qml"}))
    }
    DesktopWindowController {
        id: desktopWindow
        objectName: "desktopWindowController"
        window: root
        onNativeActivationStatusChanged: {
            if (nativeActivationStatus === "NOT_SUPPORTED") {
                root.lastError = qsTr("La activación nativa de ventanas no está disponible")
                root.lastErrorDetail = qsTr("Comprueba que el módulo KWindowSystem de KDE esté instalado. El compositor decide si concede el foco.")
                root.showErrorBanner = true
            }
        }
    }
    onVisibilityChanged: { desktopWindow.rememberState(); traceConsentWindow("visibility_changed") }
    onActiveChanged: traceConsentWindow("active_changed")
    property bool closeToTray: true
    property string desktopShortcutStatus: "disabled"

    title: "KDE Assistant v2"
    width: Theme.windowDefaultWidth
    height: Theme.windowDefaultHeight
    minimumWidth: Math.min(Theme.windowMinWidth, Screen.desktopAvailableWidth)
    minimumHeight: Math.min(Theme.windowMinHeight, Screen.desktopAvailableHeight)
    maximumWidth: Theme.windowMaxWidth
    maximumHeight: Theme.windowMaxHeight

    color: "transparent"   // El fondo lo da el Rectangle root

    flags: Qt.Window | Qt.WindowStaysOnTopHint
    visible: true

    // Cerrar la ventana NO apaga el asistente: se oculta y sigue en la
    // bandeja (wake word / hotkeys / recordatorios siguen vivos). La salida
    // real es tray "Salir" (quitRequested -> Qt.quit()).
    onClosing: function(close) {
        desktopWindow.userInteracted = true
        nativeVoice.hide()
        if (closeToTray && tray.trayAvailable) {
            close.accepted = false
            desktopWindow.hide()
        } else { root.detachChat(); Qt.quit() }
    }


    // === Modelo de datos real (conectado al backend Rust) ===
    property string backendUrl: AuthToken.backendUrl
    // Token del módulo privado generado por main.rs para este perfil.
    property string authToken: AuthToken.token
    function setAuth(xhr) {
        if (authToken !== "") xhr.setRequestHeader("Authorization", "Bearer " + authToken)
    }

    property var sessions: []          // [{id, title, updated_at, message_count}]
    property string currentSessionId: ""

    property var messages: []          // [{role, authorLabel, content, timestamp, isStreaming, toolCalls}]
    property bool voiceAvailable: false
    property bool voiceOutputReady: false
    property bool voiceModelReady: false
    onVisibleChanged: { if (nativeVoice) { if (visible && !nativeVoice.backgroundEnabled) root.syncVoiceSession(); else if (!visible) nativeVoice.hide() } }

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
            var creationGeneration = chatRequestGeneration
            var creationSession = currentSessionId
            var cxhr = new XMLHttpRequest()
            cxhr.open("POST", backendUrl + "/api/session")
            cxhr.setRequestHeader("Content-Type", "application/json")
            setAuth(cxhr)
            cxhr.onreadystatechange = function() {
                if (creationGeneration !== chatRequestGeneration || creationSession !== currentSessionId) return
                if (cxhr.readyState === XMLHttpRequest.DONE && (cxhr.status === 200 || cxhr.status === 201)) {
                    try {
                        var s = JSON.parse(cxhr.responseText)
                        currentSessionId = s.id
                        syncVoiceSession()
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

    // Repetir una acción exige un mensaje nuevo explícito, sin borrar historial.
    function regenerate() {
        if (!currentSessionId || streaming) return
        lastError = qsTr("Repetición explícita requerida")
        lastErrorDetail = qsTr("Para repetir una respuesta o herramienta, envía un mensaje nuevo. El historial se conserva para evitar reejecutar acciones anteriores.")
        showErrorBanner = true
    }

    // Motor SSE genérico: opcionalmente agrega el mensaje del usuario,
    // siempre agrega el placeholder del asistente y streamea tokens/tools.
    function streamAgent(url, bodyObj, userText) {
        var requestGeneration = ++chatRequestGeneration
        var requestSession = bodyObj.session_id || ""
        activeChatTurnId = ""
        activeChatSessionId = requestSession
        chatCancelling = false
        var lastSequence = 0
        var terminalReceived = false
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
            if (requestGeneration !== chatRequestGeneration || requestSession !== currentSessionId) return
            var arr = messages.slice()
            if (assistantIdx >= 0 && assistantIdx < arr.length) {
                var old = arr[assistantIdx]
                arr[assistantIdx] = {
                    role: "assistant",
                    authorLabel: "KDE Assistant",
                    content: chatCancelling ? qsTr("Cancelando; esperando que terminen las operaciones iniciadas…") : (rawText ? markdownToHtml(rawText) : (stillStreaming ? "" : "⚠ Sin respuesta del asistente")),
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
            if (requestGeneration !== chatRequestGeneration || requestSession !== currentSessionId) return
            if (!obj.turn_id || (obj.session_id || "") !== requestSession || obj.sequence <= lastSequence) return
            if (ev === "turn") {
                if (activeChatTurnId && activeChatTurnId !== obj.turn_id) return
                activeChatTurnId = obj.turn_id
                if (chatCancelling) requestTurnCancellation()
            }
            if (activeChatTurnId !== obj.turn_id) return
            lastSequence = obj.sequence
            if (ev === "token") {
                if (obj && obj.content) rawText += obj.content
            } else if (ev === "tool_call") {
                if (obj) toolArr.push({ id: obj.id || "", name: obj.name || "tool", status: "running", result: "", imageUrl: "", caption: "" })
            } else if (ev === "tool_result") {
                if (obj) {
                    forgetConsent(obj.tool_call_id)
                    for (var j = 0; j < toolArr.length; j++) {
                        if (toolArr[j].id === obj.tool_call_id) {
                            var denied = obj.content && (obj.content.indexOf("El usuario denegó") === 0 || obj.content.indexOf("Acción sensible denegada") === 0)
                            toolArr[j].status = (denied || obj.success === false) ? "error" : "success"
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
                        toolArr.push({ id: obj.tool_call_id, name: "tool", status: obj.success === false ? "error" : "success", result: obj.content || "", imageUrl: obj.image_url || "", caption: "" })
                    }
                }
            } else if (ev === "approval_needed") {
                if (obj) {
                    var requests = Object.assign({}, pendingConsents)
                    requests[obj.tool_call_id] = obj
                    pendingConsents = requests
                    for (var n = 0; n < toolArr.length; n++) {
                        if (toolArr[n].id === obj.tool_call_id) {
                            toolArr[n].status = "confirm"
                            toolArr[n].result = (obj.effect || qsTr("Acción")) + ": " + (obj.resource || "")
                            break
                        }
                    }
                    if (!consentDialog.visible) Qt.callLater(root.openNextConsent)
                }
            } else if (ev === "cancelled") {
                forgetTurnConsents(obj.turn_id)
                terminalReceived = true
                rawText = obj.message || qsTr("Cancelado por el usuario; los efectos iniciados pueden haber terminado.")
            } else if (ev === "done") {
                nativeVoice.observe()
                forgetTurnConsents(obj.turn_id)
                terminalReceived = true
                if (obj && obj.full_content && !rawText) rawText = obj.full_content
            } else if (ev === "error") {
                forgetTurnConsents(obj.turn_id)
                terminalReceived = true
                if (obj && obj.message) sawError = obj.message
            }
        }

        var xhr = new XMLHttpRequest()
        xhr.open("POST", url)
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        activeChatXhr = xhr
        xhr.onreadystatechange = function() {
            if (requestGeneration !== chatRequestGeneration || requestSession !== currentSessionId) return
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
                        lastErrorDetail = qsTr("Falta el token local. Reinicia KDE Assistant para regenerar el módulo autenticado del perfil.")
                        showErrorBanner = true
                        streaming = false
                        chatCancelling = false
                        activeChatXhr = null
                        refreshUI(false)
                        return
                    }
                    if (pending && pending.indexOf("event:") >= 0) {
                        processBlock(pending)
                        pending = ""
                    }
                    if (!terminalReceived && !sawError) {
                        try {
                            var failure = JSON.parse(full)
                            sawError = failure.error || failure.message || qsTr("Conexión interrumpida antes de terminar el turno")
                        } catch (e) { sawError = qsTr("Conexión interrumpida antes de terminar el turno") }
                    }
                    if (sawError) {
                        rawText += (rawText ? "\n\n" : "") + "⚠ " + sawError
                        lastError = qsTr("Error del asistente")
                        lastErrorDetail = sawError
                        showErrorBanner = true
                    }
                    streaming = false
                    chatCancelling = false
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
        var listGeneration = chatRequestGeneration
        var selectedSession = currentSessionId
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/sessions")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (listGeneration !== chatRequestGeneration || selectedSession !== currentSessionId) return
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
        detachChat()
        var creationGeneration = chatRequestGeneration
        var previousSession = currentSessionId
        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/session")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (creationGeneration !== chatRequestGeneration || previousSession !== currentSessionId) return
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
    function deleteSession(sessionId, eraseEvidence) {
        var xhr = new XMLHttpRequest()
        xhr.open("DELETE", backendUrl + "/api/session?session_id=" + encodeURIComponent(sessionId) + (eraseEvidence ? "&erase_uncertain_evidence=true" : ""))
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== XMLHttpRequest.DONE) return
            if (xhr.status === 409 && xhr.responseText.indexOf("uncertain_evidence_confirmation_required") >= 0) {
                recoveryHistory.confirmDeletion(sessionId)
                return
            }
            if (xhr.status !== 200 && xhr.status !== 201) {
                lastError = qsTr("No se pudo eliminar la conversación")
                lastErrorDetail = xhr.status === 409 ? qsTr("Hay un turno u operación en curso. Espera a que termine.") : qsTr("El backend no confirmó el borrado; el historial se conserva.")
                showErrorBanner = true
                return
            }
            if (xhr.status === 200 || xhr.status === 201) {
                if (currentSessionId === sessionId) {
                    detachChat()
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
        var loadGeneration = chatRequestGeneration
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/messages?session_id=" + encodeURIComponent(sessionId))
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (sessionId !== currentSessionId || loadGeneration !== chatRequestGeneration) return
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
        syncVoiceSession()
    }

    function syncVoiceSession() {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/voice/session")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.send(JSON.stringify({ session_id: currentSessionId ? currentSessionId : null }))
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
    property int voiceAssistantIndex: -1
    property string voiceLiveText: ""
    property string agentState: "idle"
    readonly property string avatarState: voiceState !== "idle" ? voiceState : agentState
    property bool calibrationOpen: false
    property bool calibrationRecording: false
    property int calibrationCount: 0
    property int calibrationTarget: 3
    property string calibrationHint: qsTr("Di «hey Jarvis» de forma natural")
    // SSE se conserva durante Stop y se libera al abandonar la sesión.
    property var activeChatXhr: null
    property string activeChatTurnId: ""
    property string activeChatSessionId: ""
    property bool chatCancelling: false
    property int chatRequestGeneration: 0
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
    property string characterAppearance: "capsule"
    property color characterAccentColor: "#0094bb"

    // Prepara el input con un texto y abre la ventana (acciones rápidas del tray).
    function prefill(text) {
        inputBar.text = text
        desktopWindow.present()
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
        var consent = pendingConsents[toolCallId]
        if (!consent) return
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/tools/approve")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            if (req.status !== 200) {
                lastErrorDetail = qsTr("La autorización caducó o ya no corresponde a este turno.")
                return
            }
            var arr = messages.slice()
            for (var i = 0; i < arr.length; i++) {
                var tcs = arr[i].toolCalls || []
                for (var j = 0; j < tcs.length; j++) {
                    if (tcs[j].id === toolCallId && tcs[j].status === "confirm") {
                        var updatedCalls = tcs.slice()
                        updatedCalls[j] = Object.assign({}, tcs[j], {
                            status: approved ? "running" : "error",
                            result: approved ? qsTr("Aprobada, ejecutando…") : qsTr("Denegada por ti")
                        })
                        arr[i] = Object.assign({}, arr[i], { toolCalls: updatedCalls })
                    }
                }
            }
            messages = arr
        }
        req.send(JSON.stringify({ tool_call_id: toolCallId, consent_id: consent.consent_id,
                                   turn_id: consent.turn_id, session_id: consent.session_id,
                                   approved: approved }))
        var remaining = Object.assign({}, pendingConsents)
        delete remaining[toolCallId]
        pendingConsents = remaining
    }

    // Stop conserva SSE hasta el terminal: HTTP 202 sólo confirma la solicitud.
    function cancelChat() {
        if (!streaming || chatCancelling) return
        chatCancelling = true
        var arr = messages.slice()
        for (var i = arr.length - 1; i >= 0; i--) {
            if (arr[i].isStreaming) {
                var copy = Object.assign({}, arr[i])
                copy.content = qsTr("Cancelando; esperando que terminen las operaciones iniciadas…")
                arr[i] = copy
                break
            }
        }
        messages = arr
        // Si aún falta la identidad, el primer evento turn despacha la solicitud.
        requestTurnCancellation()
    }

    function requestTurnCancellation() {
        if (!activeChatTurnId) return
        var turn = activeChatTurnId
        var session = activeChatSessionId
        var generation = chatRequestGeneration
        var c = new XMLHttpRequest()
        c.open("POST", backendUrl + "/api/chat/cancel")
        c.setRequestHeader("Content-Type", "application/json")
        setAuth(c)
        c.onreadystatechange = function() {
            if (c.readyState !== XMLHttpRequest.DONE || generation !== chatRequestGeneration || session !== currentSessionId || turn !== activeChatTurnId || !streaming) return
            if (c.status !== 200 && c.status !== 202) {
                lastError = qsTr("No se pudo confirmar la solicitud de cancelación")
                lastErrorDetail = c.responseText || qsTr("El turno sigue pendiente de un estado terminal")
                showErrorBanner = true
            }
            // Incluso 200 puede corresponder a Completed/Failed. SSE decide el terminal.
        }
        c.send(JSON.stringify({ session_id: session || null, turn_id: turn }))
    }

    function detachChat() {
        nativeVoice.detach()
        var consents = Object.keys(pendingConsents)
        for (var i = 0; i < consents.length; i++) approveTool(consents[i], false)
        if (consentDialog.visible) consentDialog.close()
        ++chatRequestGeneration
        activeChatTurnId = ""
        activeChatSessionId = ""
        chatCancelling = false
        streaming = false
        if (activeChatXhr) {
            try { activeChatXhr.abort() } catch (e) {}
            activeChatXhr = null
        }
    }

    Component.onCompleted: {
        root.startupTime = Date.now()
        root.width = Math.min(root.width, Screen.desktopAvailableWidth)
        root.height = Math.min(root.height, Screen.desktopAvailableHeight)
        // Centrar al arrancar: sin x/y el WM la deja arriba-izquierda.
        // Asignacion unica (no binding) para no pelear con el usuario.
        root.x = Math.round((Screen.desktopAvailableWidth - root.width) / 2)
        root.y = Math.round((Screen.desktopAvailableHeight - root.height) / 2)
        console.info("F3_WINDOW_READY", Date.now(), root.visible, root.active, root.width, root.height)
        loadSessions()
        checkBackend()
        applyTheme()
        startDesktopStream()
        startVoiceStream()
        startAgentStream()
        // Hey Jarvis stays OFF until the explicit, one-dictation action.
        // Rescate: si la ventana quedó oculta (recreación por flags),
        // forzar visible + diagnóstico de geometría.
        rescueTimer.start()
    }

    function checkVoiceCalibration() {
        var req = new XMLHttpRequest()
        req.open("GET", backendUrl + "/api/voice/calibration")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState === XMLHttpRequest.DONE && req.status === 200) {
                try {
                    var s = JSON.parse(req.responseText)
                    calibrationCount = s.count || 0
                    calibrationTarget = s.target || 3
                    calibrationOpen = s.required === true
                } catch (e) {}
            }
        }
        req.send()
    }

    function startCalibrationSample() {
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/voice/calibration")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState === XMLHttpRequest.DONE) {
                if (req.status === 200) {
                    calibrationRecording = true
                    calibrationHint = qsTr("Escuchando… di «hey Jarvis»")
                    calibrationTimeout.restart()
                } else {
                    calibrationHint = qsTr("No se pudo abrir el micrófono")
                }
            }
        }
        req.send()
    }

    function stopCalibrationSample() {
        var req = new XMLHttpRequest()
        req.open("DELETE", backendUrl + "/api/voice/calibration")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            calibrationRecording = false
            calibrationTimeout.stop()
            if (req.status === 200) {
                try {
                    var s = JSON.parse(req.responseText)
                    calibrationCount = s.count || calibrationCount
                    calibrationOpen = s.required === true
                    calibrationHint = calibrationOpen ? qsTr("Muestra guardada. Repite con otra entonación.") : qsTr("Calibración completada")
                } catch (e) {}
            } else {
                calibrationHint = qsTr("Muestra demasiado corta; inténtalo de nuevo")
            }
        }
        req.send()
    }

    Timer {
        id: calibrationTimeout
        interval: 8000
        repeat: false
        onTriggered: {
            if (root.calibrationRecording) {
                root.calibrationHint = qsTr("Tiempo agotado; guardando la muestra")
                root.stopCalibrationSample()
            }
        }
    }
    Timer {
        id: rescueTimer
        interval: 1500
        repeat: false
        onTriggered: {
            if (!root.visible && !desktopWindow.userInteracted) {
                console.warn("ventana oculta al arrancar: forzando show")
                root.show()
                root.raise()
                root.requestActivate()
            }
            console.log("ventana:", root.visible, root.width + "x" + root.height, "en", root.x + "," + root.y)
        }
    }

    function shortcutStatusText() {
        if (desktopShortcutStatus === "registered_portal") return qsTr("Atajo global activo")
        if (desktopShortcutStatus === "awaiting_system_consent") return qsTr("Autoriza el atajo en el sistema")
        if (desktopShortcutStatus === "disabled") return qsTr("Atajo desactivado")
        return qsTr("Atajo no disponible")
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
                                lastErrorDetail = qsTr("Token local inválido. Cierra KDE Assistant desde su menú y reinicia.")
                                showErrorBanner = true
                                return
                            }
                            if (c.status !== 200) return
                            try {
                                var cfg = JSON.parse(c.responseText)
                                voiceOutputReady = !!(cfg.capabilities && cfg.capabilities.piper_binary_present)
                                voiceAvailable = !!(cfg.capabilities && cfg.capabilities.native_dictation)
                                voiceModelReady = !!(cfg.capabilities && (cfg.capabilities.stt_model_present !== undefined ? cfg.capabilities.stt_model_present : cfg.capabilities.whisper_model_present))
                                var key = (cfg.ai && cfg.ai.has_api_key === true) ? "configured" : ""
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
                    root.closeToTray = !cfg.ui || cfg.ui.close_to_tray !== false
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
                        if (cfg.character.appearance)
                            root.characterAppearance = String(cfg.character.appearance)
                        if (cfg.character.accent_color)
                            root.characterAccentColor = cfg.character.accent_color
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
                if (id !== root.currentSessionId) root.detachChat()
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
                recoveryHistory.show("")
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
                    anchors.right: dashboardBtn.left
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

                IconButton {
                    id: dashboardBtn
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.rightMargin: Theme.spacingXs
                    iconName: "history-16"
                    iconSize: 16
                    buttonSize: 28
                    backgroundColor: "transparent"
                    iconColor: Theme.inkMuted
                    accessibleName: qsTr("Actividad de agentes")
                    onClicked: agentDashboard.open()
                }
            }

            PillButton {
                Layout.fillWidth: true
                Layout.leftMargin: Theme.spacingMd
                Layout.rightMargin: Theme.spacingMd
                visible: root.recoveryCount > 0 || root.recoveryStatus === "unavailable"
                text: root.recoveryStatus === "unavailable" ? qsTr("Recuperación no disponible · Consultar historial") : qsTr("Hay operaciones inciertas, fallidas o abandonadas · Revisar")
                elideText: true
                iconName: "history-16"
                onClicked: recoveryHistory.show(root.currentSessionId)
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
                            root.voiceState = "idle"
                        } else if (root.voiceState !== "idle") {
                            root.voiceState = "idle"
                        }
                    }
                }
            }

            // === ChatView ===
            ChatView {
                speechAvailable: root.voiceOutputReady
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
                    root.showConsent(id)
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
                voiceEnabled: root.voiceAvailable && !nativeVoice.backgroundEnabled
                voiceReady: root.voiceModelReady
                voiceUnavailableText: qsTr("Modelo STT no instalado · Configuración → Voz")
                // Estado del dictado coordinado por Rust; no representa wake
                // ni una escucha automática del personaje.
                recording: root.dictating
                voiceStatus: nativeVoice.state
                voiceBusy: nativeVoice.busy
                wakeAvailable: root.voiceAvailable
                backgroundEnabled: nativeVoice.backgroundEnabled
                backgroundPaused: nativeVoice.backgroundPaused
                microphoneActive: nativeVoice.microphoneActive
                backgroundState: nativeVoice.backgroundState
                onWakeClicked: root.controlBackground(!nativeVoice.backgroundEnabled ? "enable" : nativeVoice.backgroundPaused ? "resume" : "pause")
                onBackgroundDisableClicked: root.controlBackground("disable")
                onVoiceCancelClicked: nativeVoice.cancel()
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
    Dialog {
        id: consentDialog
        objectName: "f2ConsentDialog"
        property var request: null
        property double openedAtMs: 0
        onOpened: root.traceConsentWindow("popup_opened")
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        title: qsTr("Autorizar herramienta")
        width: Math.min(root.width - Theme.spacingMd * 2, 420)
        x: (root.width - width) / 2
        y: (root.height - height) / 2
        onClosed: {
            if (request && root.pendingConsents[request.tool_call_id])
                root.approveTool(request.tool_call_id, false)
            request = null
            Qt.callLater(root.openNextConsent)
        }

        contentItem: Column {
            spacing: Theme.spacingSm
            Label { objectName: "f2ConsentTool"; width: parent.width; wrapMode: Text.Wrap; text: consentDialog.request ? qsTr("Herramienta: ") + consentDialog.request.name : "" }
            Label { objectName: "f2ConsentEffect"; width: parent.width; wrapMode: Text.Wrap; text: consentDialog.request ? qsTr("Acción: ") + consentDialog.request.effect : "" }
            Label { objectName: "f2ConsentResource"; width: parent.width; wrapMode: Text.Wrap; text: consentDialog.request ? qsTr("Recurso: ") + consentDialog.request.resource : "" }
            Label { width: parent.width; wrapMode: Text.Wrap; visible: !!(consentDialog.request && consentDialog.request.detail); text: consentDialog.request ? consentDialog.request.detail || "" : "" }
            Label { width: parent.width; wrapMode: Text.Wrap; text: consentDialog.request ? qsTr("Riesgo: ") + consentDialog.request.permission : "" }
            Label { width: parent.width; wrapMode: Text.Wrap; text: qsTr("Alcance: esta operación exacta, una vez, durante 120 segundos. El efecto puede no revertirse al cancelar.") }
            Row {
                spacing: Theme.spacingSm
                Button {
                    text: qsTr("Denegar")
                    onClicked: {
                        if (consentDialog.request) root.approveTool(consentDialog.request.tool_call_id, false)
                        consentDialog.close()
                    }
                }
                Button {
                    text: qsTr("Aprobar")
                    onClicked: {
                        if (consentDialog.request) root.approveTool(consentDialog.request.tool_call_id, true)
                        consentDialog.close()
                    }
                }
            }
        }
    }

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
    AgentDashboard {
        id: agentDashboard
        backendUrl: root.backendUrl
    }

    RecoveryHistory {
        id: recoveryHistory
        anchors.fill: parent
        backendUrl: root.backendUrl
        authToken: root.authToken
        onEraseConfirmed: function(sessionId) { root.deleteSession(sessionId, true) }
    }

    property string recoveryStatus: "idle"
    property int recoveryCount: 0
    property int recoveryGeneration: 0
    onCurrentSessionIdChanged: { refreshRecovery(); syncVoiceSession() }
    onStreamingChanged: { if (!streaming) refreshRecovery() }
    function refreshRecovery() {
        var session = currentSessionId
        var generation = ++recoveryGeneration
        recoveryCount = 0
        recoveryStatus = "idle"
        if (!session) return
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/tools/operations?recovery_only=true&limit=1&session_id=" + encodeURIComponent(session))
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== XMLHttpRequest.DONE || generation !== recoveryGeneration || session !== currentSessionId) return
            if (xhr.status !== 200) { recoveryStatus = "unavailable"; return }
            try {
                var rows = JSON.parse(xhr.responseText)
                if (!Array.isArray(rows)) throw new Error("invalid operations")
                recoveryCount = rows.length
                recoveryStatus = rows.length ? "attention" : "empty"
            } catch (e) { recoveryStatus = "unavailable" }
        }
        xhr.send()
    }

    // === Settings dialog (overlay) ===
    SettingsDialog {
        id: settings
        backgroundEnabled: nativeVoice.backgroundEnabled
        backgroundPaused: nativeVoice.backgroundPaused
        microphoneActive: nativeVoice.microphoneActive
        onBackgroundControlRequested: function(action) { root.controlBackground(action) }
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

    Popup {
        id: calibrationPopup
        anchors.centerIn: parent
        width: Math.min(root.width - 32, 390)
        padding: 24
        modal: true
        closePolicy: Popup.NoAutoClose
        visible: root.calibrationOpen
        background: Rectangle {
            radius: Theme.radiusLg
            color: Theme.surface
            border.width: 1
            border.color: Theme.hairline
        }
        contentItem: Column {
            spacing: Theme.spacingMd
            Text {
                text: qsTr("Configura tu voz")
                color: Theme.ink
                font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, Theme.lsHero)
            }
            Text {
                text: qsTr("Para reconocer mejor «hey Jarvis», grabaremos tres muestras localmente. No se subirá ningún audio.")
                color: Theme.inkMuted
                wrapMode: Text.WordWrap
                width: calibrationPopup.width - 48
                font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
            }
            Text {
                text: root.calibrationHint
                color: root.calibrationRecording ? Theme.primary : Theme.inkMuted
                wrapMode: Text.WordWrap
                width: calibrationPopup.width - 48
                font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, 0)
            }
            Text {
                text: qsTr("Muestras: %1/%2").arg(root.calibrationCount).arg(root.calibrationTarget)
                color: Theme.ink
                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0)
            }
            AppleButton {
                Layout.fillWidth: true
                text: root.calibrationRecording ? qsTr("Guardar muestra") : qsTr("Grabar muestra")
                variant: root.calibrationRecording ? "destructive" : "primary"
                iconName: root.calibrationRecording ? "stop-16" : "microphone-16"
                onClicked: root.calibrationRecording ? root.stopCalibrationSample() : root.startCalibrationSample()
            }
            AppleButton {
                Layout.fillWidth: true
                visible: !root.calibrationRecording
                text: qsTr("Configurar más tarde")
                variant: "ghost"
                onClicked: root.calibrationOpen = false
            }
        }
    }

    // === System Tray (KDE Plasma) ===
    TrayMenu {
        id: tray
        windowVisible: desktopWindow.canHide
        recentSessions: root.sessions.slice(0, 5)
        backgroundEnabled: nativeVoice.backgroundEnabled
        backgroundPaused: nativeVoice.backgroundPaused
        microphoneActive: nativeVoice.microphoneActive
        onBackgroundActionRequested: function(action) { root.controlBackground(action) }
        statusText: root.backendStatusText() + " · " + root.shortcutStatusText()
        voiceEnabled: root.voiceAvailable
        alwaysOnTop: root.alwaysOnTop
        alwaysOnTopAvailable: desktopWindow.nativeActivator !== null && !desktopWindow.nativeActivator.wayland
        pttShortcut: root.pttShortcut
        backendUrl: root.backendUrl
        onShowRequested: desktopWindow.present()
        onHideRequested: { nativeVoice.hide(); desktopWindow.hide() }
        onMinimizeRequested: desktopWindow.minimize()
        onDictateRequested: {
            if (!root.voiceAvailable) return
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
            desktopWindow.present()
        }
        onOpenSessionRequested: function(sessionId) {
            if (sessionId !== root.currentSessionId) root.detachChat()
            root.currentSessionId = sessionId
            root.loadMessages(sessionId)
            desktopWindow.present()
        }
        onSettingsRequested: {
            settings.show()
            desktopWindow.present()
        }
        onQuitRequested: { root.detachChat(); Qt.quit() }
    }

    // === Desktop Agent (personaje): character.enabled en config.json ===
    // Por defecto desactivado (opt-in). Ver DESKTOP-AGENT-DESIGN.md.
    // El personaje refleja el estado del asistente.
    // Fase 2-3: AgentWindowStandalone (Window XWayland).
    // Fase 4+: AgentMain.qml proceso nativo Wayland (layer-shell).
    //
    // ¿Overlay corriendo? Lo sabe main.rs y lo hornea en el módulo
    // generado qml.auth (AuthToken.agentOverlay). El env
    // KDE_ASSISTANT_AGENT_OVERLAY nunca llegó a QML: Qt.platform.environment
    // es null en esta sesión qml6 (comprobado: typeof === "undefined");
    // ese era el origen del personaje duplicado (FIX v3).
    readonly property bool agentOverlayRunning: readAgentOverlay()
    function readAgentOverlay() {
        try {
            return AuthToken.agentOverlay === true
        } catch (e) { return false }
    }
    AgentWindowStandalone {
        id: agentWindow
        agentEnabled: root.characterEnabled && !root.agentOverlayRunning
        reducedMotion: root.characterReducedMotion
        presenceMode: root.characterMode
        sleepAfterSecs: root.characterSleepSecs
        characterSize: root.characterSize
        characterAppearance: root.characterAppearance
        characterAccentColor: root.characterAccentColor
        assistantState: root.avatarState
        invoked: root.voiceState !== "idle"
        voiceLevel: root.voiceLevel
    }

    // Profile-scoped, outbound-only desktop SSE. No filesystem polling.
    property real voiceLevel: 0.0
    property double startupTime: 0
    property string cacheBase: AuthToken.cacheDir
    property var desktopXhr: null
    Timer { id: desktopRetry; interval: 2000; repeat: false; onTriggered: startDesktopStream() }
    function handleDesktopAction(action) {
        if (action === "show") desktopWindow.present()
        else if (action === "toggle") desktopWindow.toggle()
        else if (action === "status") refreshDesktopStatus()
    }
    function startDesktopStream() {
        if (desktopXhr) return
        var req = new XMLHttpRequest()
        var consumed = 0
        var pending = ""
        desktopXhr = req
        req.open("GET", backendUrl + "/api/desktop/events")
        setAuth(req)
        req.onreadystatechange = function() {
            if (desktopXhr !== req) return
            if (req.readyState === 3 || req.readyState === 4) {
                pending += (req.responseText || "").substring(consumed)
                consumed = (req.responseText || "").length
                var blocks = pending.split("\n\n")
                pending = blocks.pop()
                for (var i = 0; i < blocks.length; ++i) {
                    var lines = blocks[i].split("\n")
                    var event = "desktop"
                    for (var j = 0; j < lines.length; ++j) {
                        if (lines[j].indexOf("event: ") === 0) event = lines[j].substring(7).trim()
                        if (lines[j].indexOf("data: ") !== 0) continue
                        var data = lines[j].substring(6).trim()
                        if (event === "desktop_activation") {
                            try {
                                var activation = JSON.parse(data)
                                if (activation.action === "toggle") desktopWindow.toggle(activation.token || "")
                            } catch (e) { console.warn("F3: evento de activación inválido") }
                        } else root.handleDesktopAction(data)
                    }
                }
                if (req.readyState === 4) { desktopXhr = null; desktopRetry.restart() }
            }
        }
        req.send()
        refreshDesktopStatus()
    }
    function refreshDesktopStatus() {
        var status = new XMLHttpRequest()
        status.open("GET", backendUrl + "/api/desktop/status")
        setAuth(status)
        status.onreadystatechange = function() {
            if (status.readyState === 4 && status.status === 200) {
                try { root.desktopShortcutStatus = JSON.parse(status.responseText).shortcuts } catch (e) {}
            }
        }
        status.send()
    }
    // F3-1: reintento del stream de voz si se cae (el backend lo mantiene vivo).
    Timer {
        id: voiceRetryTimer
        interval: 2000
        repeat: false
        onTriggered: startVoiceStream()
    }

    function bargeIn() { nativeVoice.cancel() }

    // Dictation inserts into the current draft; only Send creates a chat turn.
    readonly property bool dictating: nativeVoice.state === "recording"
    VoiceController {
        id: nativeVoice
        objectName: "nativeVoiceController"
        backgroundMonitoring: true
        onWakeDetected: desktopWindow.present()
        backendUrl: root.backendUrl
        token: AuthToken.token
        sessionId: root.currentSessionId
        onTranscriptReady: function(text) {
            if (text.trim().length > 0) {
                inputBar.text = inputBar.text.length > 0 ? inputBar.text + " " + text : text
                inputBar.focusInput()
            } else { root.lastError = qsTr("No se escuchó habla reconocible"); root.showErrorBanner = true }
        }
        onFailed: function(message) { root.lastError = message; root.lastErrorDetail = ""; root.showErrorBanner = true }
    }
    function controlBackground(action) {
        if (action !== "enable" || currentSessionId) { nativeVoice.backgroundControl(action); return }
        nativeVoice.request("/api/session", {title: qsTr("Conversación de voz")}, function(ok,value) {
            if (!ok || !value.id) { root.lastError = qsTr("No se pudo crear la conversación de voz"); root.showErrorBanner=true; return }
            root.currentSessionId = value.id
            root.loadSessions()
            Qt.callLater(function() { nativeVoice.backgroundControl("enable") })
        })
    }
    function toggleDictation() {
        if (nativeVoice.state === "recording") nativeVoice.stop()
        else if (nativeVoice.busy) nativeVoice.cancel()
        else root.startDictation()
    }
    function startDictation() {
        if (nativeVoice.busy || !root.visible) return
        if (currentSessionId) { nativeVoice.start(); return }
        var requestGeneration = chatRequestGeneration
        var voiceGeneration = nativeVoice.generation
        nativeVoice.requestPending = true
        nativeVoice.state = "preparing"
        var req = new XMLHttpRequest()
        req.open("POST", backendUrl + "/api/session")
        req.setRequestHeader("Content-Type", "application/json")
        setAuth(req)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE || requestGeneration !== chatRequestGeneration
                    || voiceGeneration !== nativeVoice.generation || !root.visible) return
            nativeVoice.requestPending = false
            nativeVoice.state = "idle"
            if (xhr_ok(req)) {
                var session = JSON.parse(req.responseText)
                currentSessionId = session.id
                loadSessions()
                var nextGeneration = nativeVoice.generation
                Qt.callLater(function() {
                    if (root.visible && nextGeneration === nativeVoice.generation) nativeVoice.start()
                })
            } else { root.lastError = qsTr("No se pudo crear una conversación para dictar"); root.showErrorBanner = true }
        }
        req.send(JSON.stringify({title: qsTr("Nueva conversación")}))
    }
    function stopDictation() { nativeVoice.stop() }

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

    // Actividad del chat escrito para el avatar. Es un stream distinto al de
    // voz para que escribir no active el orbe ni el micrófono.
    property var agentStreamXhr: null
    function startAgentStream() {
        if (agentStreamXhr) { try { agentStreamXhr.abort() } catch (e) {} }
        var xhr = new XMLHttpRequest()
        agentStreamXhr = xhr
        var processedLen = 0
        var pending = ""
        xhr.open("GET", backendUrl + "/api/agent/stream")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== 3 && xhr.readyState !== 4) return
            var full = xhr.responseText || ""
            pending += full.substring(processedLen)
            processedLen = full.length
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
                    if (event.type && !root.agentOverlayRunning) agentWindow.handleActivity(event)
                    if (event.state === "processing" || event.state === "idle")
                        root.agentState = event.state
                } catch (e) {}
            }
            if (xhr.readyState === 4) {
                agentStreamXhr = null
                agentWindow.resetActivity()
                agentRetryTimer.restart()
            }
        }
        xhr.send()
    }

    Timer {
        id: agentRetryTimer
        interval: 2000
        repeat: false
        onTriggered: startAgentStream()
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
        } else if (ev === "transcript" && obj && obj.content) {
            // El turno hablado entra en la conversación antes de que termine
            // el LLM; la respuesta se completa con eventos token.
            root.voiceLiveText = ""
            root.appendMessage({
                role: "user",
                authorLabel: "Tu (voz)",
                content: obj.content,
                raw: obj.content,
                timestamp: nowTime(),
                isStreaming: false,
                toolCalls: []
            })
            root.appendMessage({
                role: "assistant",
                authorLabel: "KDE Assistant",
                content: "",
                raw: "",
                timestamp: nowTime(),
                isStreaming: true,
                toolCalls: []
            })
            root.voiceAssistantIndex = root.messages.length - 1
        } else if (ev === "token" && obj && obj.content) {
            root.voiceLiveText += obj.content
            if (root.voiceAssistantIndex >= 0 && root.voiceAssistantIndex < root.messages.length) {
                var arr = root.messages.slice()
                var old = arr[root.voiceAssistantIndex]
                arr[root.voiceAssistantIndex] = {
                    role: "assistant",
                    authorLabel: "KDE Assistant",
                    content: root.markdownToHtml(root.voiceLiveText),
                    raw: root.voiceLiveText,
                    timestamp: old.timestamp,
                    isStreaming: true,
                    toolCalls: old.toolCalls || []
                }
                root.messages = arr
            }
        } else if (ev === "exchange" && obj) {
            root.lastVoiceCycle = String(obj.timestamp_ms || 0)
            if (root.voiceAssistantIndex >= 0 && root.voiceAssistantIndex < root.messages.length) {
                var done = root.messages.slice()
                var current = done[root.voiceAssistantIndex]
                var finalText = obj.response || root.voiceLiveText
                done[root.voiceAssistantIndex] = {
                    role: "assistant",
                    authorLabel: "KDE Assistant",
                    content: root.markdownToHtml(finalText),
                    raw: finalText,
                    timestamp: current.timestamp,
                    isStreaming: false,
                    toolCalls: current.toolCalls || []
                }
                root.messages = done
            }
            root.voiceAssistantIndex = -1
            root.voiceLiveText = ""
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
                    // Rust ya persistió el turno antes de emitir idle. No
                    // duplicarlo desde QML: repetiría el contexto del turno.
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
        nativeVoice.read(text)
    }

    // Copia al portapapeles via buffer oculto.
    function copyToClipboard(text) {
        clipBuffer.text = text || ""
        clipBuffer.selectAll()
        clipBuffer.copy()
        clipBuffer.deselect()
    }
}
