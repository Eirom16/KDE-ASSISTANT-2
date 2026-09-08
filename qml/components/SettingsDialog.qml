// SettingsDialog.qml - Dialog modal de configuracion
// Acceso desde el SessionDrawer ("Configuracion") o desde la X del title
// NOTA: los campos se leen/escriben de forma imperativa (por id) para
// evitar peleas de bindings con lo que escribe el usuario.

import QtQuick
import QtQuick.Controls
import qml 1.0
import QtQuick.Layouts
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
    property bool chimesEnabled: true
    property string theme: "system"  // "system" | "dark" | "light"

    // Piper-tts
    property string piperModel: "es_ES-sharvard-medium"
    property real piperLengthScale: 1.0

    // STT idioma
    property string sttLanguage: "auto"

    // Backend
    property string backendUrl: "http://127.0.0.1:8765"
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

    function modelPlaceholder() {
        if (root.provider === "groq") return "llama-3.3-70b-versatile"
        if (root.provider === "openai") return "gpt-4o-mini"
        if (root.provider === "custom") return "nombre-del-modelo"
        return "openrouter/z-ai/glm-5.2:free"
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
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE && xhr.status === 200) {
                var cfg = JSON.parse(xhr.responseText)
                root.rawConfig = cfg
                if (!cfg.ai) cfg.ai = {}
                if (!cfg.speech) cfg.speech = {}
                if (!cfg.ui) cfg.ui = {}
                root.provider = cfg.ai.provider || "openrouter"
                root.apiKey = cfg.ai.api_key || ""
                root.model = cfg.ai.model || ""
                root.baseUrl = cfg.ai.base_url || ""
                root.toolCallingEnabled = cfg.ai.enable_tool_calling !== false
                root.autoSpeak = cfg.speech.auto_speak === true
                root.chimesEnabled = cfg.speech.chimes_enabled !== false
                root.theme = cfg.ui.theme || "system"
                root.piperModel = cfg.speech.piper_model || "es_ES-sharvard-medium"
                root.piperLengthScale = cfg.speech.piper_length_scale || 1.0
                root.sttLanguage = cfg.speech.stt_language || "auto"
                // Volcar a los campos (rompe nada: asignacion directa)
                baseUrlField.value = root.baseUrl
                apiKeyField.value = root.apiKey
                modelField.value = root.model
                piperField.value = root.piperModel
                speedSlider.value = root.piperLengthScale
                // Listar lo que ofrece la API (usa la key recien cargada)
                fetchModels()
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
        xhr.onreadystatechange = function() {
            if (xhr.readyState === XMLHttpRequest.DONE) {
                if (xhr.status === 200) {
                    try {
                        var resp = JSON.parse(xhr.responseText)
                        var list = resp.models || []
                        root.modelList = list
                        if (list.length > 0) {
                            modelHint.text = qsTr("%n modelo(s) disponibles", "", list.length)
                        } else if (resp.error) {
                            modelHint.text = resp.error
                        } else {
                            modelHint.text = qsTr("La API no devolvio modelos")
                        }
                    } catch (e) {
                        modelHint.text = qsTr("Respuesta invalida del servidor")
                    }
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
        cfg.ai.provider = root.provider
        cfg.ai.api_key = apiKeyField.value
        cfg.ai.model = modelField.value
        cfg.ai.base_url = baseUrlField.value
        cfg.ai.enable_tool_calling = toolCallingEnabled
        cfg.speech.auto_speak = autoSpeak
        cfg.speech.chimes_enabled = chimesEnabled
        cfg.speech.piper_model = piperField.value
        cfg.speech.piper_length_scale = speedSlider.value
        cfg.speech.stt_language = sttLanguage
        cfg.ui.theme = theme
        // Refrescar props locales para que la UI quede consistente
        root.apiKey = apiKeyField.value
        root.model = modelField.value
        root.baseUrl = baseUrlField.value
        root.piperModel = piperField.value
        root.piperLengthScale = speedSlider.value

        var xhr = new XMLHttpRequest()
        xhr.open("POST", backendUrl + "/api/config")
        xhr.setRequestHeader("Content-Type", "application/json")
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

            // === Form ===
            ScrollView {
                id: formScroll
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true

                ColumnLayout {
                    width: formScroll.availableWidth
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

                    SettingsField {
                        id: modelField
                        Layout.fillWidth: true
                        label: qsTr("Modelo")
                        placeholder: modelPlaceholder()
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Theme.spacingXs

                        ComboBox {
                            id: modelPicker
                            Layout.fillWidth: true
                            Layout.preferredHeight: 36
                            model: [qsTr("Elegir de la API…")].concat(root.modelList)
                            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                            onActivated: function(index) {
                                if (index > 0) {
                                    modelField.value = modelPicker.currentText
                                    modelPicker.currentIndex = 0
                                }
                            }
                            background: Rectangle {
                                radius: Theme.radiusMd
                                color: modelPicker.hovered || modelPicker.activeFocus ? Theme.surface : Theme.surfacePearl
                                border.width: 1
                                border.color: modelPicker.activeFocus ? Theme.primary : Theme.hairline
                            }
                            // NOTA: sin contentItem personalizado; el estilo del
                            // sistema (Breeze) espera un TextInput con
                            // positionToRectangle() y rompe con un Text plano.
                            popup: Popup {
                                y: modelPicker.height
                                width: modelPicker.width
                                padding: 4
                                background: Rectangle {
                                    color: Theme.surface
                                    border.color: Theme.hairline
                                    border.width: 1
                                    radius: Theme.radiusMd
                                }
                                contentItem: ListView {
                                    clip: true
                                    implicitHeight: Math.min(contentHeight, 280)
                                    model: modelPicker.popup.visible ? modelPicker.delegateModel : null
                                    currentIndex: modelPicker.highlightedIndex
                                    ScrollIndicator.vertical: ScrollIndicator { }
                                }
                            }
                            delegate: ItemDelegate {
                                width: modelPicker.width
                                text: modelData
                                font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
                                highlighted: modelPicker.highlightedIndex === index
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

                    // Voz
                    Text {
                        text: qsTr("Voz (piper-tts neural)")
                        font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, 0.4)
                        color: Theme.inkMuted
                        Layout.topMargin: Theme.spacingMd
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
                        label: qsTr("Hablar respuestas automaticamente")
                        active: root.autoSpeak
                        onToggled: root.autoSpeak = !root.autoSpeak
                    }

                    SettingsToggle {
                        Layout.fillWidth: true
                        label: qsTr("Reproducir chimes")
                        description: qsTr("Sonidos sutiles al activar/desactivar voz")
                        active: root.chimesEnabled
                        onToggled: root.chimesEnabled = !root.chimesEnabled
                    }
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
