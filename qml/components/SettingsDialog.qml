// SettingsDialog.qml - Dialog modal de configuracion
// Acceso desde el SessionDrawer ("Configuracion") o desde la X del title
// NOTA: los campos se leen/escriben de forma imperativa (por id) para
// evitar peleas de bindings con lo que escribe el usuario.

import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
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

    // Memoria (F5): todo local, nada sale del equipo salvo el prompt actual.
    property bool memoryEnabled: true
    property bool autoSummarize: true
    property var memoryFacts: []

    // Atajos (F1-4): formato "Super+Shift+A".
    property string shortcutToggle: "Super+Shift+A"
    property string shortcutPtt: "Super+Shift+V"
    property string shortcutNewSession: "Ctrl+Shift+K"

    // Tabs
    property string activeTab: "general"  // general | voz | atajos | memoria

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
    color: Qt.rgba(0, 0, 0, 0.5)
    z: 998

    function show() {
        root.open_ = true
        root.loadConfig()
    }

    function hide() {
        root.open_ = false
        root.closed()
    }

    function providerBaseUrl(p) {
        if (p === "groq") return "https://api.groq.com/openai/v1"
        if (p === "openai") return "https://api.openai.com/v1"
        if (p === "openrouter") return "https://openrouter.ai/api/v1"
        return baseUrlField.value || ""
    }

    function keyPlaceholder() {
        if (root.provider === "groq") return "gsk-..."
        if (root.provider === "openai") return "sk-..."
        if (root.provider === "custom") return "tu-api-key"
        return "sk-or-..."
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
                root.provider = cfg.ai.provider || "openrouter"
                root.apiKey = cfg.ai.api_key || ""
                root.model = cfg.ai.model || ""
                root.baseUrl = cfg.ai.base_url || ""
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
                root.shortcutToggle = cfg.shortcuts.toggle || "Super+Shift+A"
                root.shortcutPtt = cfg.shortcuts.push_to_talk || "Super+Shift+V"
                root.shortcutNewSession = cfg.shortcuts.new_session || "Ctrl+Shift+K"
                root.memoryEnabled = cfg.memory.enabled !== false
                root.autoSummarize = cfg.memory.auto_summarize !== false
                // Volcar a los campos (rompe nada: asignacion directa)
                baseUrlField.value = root.baseUrl
                apiKeyField.value = root.apiKey
                modelCombo.editText = root.model
                piperField.value = root.piperModel
                speedSlider.value = root.piperLengthScale
                wakeField.value = root.wakeWord
                thresholdSlider.value = root.wakeThreshold
                greetingField.value = root.wakeGreeting
                toggleField.value = root.shortcutToggle
                pttField.value = root.shortcutPtt
                newSessionField.value = root.shortcutNewSession
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
        cfg.ai.provider = root.provider
        cfg.ai.api_key = apiKeyField.value
        cfg.ai.model = modelCombo.editText
        cfg.ai.base_url = baseUrlField.value
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
        cfg.shortcuts.toggle = toggleField.value
        cfg.shortcuts.push_to_talk = pttField.value
        cfg.shortcuts.new_session = newSessionField.value
        cfg.memory.enabled = memoryEnabled
        cfg.memory.auto_summarize = autoSummarize
        // Refrescar props locales para que la UI quede consistente
        root.apiKey = apiKeyField.value
        root.model = modelCombo.editText
        root.baseUrl = baseUrlField.value
        root.piperModel = piperField.value
        root.piperLengthScale = speedSlider.value
        root.micDevice = micCombo.currentIndex >= 0 ? root.audioDevices[micCombo.currentIndex] : ""
        root.wakeWord = wakeField.value
        root.wakeThreshold = thresholdSlider.value
        root.wakeGreeting = greetingField.value
        root.shortcutToggle = toggleField.value
        root.shortcutPtt = pttField.value
        root.shortcutNewSession = newSessionField.value

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
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 480)
        height: Math.min(parent.height - 80, 580)
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

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacingLg
            spacing: Theme.spacingMd

            // === Header ===
            RowLayout {
                Layout.fillWidth: true
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

            Flow {
                Layout.fillWidth: true
                spacing: Theme.spacingXs
                PillButton {
                    text: qsTr("General")
                    active: root.activeTab === "general"
                    onClicked: root.activeTab = "general"
                }
                PillButton {
                    text: qsTr("Voz")
                    active: root.activeTab === "voz"
                    onClicked: root.activeTab = "voz"
                }
                PillButton {
                    text: qsTr("Atajos")
                    active: root.activeTab === "atajos"
                    onClicked: root.activeTab = "atajos"
                }
                PillButton {
                    text: qsTr("Memoria")
                    active: root.activeTab === "memoria"
                    onClicked: root.activeTab = "memoria"
                }
            }

            // === Form ===
            ScrollView {
                id: formScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                    width: formScroll.availableWidth
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

                    Flow {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs
                        PillButton {
                            text: qsTr("OpenRouter")
                            active: root.provider === "openrouter"
                            onClicked: {
                                root.provider = "openrouter"
                                baseUrlField.value = providerBaseUrl("openrouter")
                                if (apiKeyField.value !== "") fetchModels()
                            }
                        }
                        PillButton {
                            text: qsTr("Groq")
                            active: root.provider === "groq"
                            onClicked: {
                                root.provider = "groq"
                                baseUrlField.value = providerBaseUrl("groq")
                                if (apiKeyField.value !== "") fetchModels()
                            }
                        }
                        PillButton {
                            text: qsTr("OpenAI")
                            active: root.provider === "openai"
                            onClicked: {
                                root.provider = "openai"
                                baseUrlField.value = providerBaseUrl("openai")
                                if (apiKeyField.value !== "") fetchModels()
                            }
                        }
                        PillButton {
                            text: qsTr("Personalizado")
                            active: root.provider === "custom"
                            onClicked: root.provider = "custom"
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

                        ComboBox {
                            id: modelCombo
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            editable: true
                            model: root.modelList
                            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                            // Sin bindings en editText: se fija por codigo en
                            // loadConfig/fetchModels y se lee en saveConfig.
                            onActivated: function(index) {
                                root.model = modelCombo.currentText
                            }
                            onAccepted: {
                                root.model = modelCombo.editText
                            }
                            background: Rectangle {
                                radius: Theme.radiusMd
                                color: modelCombo.hovered || modelCombo.activeFocus ? Theme.surface : Theme.surfacePearl
                                border.width: 1
                                border.color: modelCombo.activeFocus ? Theme.primary : Theme.hairline
                            }
                            // NOTA: sin contentItem personalizado; el estilo del
                            // sistema (Breeze) espera un TextInput con
                            // positionToRectangle() y rompe con un Text plano.
                            // Chevron vectorial propio (el glifo textual sale como emoji).
                            indicator: Item {
                                x: modelCombo.width - width - 12
                                y: (modelCombo.height - height) / 2
                                width: 12
                                height: 8
                                Shape {
                                    anchors.fill: parent
                                    antialiasing: true
                                    ShapePath {
                                        strokeColor: Theme.inkMuted
                                        strokeWidth: 2
                                        fillColor: "transparent"
                                        capStyle: ShapePath.RoundCap
                                        joinStyle: ShapePath.RoundJoin
                                        startX: 1
                                        startY: 1
                                        PathLine { x: 6; y: 7 }
                                        PathLine { x: 11; y: 1 }
                                    }
                                }
                            }
                            popup: Popup {
                                y: modelCombo.height + 4
                                width: modelCombo.width
                                padding: 4
                                background: Rectangle {
                                    color: Theme.surface
                                    border.color: Theme.hairline
                                    border.width: 1
                                    radius: Theme.radiusMd
                                }
                                // Altura por nº de items (contentHeight colapsaba a 0
                                // y la lista se abría vacía).
                                contentItem: Item {
                                    implicitWidth: modelCombo.width - 8
                                    implicitHeight: modelCombo.count > 0 ? Math.min(modelCombo.count * 36, 216) + 8 : 44
                                    ListView {
                                        anchors.fill: parent
                                        clip: true
                                        model: modelCombo.delegateModel
                                        currentIndex: modelCombo.highlightedIndex
                                        ScrollBar.vertical.policy: ScrollBar.AsNeeded
                                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                                        delegate: ItemDelegate {
                                            width: ListView.view.width
                                            height: 36
                                            text: modelData
                                            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                                            highlighted: modelCombo.highlightedIndex === index
                                        }
                                    }
                                    Text {
                                        visible: modelCombo.count === 0
                                        anchors.centerIn: parent
                                        text: qsTr("Sin modelos — pulsa recargar")
                                        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                                        color: Theme.inkMuted
                                    }
                                }
                            }
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
                        text: qsTr("Modelo STT (requiere reiniciar)")
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
                        text: qsTr("tiny ≈ 75MB, base ≈ 140MB. Se descarga solo al reiniciar.")
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
                }
            }

            // === Footer ===
            RowLayout {
                Layout.fillWidth: true
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
