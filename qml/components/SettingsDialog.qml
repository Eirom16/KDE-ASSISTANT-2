// SettingsDialog.qml - Dialog modal de configuracion
// Acceso desde el SessionDrawer ("Configuracion") o desde la X del title
// NOTA: los campos se leen/escriben de forma imperativa (por id) para
// evitar peleas de bindings con lo que escribe el usuario.

import QtQuick
import QtQuick.Controls
import qml 1.0
import QtQuick.Layouts
import qml.auth 1.0
import "."

Rectangle {
    id: root

    property bool open_: false
    property string provider: "openrouter"   // openrouter | groq | openai | custom
    property string apiKey: ""
    property string model: ""
    property string baseUrl: ""
    property var providerKeys: ({})
    property var providerBaseUrls: ({})
    property var providerModels: ({})
    property string providerSearch: ""
    property var providerCatalog: [
        { id: "openrouter", name: "OpenRouter", url: "https://openrouter.ai/api/v1", key: "sk-or-...", model: "openrouter/z-ai/glm-5.2:free", note: qsTr("Muchos modelos y proveedores detrás de una sola API") },
        { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", key: "gsk-...", model: "llama-3.3-70b-versatile", note: qsTr("Muy rápido; útil también para STT API") },
        { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", key: "sk-...", model: "gpt-4o-mini", note: qsTr("API oficial compatible OpenAI") },
        { id: "deepseek", name: "DeepSeek", url: "https://api.deepseek.com/v1", key: "sk-...", model: "deepseek-chat", note: qsTr("Modelos DeepSeek OpenAI-compatible") },
        { id: "mistral", name: "Mistral", url: "https://api.mistral.ai/v1", key: "...", model: "mistral-small-latest", note: qsTr("Modelos Mistral") },
        { id: "xai", name: "xAI", url: "https://api.x.ai/v1", key: "xai-...", model: "grok-3-mini", note: qsTr("Modelos Grok vía xAI") },
        { id: "together", name: "Together", url: "https://api.together.xyz/v1", key: "...", model: "meta-llama/Llama-3.3-70B-Instruct-Turbo", note: qsTr("Catálogo amplio OpenAI-compatible") },
        { id: "fireworks", name: "Fireworks", url: "https://api.fireworks.ai/inference/v1", key: "...", model: "accounts/fireworks/models/llama-v3p3-70b-instruct", note: qsTr("Inferencia rápida OpenAI-compatible") },
        { id: "cerebras", name: "Cerebras", url: "https://api.cerebras.ai/v1", key: "csk-...", model: "llama3.1-8b", note: qsTr("Inferencia acelerada") },
        { id: "sambanova", name: "SambaNova", url: "https://api.sambanova.ai/v1", key: "...", model: "Meta-Llama-3.1-8B-Instruct", note: qsTr("Proveedor OpenAI-compatible") },
        { id: "lmstudio", name: "LM Studio", url: "http://127.0.0.1:1234/v1", key: "lm-studio", model: "local-model", note: qsTr("Servidor local; normalmente no requiere key real") },
        { id: "ollama", name: "Ollama", url: "http://127.0.0.1:11434/v1", key: "ollama", model: "llama3.1", note: qsTr("Servidor local OpenAI-compatible") },
        { id: "custom", name: qsTr("Personalizado"), url: "", key: qsTr("tu-api-key"), model: "", note: qsTr("Cualquier endpoint /v1 compatible con OpenAI") }
    ]
    property bool toolCallingEnabled: true
    property bool autoSpeak: false
    property bool autoListen: false
    property bool chimesEnabled: true
    property string theme: "system"  // "system" | "dark" | "light"

    // Piper-tts
    property string piperModel: "es_ES-sharvard-medium"
    property real piperLengthScale: 1.0

    // STT idioma
    property string sttLanguage: "auto"
    // F3-3: micro preferido + nivel en vivo (lo pasa Main).
    property string micDevice: ""
    property var audioDevices: []
    property real micLevel: 0.0

    // Voz avanzada (F1-4): se persisten, aplican al reiniciar el pipeline.
    property string wakeWord: "hey jarvis"
    property real wakeThreshold: 0.5
    property string wakeGreeting: "Sí, dígame"
    property string sttModel: "base"
    // Motor STT: "auto" (API si hay Groq), "api" (forzar nube), "local".
    property string sttBackend: "auto"
    property string sttApiModel: "whisper-large-v3-turbo"

    // Memoria (F5): todo local, nada sale del equipo salvo el prompt actual.
    property bool memoryEnabled: true
    property bool autoSummarize: true
    property var memoryFacts: []

    // Atajos (F1-4): formato "Super+Shift+A".
    property string shortcutToggle: "Super+Shift+A"
    property string shortcutPtt: "Super+Shift+V"
    property string shortcutNewSession: "Ctrl+Shift+K"
    property string shortcutMenu: "Super+Shift+M"

    // Personaje (F8): Desktop Agent settings
    property bool characterEnabled: false
    property string characterMode: "companion"
    property bool characterReducedMotion: false
    property int characterSleepSecs: 240
    property int characterSize: 140
    property string characterAppearance: "capsule"
    property string characterAccentColor: "#0094bb"

    // Tabs
    property string activeTab: "general"  // general | voz | atajos | memoria | personaje

    // Backend
    property string backendUrl: "http://127.0.0.1:8765"
    // Token local F0-3/FIX: módulo generado por main.rs y, como
    // respaldo, env KDE_ASSISTANT_TOKEN.
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
    property var rawConfig: ({})   // config completa cargada del backend
    property var modelList: []     // modelos ofrecidos por la API

    signal closed()
    signal saved()

    visible: open_
    color: Theme.overlayBackdrop
    z: 998

    function show() {
        root.open_ = true
        root.loadConfig()
    }

    function hide() {
        root.open_ = false
        root.closed()
    }

    function providerProfile(p) {
        for (var i = 0; i < providerCatalog.length; ++i) {
            if (providerCatalog[i].id === p) return providerCatalog[i]
        }
        return providerCatalog[0]
    }

    function cloneMap(src) {
        var out = ({})
        if (!src) return out
        for (var k in src) out[k] = src[k]
        return out
    }

    function rememberProviderFields() {
        var keys = cloneMap(root.providerKeys)
        var urls = cloneMap(root.providerBaseUrls)
        var models = cloneMap(root.providerModels)
        keys[root.provider] = apiKeyField.value || ""
        urls[root.provider] = baseUrlField.value || ""
        models[root.provider] = modelCombo.editText || ""
        root.providerKeys = keys
        root.providerBaseUrls = urls
        root.providerModels = models
    }

    function applyProviderFields(p) {
        var profile = providerProfile(p)
        baseUrlField.value = root.providerBaseUrls[p] !== undefined ? root.providerBaseUrls[p] : profile.url
        apiKeyField.value = root.providerKeys[p] !== undefined ? root.providerKeys[p] : ((p === "lmstudio" || p === "ollama") ? profile.key : "")
        modelCombo.editText = root.providerModels[p] !== undefined ? root.providerModels[p] : profile.model
        root.baseUrl = baseUrlField.value
        root.apiKey = apiKeyField.value
        root.model = modelCombo.editText
        root.modelList = []
        modelHint.color = Theme.inkMuted
        modelHint.text = profile.note || ""
    }

    function switchProvider(p) {
        if (p === root.provider) return
        rememberProviderFields()
        root.provider = p
        applyProviderFields(p)
        if (apiKeyField.value !== "" || p === "lmstudio" || p === "ollama") fetchModels()
    }

    function providerBaseUrl(p) {
        return providerProfile(p).url
    }

    function keyPlaceholder() {
        return providerProfile(root.provider).key || "sk-..."
    }

    function providerMatches(profile, query) {
        var q = String(query || "").trim().toLowerCase()
        if (q === "") return true
        var haystack = [
            profile.id || "",
            profile.name || "",
            profile.note || "",
            profile.url || "",
            profile.model || ""
        ].join(" ").toLowerCase()
        return haystack.indexOf(q) >= 0
    }

    // Carga la config del backend y rellena los campos (imperativo, sin bindings)
    function loadConfig() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/config")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                var cfg = JSON.parse(xhr.responseText)
                root.rawConfig = cfg
                if (!cfg.ai) cfg.ai = {}
                if (!cfg.speech) cfg.speech = {}
                if (!cfg.ui) cfg.ui = {}
                if (!cfg.shortcuts) cfg.shortcuts = {}
                if (!cfg.memory) cfg.memory = {}
                if (!cfg.character) cfg.character = {}
                root.provider = cfg.ai.provider || "openrouter"
                root.model = cfg.ai.model || ""
                root.baseUrl = cfg.ai.base_url || ""
                root.providerKeys = cloneMap(cfg.ai.provider_keys || ({}))
                root.providerBaseUrls = cloneMap(cfg.ai.provider_base_urls || ({}))
                root.providerModels = cloneMap(cfg.ai.provider_models || ({}))
                // Migración suave: configs antiguas tenían una sola key/url/modelo.
                if (Object.keys(root.providerKeys).length === 0 && cfg.ai.api_key)
                    root.providerKeys[root.provider] = cfg.ai.api_key
                if (root.providerBaseUrls[root.provider] === undefined && root.baseUrl)
                    root.providerBaseUrls[root.provider] = root.baseUrl
                if (root.providerModels[root.provider] === undefined && root.model)
                    root.providerModels[root.provider] = root.model
                root.apiKey = root.providerKeys[root.provider] || ""
                root.toolCallingEnabled = cfg.ai.enable_tool_calling !== false
                root.autoSpeak = cfg.speech.auto_speak === true
                root.autoListen = cfg.speech.auto_listen === true
                root.chimesEnabled = cfg.speech.chimes_enabled !== false
                root.theme = cfg.ui.theme || "system"
                root.piperModel = cfg.speech.piper_model || "es_ES-sharvard-medium"
                root.piperLengthScale = cfg.speech.piper_length_scale || 1.0
                root.sttLanguage = cfg.speech.stt_language || "auto"
                root.micDevice = cfg.speech.mic_device || ""
                root.wakeWord = cfg.speech.wake_word || "hey jarvis"
                root.wakeThreshold = cfg.speech.wake_word_threshold || 0.5
                root.wakeGreeting = cfg.speech.wake_greeting || "Sí, dígame"
                root.sttModel = cfg.speech.stt_model || "base"
                root.sttBackend = cfg.speech.stt_backend || "auto"
                root.sttApiModel = cfg.speech.stt_api_model || "whisper-large-v3-turbo"
                root.shortcutToggle = cfg.shortcuts.toggle || "Super+Shift+A"
                root.shortcutPtt = cfg.shortcuts.push_to_talk || "Super+Shift+V"
                root.shortcutNewSession = cfg.shortcuts.new_session || "Ctrl+Shift+K"
                root.shortcutMenu = cfg.shortcuts.menu || "Super+Shift+M"
                root.memoryEnabled = cfg.memory.enabled !== false
                root.autoSummarize = cfg.memory.auto_summarize !== false
                root.characterEnabled = cfg.character.enabled === true
                root.characterMode = cfg.character.mode || "companion"
                root.characterReducedMotion = cfg.character.reduced_motion === true
                root.characterSleepSecs = parseInt(cfg.character.sleep_timeout_secs) || 240
                root.characterSize = parseInt(cfg.character.size) || 140
                root.characterAppearance = cfg.character.appearance || "capsule"
                root.characterAccentColor = cfg.character.accent_color || "#0094bb"
                // Volcar a los campos (rompe nada: asignacion directa)
                applyProviderFields(root.provider)
                piperField.value = root.piperModel
                speedSlider.value = root.piperLengthScale
                wakeField.value = root.wakeWord
                thresholdSlider.value = root.wakeThreshold
                greetingField.value = root.wakeGreeting
                toggleField.value = root.shortcutToggle
                pttField.value = root.shortcutPtt
                newSessionField.value = root.shortcutNewSession
                menuField.value = root.shortcutMenu
                // Listar lo que ofrece la API (usa la key recien cargada)
                fetchModels()
                fetchAudioDevices()
                fetchFacts()
            }
        }
        xhr.send()
    }

    // Facts de memoria local (F5): get/add/delete inmediatos.
    function fetchFacts() {
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/memory/facts")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                try {
                    root.memoryFacts = JSON.parse(xhr.responseText)
                } catch (e) { root.memoryFacts = [] }
            }
        }
        xhr.send()
    }

    function addFact() {
        var k = (factKeyField.value || "").trim()
        var v = (factValueField.value || "").trim()
        if (!k || !v) return
        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/memory/facts")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                factKeyField.value = ""
                factValueField.value = ""
                fetchFacts()
            }
        }
        xhr.send(JSON.stringify({ key: k, value: v }))
    }

    function deleteFact(key) {
        var xhr = new XMLHttpRequest()
        xhr.open("DELETE", backendUrl + "/api/memory/facts?key=" + encodeURIComponent(key))
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                fetchFacts()
            }
        }
        xhr.send()
    }

    // Lista los micrófonos del sistema (F3-3).
    function fetchAudioDevices() {
        micHint.text = qsTr("Buscando micrófonos…")
        var xhr = new XMLHttpRequest()
        xhr.open("GET", backendUrl + "/api/audio/devices")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                if (xhr.status === 200) {
                    try {
                        var resp = JSON.parse(xhr.responseText)
                        var list = resp.devices || []
                        root.audioDevices = list
                        var cur = resp.current || root.micDevice
                        var idx = list.indexOf(cur)
                        micCombo.currentIndex = idx >= 0 ? idx : -1
                        micHint.text = list.length > 0
                            ? qsTr("%n micrófono(s)", "", list.length)
                            : qsTr("Sin micrófonos detectados")
                    } catch (e) {
                        micHint.text = qsTr("Respuesta inválida del servidor")
                    }
                } else {
                    micHint.text = qsTr("No se pudo contactar al backend")
                }
            }
        }
        xhr.send()
    }

    // Pide al backend la lista de modelos de la API (usa los campos actuales,
    // sin necesidad de haber guardado). Rellena el desplegable.
    function fetchModels() {
        modelHint.text = qsTr("Cargando modelos…")
        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/ai-models")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                if (xhr.status === 200) {
                    try {
                        var resp = JSON.parse(xhr.responseText)
                        var list = resp.models || []
                        // Preservar lo escrito: cambiar el modelo resetea el campo
                        var keep = modelCombo.editText
                        root.modelList = list
                        modelCombo.editText = keep
                        var toolsNote = root.toolCallingEnabled ? qsTr(" · tools ON") : qsTr(" · tools OFF")
                        if (list.length > 0) {
                            modelHint.text = qsTr("%n modelo(s) disponibles", "", list.length) + toolsNote
                            // El modelo escrito ya no existe en la API (retirado):
                            // avisarlo claro en vez de un 404 mudo al chatear.
                            if (keep && list.indexOf(keep) === -1) {
                                modelHint.text = qsTr("“%1” no está en la API (¿retirado?). Elige uno de la lista.").arg(keep) + toolsNote
                                modelHint.color = Theme.warn
                            } else {
                                modelHint.color = Theme.inkMuted
                            }
                        } else if (resp.error) {
                            modelHint.text = resp.error + toolsNote
                        } else {
                            modelHint.text = qsTr("La API no devolvio modelos") + toolsNote
                        }
                    } catch (e) {
                        modelHint.text = qsTr("Respuesta invalida del servidor")
                    }
                } else if (xhr.status === 401) {
                    modelHint.text = qsTr("No autorizado: reinicia la app")
                } else {
                    modelHint.text = qsTr("No se pudo contactar al backend")
                }
            }
        }
        xhr.send(JSON.stringify({
            base_url: baseUrlField.value,
            api_key: apiKeyField.value,
            provider: root.provider
        }))
    }

    // Guarda la config en el backend (lee los campos por id)
    function saveConfig() {
        var cfg = root.rawConfig
        if (!cfg.ai) cfg.ai = {}
        if (!cfg.speech) cfg.speech = {}
        if (!cfg.ui) cfg.ui = {}
        if (!cfg.shortcuts) cfg.shortcuts = {}
        if (!cfg.memory) cfg.memory = {}
        rememberProviderFields()
        cfg.ai.provider = root.provider
        cfg.ai.provider_keys = cloneMap(root.providerKeys)
        cfg.ai.provider_base_urls = cloneMap(root.providerBaseUrls)
        cfg.ai.provider_models = cloneMap(root.providerModels)
        cfg.ai.api_key = root.providerKeys[root.provider] || ""
        cfg.ai.model = root.providerModels[root.provider] || ""
        cfg.ai.base_url = root.providerBaseUrls[root.provider] || providerBaseUrl(root.provider)
        cfg.ai.enable_tool_calling = toolCallingEnabled
        cfg.speech.auto_speak = autoSpeak
        cfg.speech.auto_listen = autoListen
        cfg.speech.chimes_enabled = chimesEnabled
        cfg.speech.piper_model = piperField.value
        cfg.speech.piper_length_scale = speedSlider.value
        cfg.speech.stt_language = sttLanguage
        cfg.speech.mic_device = micCombo.currentIndex >= 0 ? root.audioDevices[micCombo.currentIndex] : ""
        cfg.speech.wake_word = wakeField.value
        cfg.speech.wake_word_threshold = thresholdSlider.value
        cfg.speech.wake_greeting = greetingField.value
        cfg.speech.stt_model = sttModel
        cfg.speech.stt_backend = sttBackend
        cfg.speech.stt_api_model = sttApiModel
        cfg.shortcuts.toggle = toggleField.value
        cfg.shortcuts.push_to_talk = pttField.value
        cfg.shortcuts.new_session = newSessionField.value
        cfg.shortcuts.menu = menuField.value
        cfg.memory.enabled = memoryEnabled
        cfg.memory.auto_summarize = autoSummarize
        if (!cfg.character) cfg.character = {}
        cfg.character.enabled = characterEnabled
        cfg.character.mode = characterMode
        cfg.character.reduced_motion = characterReducedMotion
        cfg.character.sleep_timeout_secs = characterSleepSecs
        cfg.character.size = characterSize
        cfg.character.appearance = characterAppearance
        cfg.character.accent_color = characterAccentColor
        // Refrescar props locales para que la UI quede consistente
        root.apiKey = cfg.ai.api_key
        root.model = cfg.ai.model
        root.baseUrl = cfg.ai.base_url
        root.piperModel = piperField.value
        root.piperLengthScale = speedSlider.value
        root.micDevice = micCombo.currentIndex >= 0 ? root.audioDevices[micCombo.currentIndex] : ""
        root.wakeWord = wakeField.value
        root.wakeThreshold = thresholdSlider.value
        root.wakeGreeting = greetingField.value
        root.shortcutToggle = toggleField.value
        root.shortcutPtt = pttField.value
        root.shortcutNewSession = newSessionField.value
        root.shortcutMenu = menuField.value
        root.characterEnabled = characterEnabled
        root.characterMode = characterMode
        root.characterReducedMotion = characterReducedMotion
        root.characterSleepSecs = characterSleepSecs
        root.characterSize = characterSize
        root.characterAppearance = characterAppearance
        root.characterAccentColor = characterAccentColor

        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/config")
        xhr.setRequestHeader("Content-Type", "application/json")
        setAuth(xhr)
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                root.saved()
                root.hide()
            }
        }
        xhr.send(JSON.stringify(cfg))
    }

    MouseArea {
        anchors.fill: parent
        onClicked: root.hide()
    }

    // === Panel de settings ===
    Rectangle {
        id: panel
        objectName: "settingsPanel"
        anchors.centerIn: parent
        width: Math.min(parent.width - 24, 760)
        height: Math.min(parent.height - 32, 720)
        radius: Theme.radiusLg + 2
        color: Theme.surface
        border.width: 1
        border.color: Theme.hairline

        // Sombra
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            z: -1
            color: Theme.shadowCard
            opacity: 0.5
            radius: parent.radius + 4
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.AllButtons
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: Theme.spacingLg
            anchors.topMargin: Theme.spacingLg
            anchors.bottomMargin: Theme.spacingLg
            // Margen derecho estrecho (8px): el ScrollBar del form queda
            // pegado al borde del panel. Header, tabs y footer recuperan
            // el aire con Layout.rightMargin: 10 para alinearse con el form.
            anchors.rightMargin: Theme.spacingXs
            spacing: Theme.spacingMd

            // === Header ===
            RowLayout {
                Layout.fillWidth: true
                Layout.rightMargin: 10
                spacing: Theme.spacingXs

                Text {
                    text: qsTr("Configuracion")
                    font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, -0.1)
                    color: Theme.ink
                    Layout.fillWidth: true
                }

                IconButton {
                    iconName: "x-16"
                    iconSize: 18
                    buttonSize: 32
                    backgroundColor: "transparent"
                    iconColor: Theme.ink
                    onClicked: root.hide()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: Theme.spacingSm
                ColumnLayout {
                    Layout.preferredWidth: panel.width < 500 ? 88 : 132
                    Layout.fillHeight: true
                    spacing: Theme.spacingXs
                    Repeater {
                        model: [
                            {key:"general", label:qsTr("General")},
                            {key:"voz", label:qsTr("Voz")},
                            {key:"atajos", label:qsTr("Atajos")},
                            {key:"memoria", label:qsTr("Memoria")},
                            {key:"personaje", label:qsTr("Personaje")}
                        ]
                        delegate: Button {
                            required property var modelData
                            Layout.fillWidth: true
                            text: modelData.label
                            checkable: true
                            checked: root.activeTab === modelData.key
                            Accessible.name: text
                            onClicked: {
                                root.activeTab = modelData.key
                                formScroll.contentItem.contentY = 0
                            }
                            contentItem: Text {
                                text: parent.text
                                color: parent.checked ? Theme.inkOnPrimary : Theme.ink
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0)
                                elide: Text.ElideRight
                            }
                            background: Rectangle {
                                radius: Theme.radiusSm
                                color: parent.checked ? Theme.primary : parent.hovered ? Theme.surfaceHover : Theme.surfacePearl
                                border.color: parent.activeFocus ? Theme.primary : "transparent"
                            }
                        }
                    }
                    Item { Layout.fillHeight: true }
                }

            // === Form ===
            ScrollView {
                id: formScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                ScrollBar.vertical: ScrollBar {
                    policy: ScrollBar.AsNeeded
                    // El ScrollView no dimensiona ni coloca las barras custom
                    // (quedan en x=0,h=0): anclas explicitas a la derecha y
                    // a altura completa del form. En ListView no hace falta.
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.right: parent.right
                    anchors.rightMargin: 2
                    width: 6
                    padding: 0
                    // Sin pista ni linea separadora: solo el thumb flotante.
                    background: Item {}
                    contentItem: Rectangle {
                        radius: 3
                        color: Theme.inkMuted
                        opacity: parent.active ? 0.65 : 0.35
                        Behavior on opacity {
                            NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
                        }
                    }
                }

                ColumnLayout {
                    // 10px de aire a la derecha para el thumb overlay de 6px.
                    width: formScroll.availableWidth - 10
                    spacing: Theme.spacingMd

                    // --- TAB: General ---
                    ColumnLayout {
                        visible: root.activeTab === "general"
                        Layout.fillWidth: true
                        spacing: Theme.spacingMd

                    // AI section
                    Text {
                        text: qsTr("Inteligencia Artificial")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Proveedor")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                        color: Theme.inkMuted
                        Layout.fillWidth: true
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 64
                        radius: Theme.radiusMd
                        color: Theme.surfacePearl
                        border.width: 1
                        border.color: Theme.hairline

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: Theme.spacingSm
                            spacing: Theme.spacingSm

                            Rectangle {
                                Layout.preferredWidth: 36
                                Layout.preferredHeight: 36
                                radius: Theme.radiusPill
                                color: Qt.rgba(0.16, 0.58, 1.0, 0.16)
                                Octicon {
                                    anchors.centerIn: parent
                                    name: "hubot-16"
                                    size: 16
                                    color: Theme.primary
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    Layout.fillWidth: true
                                    text: providerProfile(root.provider).name
                                    color: Theme.ink
                                    font: Theme.font(Theme.fontSizeBody, Theme.weightBold, Theme.lsBody)
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: providerProfile(root.provider).note
                                    color: Theme.inkMuted
                                    font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                                    elide: Text.ElideRight
                                }
                            }

                            Text {
                                text: qsTr("Activo")
                                color: Theme.primary
                                font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.2)
                            }
                        }
                    }

                    TextField {
                        id: providerSearchField
                        Layout.fillWidth: true
                        Layout.preferredHeight: 38
                        text: root.providerSearch
                        placeholderText: qsTr("Buscar proveedor por nombre, API o modelo…")
                        placeholderTextColor: Theme.inkMuted
                        color: Theme.ink
                        font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                        selectByMouse: true
                        Accessible.name: qsTr("Buscar proveedor de IA")
                        onTextChanged: root.providerSearch = text
                        Keys.onDownPressed: {
                            providerList.currentIndex = 0
                            providerList.forceActiveFocus()
                        }
                        background: Rectangle {
                            radius: Theme.radiusMd
                            color: providerSearchField.activeFocus ? Theme.surface : Theme.surfacePearl
                            border.width: 1
                            border.color: providerSearchField.activeFocus ? Theme.primary : Theme.hairline
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: Math.min(244, Math.max(48, providerList.count * 48))
                        radius: Theme.radiusMd
                        color: Theme.surfacePearl
                        border.width: 1
                        border.color: Theme.hairline
                        clip: true

                        ListView {
                            id: providerList
                            anchors.fill: parent
                            anchors.margins: 4
                            clip: true
                            currentIndex: 0
                            model: root.providerCatalog.filter(function(profile) {
                                return root.providerMatches(profile, providerSearchField.text)
                            })
                            ScrollBar.vertical: ScrollBar {
                                policy: ScrollBar.AsNeeded
                            }
                            Keys.onReturnPressed: {
                                if (currentIndex >= 0 && currentIndex < count)
                                    root.switchProvider(model[currentIndex].id)
                            }
                            delegate: Button {
                                id: providerChoice
                                required property var modelData
                                width: providerList.width
                                height: 48
                                checkable: true
                                checked: root.provider === modelData.id
                                Accessible.name: modelData.name
                                onClicked: {
                                    root.switchProvider(modelData.id)
                                    providerSearchField.forceActiveFocus()
                                }
                                contentItem: RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: Theme.spacingSm
                                    anchors.rightMargin: Theme.spacingSm
                                    spacing: Theme.spacingSm
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 1
                                        Text {
                                            Layout.fillWidth: true
                                            text: modelData.name
                                            color: providerChoice.checked ? Theme.inkOnPrimary : Theme.ink
                                            font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0)
                                            elide: Text.ElideRight
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            text: modelData.url
                                            color: providerChoice.checked ? Qt.rgba(1,1,1,0.72) : Theme.inkMuted
                                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                                            elide: Text.ElideMiddle
                                        }
                                    }
                                    Text {
                                        text: modelData.model
                                        color: providerChoice.checked ? Qt.rgba(1,1,1,0.8) : Theme.inkMuted
                                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                                        elide: Text.ElideRight
                                        Layout.maximumWidth: Math.max(84, providerList.width * 0.32)
                                    }
                                }
                                background: Rectangle {
                                    radius: Theme.radiusSm
                                    color: parent.checked ? Theme.primary : parent.hovered ? Theme.surfaceHover : Theme.surfacePearl
                                    Behavior on color {
                                        ColorAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
                                    }
                                }
                            }
                        }

                        Text {
                            anchors.centerIn: parent
                            visible: providerList.count === 0
                            text: qsTr("Sin proveedores para esa búsqueda")
                            color: Theme.inkMuted
                            font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                        }
                    }

                    SettingsField {
                        id: baseUrlField
                        Layout.fillWidth: true
                        label: qsTr("API URL")
                        placeholder: "https://..."
                    }

                    SettingsField {
                        id: apiKeyField
                        Layout.fillWidth: true
                        label: qsTr("API Key")
                        placeholder: keyPlaceholder()
                        isPassword: true
                    }

                    // Modelo: un solo control editable (muestra, escribe y elige)
                    Text {
                        text: qsTr("Modelo")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                        color: Theme.inkMuted
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs

                        ModelPicker {
                            id: modelCombo
                            Layout.fillWidth: true
                            Layout.preferredHeight: 40
                            models: root.modelList
                        }

                        IconButton {
                            iconName: "sync-16"
                            iconSize: 18
                            buttonSize: 36
                            backgroundColor: Theme.surfacePearl
                            iconColor: Theme.ink
                            onClicked: fetchModels()
                        }
                    }

                    Text {
                        id: modelHint
                        text: ""
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    SettingsToggle {
                        Layout.fillWidth: true
                        label: qsTr("Tool calling (agente)")
                        description: qsTr("Permitir al asistente ejecutar herramientas")
                        active: root.toolCallingEnabled
                        onToggled: root.toolCallingEnabled = !root.toolCallingEnabled
                    }

                    // Tema
                    Text {
                        text: qsTr("Apariencia")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingMd
                        Layout.fillWidth: true
                    }

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("Sistema")
                            active: root.theme === "system"
                            onClicked: root.theme = "system"
                        }
                        PillButton {
                            text: qsTr("Oscuro")
                            active: root.theme === "dark"
                            onClicked: root.theme = "dark"
                        }
                        PillButton {
                            text: qsTr("Claro")
                            active: root.theme === "light"
                            onClicked: root.theme = "light"
                        }
                    }

                    } // General

                    // --- TAB: Voz ---
                    ColumnLayout {
                        visible: root.activeTab === "voz"
                        Layout.fillWidth: true
                        spacing: Theme.spacingSm

                    // Voz
                    Text {
                        text: qsTr("Voz (piper-tts neural)")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Reconocimiento (STT)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.fillWidth: true
                    }

                    // Idioma STT
                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("Auto")
                            active: root.sttLanguage === "auto"
                            onClicked: root.sttLanguage = "auto"
                        }
                        PillButton {
                            text: qsTr("Español")
                            active: root.sttLanguage === "es"
                            onClicked: root.sttLanguage = "es"
                        }
                        PillButton {
                            text: qsTr("Inglés")
                            active: root.sttLanguage === "en"
                            onClicked: root.sttLanguage = "en"
                        }
                    }

                    Text {
                        text: qsTr("Motor STT (transcripción de voz)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingSm
                        Layout.fillWidth: true
                    }

                    // Motor STT: Groq API (rápido, ~1s) o whisper local.
                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("Auto")
                            active: root.sttBackend === "auto"
                            onClicked: root.sttBackend = "auto"
                        }
                        PillButton {
                            text: qsTr("API Groq (rápido)")
                            active: root.sttBackend === "api"
                            onClicked: root.sttBackend = "api"
                        }
                        PillButton {
                            text: qsTr("Local (sin nube)")
                            active: root.sttBackend === "local"
                            onClicked: root.sttBackend = "local"
                        }
                    }
                    Text {
                        text: qsTr("Auto usa la API de Groq si hay key; si no, whisper local. El audio de tus turnos de voz sale a Groq en modo API.")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    // Modelo del API (solo cuando el backend no es local)
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        visible: root.sttBackend !== "local"

                        ComboBox {
                            id: sttApiModelCombo
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            model: ["whisper-large-v3-turbo", "whisper-large-v3"]
                            Component.onCompleted: {
                                currentIndex = root.sttApiModel === "whisper-large-v3" ? 1 : 0
                            }
                            onActivated: root.sttApiModel = currentText
                            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                            background: Rectangle {
                                radius: Theme.radiusMd
                                color: sttApiModelCombo.hovered || sttApiModelCombo.activeFocus ? Theme.surface : Theme.surfacePearl
                                border.width: 1
                                border.color: sttApiModelCombo.activeFocus ? Theme.primary : Theme.hairline
                            }
                        }
                    }
                    Text {
                        visible: root.sttBackend !== "local"
                        text: qsTr("turbo: ~0.2x tiempo real, $0.04/h. large-v3: un poco más preciso, $0.111/h.")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Modelo STT local (sin nube, requiere reiniciar)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingSm
                        Layout.fillWidth: true
                    }

                    // Modelo STT: tiny rápido, base preciso.
                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("tiny (rápido)")
                            active: root.sttModel === "tiny"
                            onClicked: root.sttModel = "tiny"
                        }
                        PillButton {
                            text: qsTr("base (preciso)")
                            active: root.sttModel === "base"
                            onClicked: root.sttModel = "base"
                        }
                    }
                    Text {
                        text: qsTr("tiny ≈ 75MB, base ≈ 140MB. Solo se usa con motor Local o como respaldo si la API falla.")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    Text {
                        text: qsTr("Micrófono (se aplica al reiniciar)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingSm
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs

                        ComboBox {
                            id: micCombo
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            model: root.audioDevices
                            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                            background: Rectangle {
                                radius: Theme.radiusMd
                                color: micCombo.hovered || micCombo.activeFocus ? Theme.surface : Theme.surfacePearl
                                border.width: 1
                                border.color: micCombo.activeFocus ? Theme.primary : Theme.hairline
                            }
                        }

                        IconButton {
                            iconName: "sync-16"
                            iconSize: 18
                            buttonSize: 36
                            backgroundColor: Theme.surfacePearl
                            iconColor: Theme.ink
                            onClicked: fetchAudioDevices()
                        }
                    }

                    Text {
                        id: micHint
                        text: ""
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    // VU del micro en vivo (viene de Main por SSE).
                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 28
                        RowLayout {
                            anchors.fill: parent
                            spacing: Theme.spacingSm
                            Text {
                                text: qsTr("Nivel")
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                color: Theme.ink
                                Layout.preferredWidth: 80
                            }
                            ProgressBar {
                                Layout.fillWidth: true
                                from: 0
                                to: 1
                                value: root.micLevel
                            }
                        }
                    }

                    Text {
                        text: qsTr("Sintesis (TTS)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingSm
                        Layout.fillWidth: true
                    }

                    SettingsField {
                        id: piperField
                        Layout.fillWidth: true
                        label: qsTr("Modelo de voz")
                        placeholder: "es_ES-sharvard-medium"
                    }

                    Text {
                        text: qsTr("Descarga modelos desde huggingface.co/rhasspy/piper-voices y copialos a ~/.local/share/kde-assistant/models/piper/")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }

                    // Speed slider
                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 36
                        RowLayout {
                            anchors.fill: parent
                            spacing: Theme.spacingSm

                            Text {
                                text: qsTr("Velocidad")
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                color: Theme.ink
                                Layout.preferredWidth: 80
                            }

                            Slider {
                                id: speedSlider
                                Layout.fillWidth: true
                                from: 0.5
                                to: 1.5
                                value: 1.0
                            }

                            Text {
                                text: speedSlider.value.toFixed(2) + "x"
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                color: Theme.inkMuted
                                Layout.preferredWidth: 40
                            }
                        }
                    }

                    SettingsToggle {
                        Layout.fillWidth: true
                        label: qsTr("Hablar también las respuestas escritas")
                        description: qsTr("Las respuestas por voz siempre se dictan")
                        active: root.autoSpeak
                        onToggled: root.autoSpeak = !root.autoSpeak
                    }

                    SettingsToggle {
                        Layout.fillWidth: true
                        label: qsTr("Escucha continua")
                        description: qsTr("Tras responder por voz, sigue escuchando (sin wake word)")
                        active: root.autoListen
                        onToggled: root.autoListen = !root.autoListen
                    }

                    SettingsToggle {
                        Layout.fillWidth: true
                        label: qsTr("Reproducir chimes")
                        description: qsTr("Sonidos sutiles al activar/desactivar voz")
                        active: root.chimesEnabled
                        onToggled: root.chimesEnabled = !root.chimesEnabled
                    }

                    Text {
                        text: qsTr("Wake word (requiere reiniciar)")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingSm
                        Layout.fillWidth: true
                    }
                    SettingsField {
                        id: wakeField
                        Layout.fillWidth: true
                        label: qsTr("Palabra de activación")
                        placeholder: "hey jarvis"
                    }
                    SettingsField {
                        id: greetingField
                        Layout.fillWidth: true
                        label: qsTr("Saludo al activar")
                        placeholder: "Sí, dígame"
                    }
                    Item {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 36
                        RowLayout {
                            anchors.fill: parent
                            spacing: Theme.spacingSm
                            Text {
                                text: qsTr("Umbral")
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                color: Theme.ink
                                Layout.preferredWidth: 80
                            }
                            Slider {
                                id: thresholdSlider
                                Layout.fillWidth: true
                                from: 0.1
                                to: 0.95
                                value: 0.5
                            }
                            Text {
                                text: thresholdSlider.value.toFixed(2)
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                color: Theme.inkMuted
                                Layout.preferredWidth: 40
                            }
                        }
                    }
                    Text {
                        text: qsTr("Umbral alto = más exigente (menos falsos positivos).")
                        font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                        color: Theme.inkMuted
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                    } // Voz

                    // --- TAB: Atajos ---
                    ColumnLayout {
                        visible: root.activeTab === "atajos"
                        Layout.fillWidth: true
                        spacing: Theme.spacingSm

                        Text {
                            text: qsTr("Atajos globales")
                            font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.fillWidth: true
                        }
                        SettingsField {
                            id: toggleField
                            Layout.fillWidth: true
                            label: qsTr("Mostrar / ocultar")
                            placeholder: "Super+Shift+A"
                        }
                        SettingsField {
                            id: pttField
                            Layout.fillWidth: true
                            label: qsTr("Dictar (mantener)")
                            placeholder: "Super+Shift+V"
                        }
                        SettingsField {
                            id: newSessionField
                            Layout.fillWidth: true
                            label: qsTr("Nueva sesión")
                            placeholder: "Ctrl+Shift+K"
                        }
                        SettingsField {
                            id: menuField
                            Layout.fillWidth: true
                            label: qsTr("Abrir menú")
                            placeholder: "Super+Shift+M"
                        }
                        Text {
                            text: qsTr("Formato: Super/Shift/Ctrl/Alt + letra, dígito o F1-F12. Se aplican al guardar (sin reiniciar). En Wayland algunos atajos los reserva el compositor.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }
                    } // Atajos

                    // --- TAB: Memoria (F5) ---
                    ColumnLayout {
                        visible: root.activeTab === "memoria"
                        Layout.fillWidth: true
                        spacing: Theme.spacingSm

                        Text {
                            text: qsTr("Memoria local")
                            font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.fillWidth: true
                        }
                        Text {
                            text: qsTr("Todo se guarda en SQLite local. Solo el prompt actual viaja a la API.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        SettingsToggle {
                            Layout.fillWidth: true
                            label: qsTr("Recordar datos")
                            description: qsTr("Inyectar mis datos en las respuestas")
                            active: root.memoryEnabled
                            onToggled: root.memoryEnabled = !root.memoryEnabled
                        }

                        SettingsToggle {
                            Layout.fillWidth: true
                            label: qsTr("Resumir hilos largos")
                            description: qsTr("Resumen automático en segundo plano")
                            active: root.autoSummarize
                            onToggled: root.autoSummarize = !root.autoSummarize
                        }

                        Text {
                            text: qsTr("Mis datos")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.fillWidth: true
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Theme.spacingXs
                            visible: root.memoryFacts.length > 0

                            Repeater {
                                model: root.memoryFacts
                                delegate: RowLayout {
                                    required property var modelData
                                    Layout.fillWidth: true
                                    spacing: Theme.spacingXs

                                    Text {
                                        text: (modelData.key || "") + ": " + (modelData.value || "")
                                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                        color: Theme.ink
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    IconButton {
                                        Layout.preferredWidth: 28
                                        Layout.preferredHeight: 28
                                        iconName: "trash-16"
                                        iconSize: 14
                                        buttonSize: 28
                                        backgroundColor: "transparent"
                                        iconColor: Theme.inkMuted
                                        onClicked: root.deleteFact(modelData.key)
                                    }
                                }
                            }
                        }
                        Text {
                            visible: root.memoryFacts.length === 0
                            text: qsTr("Sin datos guardados. Añade p. ej. nombre → tu nombre.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        SettingsField {
                            id: factKeyField
                            Layout.fillWidth: true
                            label: qsTr("Clave (p. ej. nombre)")
                            placeholder: "nombre"
                        }
                        SettingsField {
                            id: factValueField
                            Layout.fillWidth: true
                            label: qsTr("Valor")
                            placeholder: qsTr("Tu valor")
                        }
                        PillButton {
                            text: qsTr("Añadir dato")
                            iconName: "plus-16"
                            onClicked: root.addFact()
                        }
                    } // Memoria

                    // --- TAB: Personaje (F8: Desktop Agent) ---
                    ColumnLayout {
                        visible: root.activeTab === "personaje"
                        Layout.fillWidth: true
                        spacing: Theme.spacingSm

                        Text {
                            text: qsTr("Personaje de escritorio")
                            font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.fillWidth: true
                        }
                        Text {
                            text: qsTr("Requiere reinicio de la app para aplicar cambios.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.warn
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        SettingsToggle {
                            Layout.fillWidth: true
                            label: qsTr("Activar personaje")
                            description: qsTr("Muestra el agente visual en el escritorio")
                            active: root.characterEnabled
                            onToggled: root.characterEnabled = !root.characterEnabled
                        }

                        Text {
                            text: qsTr("Modo de presencia")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.topMargin: Theme.spacingSm
                            Layout.fillWidth: true
                        }

                        Flow {
                            Layout.fillWidth: true
                            spacing: Theme.spacingXs
                            PillButton {
                                text: qsTr("Mínimo")
                                active: root.characterMode === "minimal"
                                onClicked: root.characterMode = "minimal"
                            }
                            PillButton {
                                text: qsTr("Reactivo")
                                active: root.characterMode === "reactive"
                                onClicked: root.characterMode = "reactive"
                            }
                            PillButton {
                                text: qsTr("Compañero")
                                active: root.characterMode === "companion"
                                onClicked: root.characterMode = "companion"
                            }
                            PillButton {
                                text: qsTr("Cinemático")
                                active: root.characterMode === "cinematic"
                                onClicked: root.characterMode = "cinematic"
                            }
                        }
                        Text {
                            text: qsTr("Mínimo: solo expresiones clave. Reactivo: mira al cursor. Compañero: se mueve e interactúa. Cinemático: escenas completas.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                        SettingsToggle {
                            Layout.fillWidth: true
                            label: qsTr("Reducir movimiento")
                            description: qsTr("Desactiva animaciones complejas para accesibilidad")
                            active: root.characterReducedMotion
                            onToggled: root.characterReducedMotion = !root.characterReducedMotion
                        }

                        Text {
                            text: qsTr("Tiempo de sueño (segundos)")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.topMargin: Theme.spacingSm
                            Layout.fillWidth: true
                        }
                        Item {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            RowLayout {
                                anchors.fill: parent
                                spacing: Theme.spacingSm
                                Text {
                                    text: qsTr("Inactividad")
                                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                    color: Theme.ink
                                    Layout.preferredWidth: 80
                                }
                                Slider {
                                    id: sleepSlider
                                    Layout.fillWidth: true
                                    from: 30
                                    to: 600
                                    value: root.characterSleepSecs
                                    stepSize: 30
                                    onValueChanged: root.characterSleepSecs = sleepSlider.value
                                }
                                Text {
                                    text: sleepSlider.value + "s"
                                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                    color: Theme.inkMuted
                                    Layout.preferredWidth: 50
                                }
                            }
                        }

                        Text {
                            text: qsTr("Tamaño (píxeles)")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.topMargin: Theme.spacingSm
                            Layout.fillWidth: true
                        }
                        Item {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            RowLayout {
                                anchors.fill: parent
                                spacing: Theme.spacingSm
                                Text {
                                    text: qsTr("Tamaño")
                                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
                                    color: Theme.ink
                                    Layout.preferredWidth: 80
                                }
                                Slider {
                                    id: sizeSlider
                                    Layout.fillWidth: true
                                    from: 80
                                    to: 240
                                    value: root.characterSize
                                    stepSize: 10
                                    onValueChanged: root.characterSize = sizeSlider.value
                                }
                                Text {
                                    text: sizeSlider.value + "px"
                                    font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                    color: Theme.inkMuted
                                    Layout.preferredWidth: 50
                                }
                            }
                        }

                        Text {
                            text: qsTr("Aspecto")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.topMargin: Theme.spacingSm
                            Layout.fillWidth: true
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: Theme.spacingXs
                            PillButton {
                                text: qsTr("Cápsula")
                                active: root.characterAppearance === "capsule"
                                onClicked: root.characterAppearance = "capsule"
                            }
                            PillButton {
                                text: qsTr("Redondo")
                                active: root.characterAppearance === "round"
                                onClicked: root.characterAppearance = "round"
                            }
                            PillButton {
                                text: qsTr("Pebble")
                                active: root.characterAppearance === "pebble"
                                onClicked: root.characterAppearance = "pebble"
                            }
                            PillButton {
                                text: qsTr("Compacto")
                                active: root.characterAppearance === "compact"
                                onClicked: root.characterAppearance = "compact"
                            }
                        }

                        Text {
                            text: qsTr("Color")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightBold, 0.4)
                            color: Theme.inkMuted
                            Layout.topMargin: Theme.spacingSm
                            Layout.fillWidth: true
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: Theme.spacingXs
                            Repeater {
                                model: [
                                    { name: qsTr("Cian"), value: "#0094bb" },
                                    { name: qsTr("Azul"), value: "#2997ff" },
                                    { name: qsTr("Violeta"), value: "#5e5ce6" },
                                    { name: qsTr("Rosa"), value: "#bf5af2" },
                                    { name: qsTr("Verde"), value: "#30d158" },
                                    { name: qsTr("Naranja"), value: "#ff9f0a" }
                                ]
                                delegate: Button {
                                    required property var modelData
                                    text: modelData.name
                                    checkable: true
                                    checked: root.characterAccentColor.toLowerCase() === modelData.value
                                    onClicked: root.characterAccentColor = modelData.value
                                    contentItem: Row {
                                        spacing: 6
                                        Rectangle {
                                            width: 14
                                            height: 14
                                            radius: 7
                                            color: modelData.value
                                            anchors.verticalCenter: parent.verticalCenter
                                        }
                                        Text {
                                            text: parent.parent.text
                                            color: parent.parent.checked ? Theme.inkOnPrimary : Theme.ink
                                            font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0)
                                            anchors.verticalCenter: parent.verticalCenter
                                        }
                                    }
                                    background: Rectangle {
                                        radius: Theme.radiusSm
                                        color: parent.checked ? Theme.primary : parent.hovered ? Theme.surfaceHover : Theme.surfacePearl
                                        border.color: parent.activeFocus ? Theme.primary : "transparent"
                                    }
                                }
                            }
                        }
                        Text {
                            text: qsTr("Se mantiene la misma librería/identidad Blobatar; estos controles solo cambian forma visual y tinte local.")
                            font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, 0)
                            color: Theme.inkMuted
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                        }

                    } // Personaje
                }
            }

            } // Sidebar + form

            // === Footer ===
            RowLayout {
                Layout.fillWidth: true
                Layout.rightMargin: 10
                spacing: Theme.spacingXs

                PillButton {
                    text: qsTr("Cancelar")
                    onClicked: root.hide()
                }
                Item { Layout.fillWidth: true }
                AppleButton {
                    text: qsTr("Guardar")
                    iconName: "check-16"
                    variant: "primary"
                    onClicked: {
                        root.saveConfig()
                    }
                }
            }
        }
    }
}
