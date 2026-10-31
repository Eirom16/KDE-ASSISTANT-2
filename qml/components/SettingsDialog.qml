import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import qml 1.0
import qml.auth 1.0
import "."

Rectangle {
    id: root
    property bool backgroundEnabled: false
    property bool backgroundPaused: false
    property bool microphoneActive: false
    signal backgroundControlRequested(string action)
    property bool open_: false
    property string backendUrl: AuthToken.backendUrl
    property string authToken: AuthToken.token
    property string activeTab: "general"
    property var rawConfig: ({})
    property bool loaded: false
    property bool busy: false
    property string statusText: ""
    property string statusKind: "info"
    property string provider: "openrouter"
    // Drafts contain only replacements typed in this visit, never saved secrets.
    property var providerKeys: ({})
    property var providerBaseUrls: ({})
    property var providerModels: ({})
    property var endpointApprovals: ({})
    property bool dirty: false
    property bool deleteLegacyKey: false
    property var credentialStates: ({})
    property var deletedKeys: ({})
    property var modelList: []
    property var memoryFacts: []
    property bool factsLoaded: false
    property var audioDevices: []
    property bool characterDetails: false
    property real micLevel: 0
    property int generation: 0
    property var pendingRequest: null
    property var navigation: [
        {key:"general", label:qsTr("General"), help:qsTr("Comportamiento de la ventana y acceso rápido.")},
        {key:"ia", label:qsTr("IA y proveedores"), help:qsTr("Cada proveedor conserva su propia clave, URL y modelo.")},
        {key:"apariencia", label:qsTr("Apariencia"), help:qsTr("Personaliza el tema del asistente.")},
        {key:"voz", label:qsTr("Voz"), help:qsTr("Whisper y Piper locales. Los ajustes de voz se aplican al reiniciar el pipeline.")},
        {key:"herramientas", label:qsTr("Herramientas y permisos"), help:qsTr("Habilitar herramientas conserva las confirmaciones de seguridad F2.")},
        {key:"privacidad", label:qsTr("Privacidad y datos"), help:qsTr("Controla la memoria local y conoce qué se envía al proveedor.")},
        {key:"avanzado", label:qsTr("Avanzado"), help:qsTr("Parámetros de generación y ajustes técnicos de voz.")}
    ]
    property var providerCatalog: [
        {id:"groq", name:"Groq", url:"https://api.groq.com/openai/v1", model:"openai/gpt-oss-120b"},
        {id:"openrouter", name:"OpenRouter", url:"https://openrouter.ai/api/v1", model:"openrouter/z-ai/glm-5.2:free"},
        {id:"openai", name:"OpenAI", url:"https://api.openai.com/v1", model:"gpt-4o-mini"},
        {id:"deepseek", name:"DeepSeek", url:"https://api.deepseek.com/v1", model:"deepseek-chat"},
        {id:"mistral", name:"Mistral", url:"https://api.mistral.ai/v1", model:"mistral-small-latest"},
        {id:"xai", name:"xAI (Grok)", url:"https://api.x.ai/v1", model:""},
        {id:"together", name:"Together AI", url:"https://api.together.xyz/v1", model:""},
        {id:"fireworks", name:"Fireworks AI", url:"https://api.fireworks.ai/inference/v1", model:""},
        {id:"cerebras", name:"Cerebras", url:"https://api.cerebras.ai/v1", model:""},
        {id:"sambanova", name:"SambaNova", url:"https://api.sambanova.ai/v1", model:""},
        {id:"lmstudio", name:"LM Studio", url:"http://127.0.0.1:1234/v1", model:""},
        {id:"ollama", name:"Ollama", url:"http://127.0.0.1:11434/v1", model:""},
        {id:"custom", name:qsTr("Personalizado"), url:"", model:""}
    ]
    onActiveTabChanged: {
        if (formScroll && formScroll.contentItem) formScroll.contentItem.contentY = 0
    }
    signal closed()
    signal saved()
    visible: open_
    color: Theme.overlayBackdrop
    z: 998
    focus: open_

    function cloneMap(value) { return JSON.parse(JSON.stringify(value || ({}))) }
    function providerProfile(id) {
        for (var i = 0; i < providerCatalog.length; ++i)
            if (providerCatalog[i].id === id) return providerCatalog[i]
        return providerCatalog[providerCatalog.length - 1]
    }
    function tabInfo() {
        for (var i = 0; i < navigation.length; ++i)
            if (navigation[i].key === activeTab) return navigation[i]
        return navigation[0]
    }
    function markDirty() {
        if (!loaded || busy) return
        dirty = true
        message(qsTr("Cambios pendientes de guardar."))
    }
    function endpointMismatch() {
        var profile = providerProfile(provider)
        if (provider === "custom" || !baseUrlField.value.trim()) return false
        function origin(url) {
            var match = String(url).trim().match(/^([a-z]+):\/\/([^/]+)/i)
            return match ? (match[1] + "://" + match[2]).toLowerCase().replace(/:443$/, "") : ""
        }
        return origin(baseUrlField.value) !== origin(profile.url)
    }
    function approveEndpoint() {
        var approvals = cloneMap(endpointApprovals)
        var url = baseUrlField.value.trim().replace(/\/+$/, "")
        if (approvals[provider] === url) delete approvals[provider]
        else approvals[provider] = url
        endpointApprovals = approvals
        markDirty()
    }
    function message(text, kind) { statusText = text; statusKind = kind || "info" }
    function show() {
        open_ = true
        loadConfig()
        closeButton.forceActiveFocus()
    }
    function hide() {
        if (busy) return
        generation++
        open_ = false
        providerKeys = ({})
        apiKeyField.value = ""
        rawConfig = ({})
        memoryFacts = []; factsLoaded = false
        loaded = false
        closed()
    }
    function request(method, path, body, callback) {
        if (busy) return
        busy = true
        var epoch = ++generation
        var xhr = new XMLHttpRequest()
        pendingRequest = xhr
        xhr.open(method, backendUrl + path)
        if (authToken) xhr.setRequestHeader("Authorization", "Bearer " + authToken)
        if (body !== null) xhr.setRequestHeader("Content-Type", "application/json")
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== XMLHttpRequest.DONE || epoch !== generation) return
            watchdog.stop()
            pendingRequest = null
            busy = false
            if (xhr.status === 0) { message(qsTr("No se pudo contactar al backend. Los cambios no se confirmaron."), "error"); return }
            var response = ({})
            try { response = JSON.parse(xhr.responseText) } catch (e) {
                message(qsTr("Respuesta inválida del backend. Los cambios no se confirmaron."), "error")
                return
            }
            if (xhr.status === 200) {
                try { callback(response) }
                catch (e) { message(qsTr("El backend devolvió una respuesta de configuración incompleta o inválida."), "error") }
            }
            else if (xhr.status === 401) message(qsTr("La autenticación local falló. Reinicia la aplicación; la clave del proveedor no se ha comprobado."), "error")
            else message(response.error || (xhr.status === 0 ? qsTr("No se pudo contactar al backend.") : qsTr("El backend rechazó la solicitud (HTTP %1).").arg(xhr.status)), "error")
        }
        watchdog.start()
        xhr.send(body === null ? "" : JSON.stringify(body))
    }
    function rememberProviderFields() {
        var keys = cloneMap(providerKeys), urls = cloneMap(providerBaseUrls), models = cloneMap(providerModels)
        keys[provider] = apiKeyField.value
        urls[provider] = baseUrlField.value.trim()
        models[provider] = modelCombo.editText.trim()
        providerKeys = keys; providerBaseUrls = urls; providerModels = models
    }
    function applyProviderFields(id) {
        var profile = providerProfile(id)
        providerChoice.currentIndex = providerCatalog.findIndex(function(p) { return p.id === id })
        apiKeyField.value = providerKeys[id] || ""
        baseUrlField.value = providerBaseUrls[id] !== undefined ? providerBaseUrls[id] : profile.url
        modelList = []
        modelCombo.editText = providerModels[id] !== undefined ? providerModels[id] : profile.model
    }
    function switchProvider(id) {
        if (id === provider || busy) return
        rememberProviderFields()
        provider = id
        applyProviderFields(id)
        markDirty()
    }
    function removeCredential() {
        var keys = cloneMap(providerKeys), deleted = cloneMap(deletedKeys)
        keys[provider] = ""; deleted[provider] = true
        providerKeys = keys; deletedKeys = deleted
        apiKeyField.value = ""
        dirty = true
        message(qsTr("La clave de este proveedor se eliminará al guardar. También se desactivará su fallback de entorno."))
    }
    function credentialText() {
        if (apiKeyField.value.length > 0) return qsTr("Nueva clave pendiente de guardar")
        if (deletedKeys[provider]) return qsTr("Eliminación pendiente de guardar")
        if (credentialStates[provider] === "stored") return qsTr("Clave configurada · guardada en este equipo")
        if (credentialStates[provider] === "environment") return qsTr("Clave configurada · variable de entorno")
        return qsTr("Sin clave configurada")
    }
    function hydrate(cfg) {
        if (!cfg.ai || !cfg.ui || !cfg.speech || !cfg.tools || !cfg.shortcuts || !cfg.memory) {
            message(qsTr("La configuración recibida está incompleta. Vuelve a cargarla."), "error"); return
        }
        rawConfig = cfg
        provider = cfg.ai.provider
        providerKeys = ({})
        providerBaseUrls = cloneMap(cfg.ai.provider_base_urls)
        providerModels = cloneMap(cfg.ai.provider_models)
        endpointApprovals = cloneMap(cfg.ai.provider_endpoint_approvals)
        if (providerBaseUrls[provider] === undefined) providerBaseUrls[provider] = cfg.ai.base_url
        if (providerModels[provider] === undefined) providerModels[provider] = cfg.ai.model
        credentialStates = cfg.ai.credential_states || ({})
        deletedKeys = ({})
        deleteLegacyKey = false
        applyProviderFields(provider)
        closeToTray.active = cfg.ui.close_to_tray
        themeChoice.currentIndex = Math.max(0, ["system", "dark", "light"].indexOf(cfg.ui.theme))
        toolCalling.active = cfg.ai.enable_tool_calling
        autoSpeak.active = cfg.speech.auto_speak
        fastPiper.active = cfg.speech.piper_fast === true
        autoListen.active = cfg.speech.auto_listen
        chimes.active = cfg.speech.chimes_enabled
        piperRuntimeField.value = cfg.speech.piper_runtime || ""
        piperSpeakerChoice.currentIndex = cfg.speech.piper_speaker === 1 ? 1 : 0
        piperField.value = cfg.speech.piper_model
        speedSlider.value = cfg.speech.piper_length_scale
        languageField.value = cfg.speech.stt_language
        micField.value = cfg.speech.mic_device
        sttField.value = cfg.speech.stt_model
        voskLibraryField.value = cfg.speech.vosk_library || ""
        voskModelField.value = cfg.speech.vosk_model || ""
        maxRecordingField.value = String(cfg.speech.max_recording_secs || 30)
        sttTimeoutField.value = String(cfg.speech.stt_timeout_secs || 90)
        wakeField.value = cfg.speech.wake_word
        wakeModelsField.value = cfg.speech.wake_word_model_path || ""
        greetingField.value = cfg.speech.wake_greeting
        thresholdSlider.value = cfg.speech.wake_word_threshold
        temperatureField.value = String(cfg.ai.temperature)
        tokensField.value = String(cfg.ai.max_tokens)
        memoryEnabled.active = cfg.memory.enabled
        autoSummarize.active = cfg.memory.auto_summarize
        toggleField.value = cfg.shortcuts.toggle
        pttField.value = cfg.shortcuts.push_to_talk
        newSessionField.value = cfg.shortcuts.new_session
        menuField.value = cfg.shortcuts.menu
        sttBackendChoice.currentIndex = cfg.speech.stt_engine === "vosk" ? 1 : 0
        sttApiField.value = cfg.speech.stt_api_model || "whisper-large-v3-turbo"
        var character = cfg.character || ({})
        characterEnabled.active = character.enabled === true
        characterMotion.active = character.reduced_motion === true
        characterModeChoice.currentIndex = Math.max(0, ["minimal", "reactive", "companion", "cinematic"].indexOf(character.mode))
        characterShapeChoice.currentIndex = Math.max(0, ["capsule", "round", "pebble", "compact"].indexOf(character.appearance))
        characterSizeField.value = String(character.size || 140)
        characterSleepField.value = String(character.sleep_timeout_secs || 240)
        characterColorField.value = character.accent_color || "#0094bb"
        loaded = true
        dirty = false
    }
    function fetchFacts() {
        request("GET", "/api/memory/facts", null, function(response) {
            if (!Array.isArray(response)) { message(qsTr("Respuesta de memoria inválida."), "error"); return }
            memoryFacts = response; factsLoaded = true; message(qsTr("Memoria local cargada."))
        })
    }
    function addFact() {
        if (!factKeyField.value.trim() || !factValueField.value.trim()) { message(qsTr("Escribe una etiqueta y un dato."), "error"); return }
        request("POST", "/api/memory/facts", {key:factKeyField.value.trim(), value:factValueField.value.trim()}, function(response) {
            if (response.status !== "ok") { message(qsTr("No se confirmó la escritura de memoria."), "error"); return }
            factKeyField.value = ""; factValueField.value = ""; fetchFacts()
        })
    }
    function deleteFact(key) {
        request("DELETE", "/api/memory/facts?key=" + encodeURIComponent(key), null, function(response) {
            if (response.status !== "ok") { message(qsTr("No se confirmó la eliminación."), "error"); return }
            fetchFacts()
        })
    }
    function fetchAudioDevices() {
        request("GET", "/api/audio/devices", null, function(response) {
            audioDevices = response.devices || []
            message(qsTr("%1 micrófonos disponibles.").arg(audioDevices.length))
        })
    }
    function loadConfig(keepStatus) {
        if (busy) return
        loaded = false
        if (!keepStatus) message(qsTr("Cargando configuración…"))
        request("GET", "/api/config", null, function(cfg) {
            hydrate(cfg)
            if (loaded && !keepStatus) message("")
        })
    }
    function saveConfig() {
        if (!loaded || busy) return
        rememberProviderFields()
        var cfg = cloneMap(rawConfig)
        cfg.ai.provider = provider
        cfg.ai.api_key = ""
        cfg.ai.provider_keys = cloneMap(providerKeys)
        cfg.ai.provider_base_urls = cloneMap(providerBaseUrls)
        cfg.ai.provider_models = cloneMap(providerModels)
        cfg.ai.provider_endpoint_approvals = cloneMap(endpointApprovals)
        cfg.ai.base_url = providerBaseUrls[provider]
        cfg.ai.model = providerModels[provider]
        cfg.ai.enable_tool_calling = toolCalling.active
        cfg.ai.temperature = Number(temperatureField.value)
        cfg.ai.max_tokens = Number(tokensField.value)
        if (!baseUrlField.value.trim() || !cfg.ai.model || !temperatureField.inputItem.acceptableInput || !tokensField.inputItem.acceptableInput) {
            message(qsTr("Revisa URL, modelo y parámetros de Avanzado antes de guardar."), "error"); return
        }
        cfg.ui.close_to_tray = closeToTray.active
        cfg.ui.theme = ["system", "dark", "light"][themeChoice.currentIndex]
        cfg.speech.auto_speak = autoSpeak.active
        cfg.speech.piper_fast = fastPiper.active
        cfg.speech.auto_listen = autoListen.active
        cfg.speech.chimes_enabled = chimes.active
        cfg.speech.piper_runtime = piperRuntimeField.value.trim()
        cfg.speech.piper_speaker = piperSpeakerChoice.currentIndex
        cfg.speech.piper_model = piperField.value.trim()
        cfg.speech.piper_length_scale = speedSlider.value
        cfg.speech.stt_language = languageField.value.trim()
        cfg.speech.mic_device = micField.value.trim()
        cfg.speech.stt_model = sttField.value.trim()
        cfg.speech.stt_engine = sttBackendChoice.currentIndex === 1 ? "vosk" : "whisper"
        cfg.speech.vosk_library = voskLibraryField.value.trim()
        cfg.speech.vosk_model = voskModelField.value.trim()
        cfg.speech.max_recording_secs = Number(maxRecordingField.value)
        cfg.speech.stt_timeout_secs = Number(sttTimeoutField.value)
        cfg.speech.wake_word = wakeField.value.trim()
        cfg.speech.wake_word_model_path = wakeModelsField.value.trim()
        cfg.speech.wake_word_threshold = thresholdSlider.value
        cfg.speech.wake_greeting = greetingField.value
        cfg.memory.enabled = memoryEnabled.active
        cfg.memory.auto_summarize = autoSummarize.active
        cfg.shortcuts.toggle = toggleField.value.trim()
        cfg.shortcuts.push_to_talk = pttField.value.trim()
        cfg.shortcuts.new_session = newSessionField.value.trim()
        cfg.shortcuts.menu = menuField.value.trim()
        cfg.speech.stt_backend = "local"
        cfg.speech.stt_api_model = sttApiField.value.trim()
        if (!characterSizeField.inputItem.acceptableInput || !characterSleepField.inputItem.acceptableInput || !characterColorField.inputItem.acceptableInput) {
            message(qsTr("Revisa los ajustes del personaje en Apariencia."), "error"); return
        }
        cfg.character = cfg.character || ({})
        cfg.character.enabled = characterEnabled.active
        cfg.character.reduced_motion = characterMotion.active
        cfg.character.mode = ["minimal", "reactive", "companion", "cinematic"][characterModeChoice.currentIndex]
        cfg.character.appearance = ["capsule", "round", "pebble", "compact"][characterShapeChoice.currentIndex]
        cfg.character.size = Number(characterSizeField.value)
        cfg.character.sleep_timeout_secs = Number(characterSleepField.value)
        cfg.character.accent_color = characterColorField.value
        cfg.delete_legacy_key = deleteLegacyKey
        cfg.delete_provider_keys = Object.keys(deletedKeys).filter(function(id) { return deletedKeys[id] && !(providerKeys[id] || "").length })
        message(qsTr("Guardando…"))
        request("POST", "/api/config", cfg, function(response) {
            if (response.status !== "ok") { message(qsTr("El backend no confirmó el guardado."), "error"); return }
            message(qsTr("Guardado. Los cambios se confirmaron en el servidor."), "success")
            saved()
            loadConfig(true)
        })
    }
    function providerRequest() {
        return {provider:provider, base_url:baseUrlField.value.trim(), api_key:apiKeyField.value,
                model:modelCombo.editText.trim(), approved_endpoint:endpointApprovals[provider] || "", remove_key:deletedKeys[provider] === true && apiKeyField.value.length === 0}
    }
    function fetchModels() {
        message(qsTr("Consultando modelos…"))
        request("POST", "/api/ai-models", providerRequest(), function(response) {
            var keep = modelCombo.editText
            modelList = response.models || []
            modelCombo.editText = keep
            message(modelList.indexOf(keep) < 0 ? qsTr("El modelo elegido no aparece en el catálogo. Selecciona un modelo disponible.") : qsTr("%1 modelos disponibles.").arg(modelList.length))
        })
    }
    function testConnection() {
        message(qsTr("Probando catálogo y streaming…"))
        request("POST", "/api/ai-test", providerRequest(), function(response) {
            if (response.status === "ok") message(response.message, "success")
            else message(qsTr("El backend no confirmó la conexión."), "error")
        })
    }
    function inside(item, ancestor) {
        while (item) { if (item === ancestor) return true; item = item.parent }
        return false
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) { hide(); event.accepted = true }
        if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
            var backwards = event.key === Qt.Key_Backtab || (event.modifiers & Qt.ShiftModifier)
            var current = root.Window.window.activeFocusItem
            if (current && !inside(current.nextItemInFocusChain(!backwards), panel)) {
                if (backwards) saveButton.forceActiveFocus(); else closeButton.forceActiveFocus()
                event.accepted = true
            }
        }
    }
    Timer {
        id: watchdog
        objectName: "settingsWatchdog"
        interval: 30000
        onTriggered: {
            root.generation++
            if (root.pendingRequest) root.pendingRequest.abort()
            root.pendingRequest = null
            root.busy = false
            root.message(qsTr("El backend no respondió a tiempo. Los cambios no se confirmaron; vuelve a cargar para comprobarlos."), "error")
        }
    }
    MouseArea { anchors.fill: parent; onClicked: root.hide() }
    Rectangle {
        id: panel
        objectName: "settingsPanel"
        anchors.centerIn: parent
        width: Math.min(parent.width - Theme.spacingLg, 900)
        height: Math.min(parent.height - Theme.spacingXl, 700)
        radius: Theme.radiusLg
        color: Theme.surface
        border.color: Theme.hairline
        MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacingLg
            spacing: Theme.spacingMd
            RowLayout {
                Layout.fillWidth: true
                Text { text: qsTr("Configuración"); color: Theme.ink; font: Theme.font(Theme.fontSizeTagline, Theme.weightBold); Layout.fillWidth: true }
                SettingsButton { id: closeButton; KeyNavigation.backtab: saveButton; KeyNavigation.priority: KeyNavigation.BeforeItem; objectName: "settingsClose"; text: qsTr("Cerrar"); enabled: !root.busy; onClicked: root.hide() }
            }
            Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.hairline }
            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: Theme.spacingLg
                ScrollView {
                    visible: panel.width >= 650
                    Layout.preferredWidth: 184
                    Layout.fillHeight: true
                    contentWidth: availableWidth
                    clip: true
                    ColumnLayout {
                        width: parent.width
                        spacing: Theme.spacingXs
                        Repeater {
                            model: root.navigation
                            SettingsButton {
                                required property var modelData
                                objectName: "settingsNav_" + modelData.key
                                Layout.fillWidth: true
                                implicitHeight: Theme.buttonIconSize + Theme.spacingXs
                                text: modelData.label
                                primary: root.activeTab === modelData.key
                                onClicked: { root.activeTab = modelData.key; formScroll.contentItem.contentY = 0 }
                                contentItem: Text {
                                    text: parent.text; wrapMode: Text.Wrap; verticalAlignment: Text.AlignVCenter
                                    font: Theme.font(Theme.fontSizeBodySmall, Theme.weightBold)
                                    color: parent.primary ? Theme.inkOnPrimary : Theme.ink
                                }
                            }
                        }
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    SettingsChoice {
                        visible: panel.width < 650
                        objectName: "settingsNavigation"
                        model: root.navigation
                        textRole: "label"
                        currentIndex: root.navigation.findIndex(function(p) { return p.key === root.activeTab })
                        onActivated: { root.activeTab = root.navigation[currentIndex].key; formScroll.contentItem.contentY = 0 }
                    }
                    Text { text: root.tabInfo().label; font: Theme.font(Theme.fontSizeBodyStrong, Theme.weightBold); color: Theme.ink; Layout.fillWidth: true; wrapMode: Text.Wrap }
                    Text { text: root.tabInfo().help; font: Theme.font(Theme.fontSizeBodySmall); color: Theme.inkMuted80; Layout.fillWidth: true; wrapMode: Text.Wrap }
                    ScrollView {
                        id: formScroll
                        objectName: "settingsScroll"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: availableWidth
                        clip: true
                        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                        ColumnLayout {
                            id: formContent
                            width: formScroll.availableWidth - Theme.spacingSm
                            spacing: Theme.spacingLg
                            enabled: root.loaded && !root.busy
                            ColumnLayout {
                                visible: root.activeTab === "general"
                                Layout.fillWidth: true
                                SettingsToggle { id: closeToTray; label: qsTr("Cerrar a la bandeja"); description: qsTr("La aplicación sigue disponible desde el icono de Plasma."); onToggled: { active = !active; root.markDirty() } }
                                Help { text: qsTr("Ctrl+, abre Configuración. El atajo global se gestiona mediante el portal de Plasma; su configuración actual se conserva.") }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "ia"
                                Layout.fillWidth: true
                                spacing: Theme.spacingMd
                                LabelText { text: qsTr("Proveedor") }
                                SettingsChoice { id: providerChoice; objectName: "settingsProvider"; model: root.providerCatalog; textRole: "name"; onActivated: root.switchProvider(root.providerCatalog[currentIndex].id) }
                                DraftField { id: baseUrlField; objectName: "settingsBaseUrl"; label: qsTr("URL base"); placeholder: "https://api.groq.com/openai/v1" }
                                ColumnLayout {
                                    visible: root.endpointMismatch()
                                    Layout.fillWidth: true
                                    spacing: Theme.spacingXs
                                    Help { text: qsTr("Destino distinto del proveedor. El destino oficial es %1. La clave y los mensajes se enviarán a la URL indicada arriba.").arg(root.providerProfile(root.provider).url) }
                                    SettingsToggle {
                                        objectName: "settingsApproveEndpoint"
                                        label: qsTr("Confirmar esta URL alternativa")
                                        description: qsTr("Decisión explícita para este proveedor y esta URL. Cambiar la URL requiere confirmarla de nuevo.")
                                        active: root.endpointApprovals[root.provider] === baseUrlField.value.trim().replace(/\/+$/, "")
                                        onToggled: root.approveEndpoint()
                                    }
                                }
                                DraftField { id: apiKeyField; objectName: "settingsApiKey"; label: qsTr("Reemplazar clave API"); isPassword: true; placeholder: qsTr("Deja vacío para conservar la clave configurada") }
                                Help { objectName: "settingsCredentialState"; text: root.credentialText() }
                                SettingsButton { text: root.deletedKeys[root.provider] ? qsTr("Deshacer eliminación") : qsTr("Eliminar clave"); destructive: true; onClicked: {
                                    if (root.deletedKeys[root.provider]) { var next = root.cloneMap(root.deletedKeys); delete next[root.provider]; root.deletedKeys = next; root.markDirty() }
                                    else root.removeCredential()
                                } }
                                LabelText { text: qsTr("Modelo de chat") }
                                SettingsChoice { id: modelCombo; onEditTextChanged: root.markDirty(); objectName: "settingsModel"; editable: true; model: root.modelList }
                                GridLayout {
                                    Layout.fillWidth: true
                                    columns: formContent.width >= 440 ? 2 : 1
                                    columnSpacing: Theme.spacingSm
                                    rowSpacing: Theme.spacingSm
                                    SettingsButton { objectName: "settingsModels"; text: qsTr("Consultar modelos"); Layout.fillWidth: true; onClicked: root.fetchModels() }
                                    SettingsButton { text: qsTr("Probar conexión"); objectName: "settingsTestConnection"; Layout.fillWidth: true; onClicked: root.testConnection() }
                                }
                                Help { text: qsTr("Probar conexión envía una petición breve de chat al proveedor y puede consumir cuota o saldo. No guarda cambios ni prueba herramientas. Groq es distinto de Grok/xAI.") }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "apariencia"
                                Layout.fillWidth: true
                                LabelText { text: qsTr("Tema") }
                                SettingsChoice { id: themeChoice; onActivated: root.markDirty(); objectName: "settingsTheme"; model: [qsTr("Sistema"), qsTr("Oscuro"), qsTr("Claro")] }
                                Help { text: qsTr("El tema se aplica al guardar. El seguimiento automático de cambios del sistema depende de la integración de escritorio existente.") }
                                SettingsButton { text: root.characterDetails ? qsTr("Ocultar ajustes del personaje") : qsTr("Ajustes del personaje"); onClicked: root.characterDetails = !root.characterDetails }
                                ColumnLayout {
                                    visible: root.characterDetails
                                    Layout.fillWidth: true
                                    spacing: Theme.spacingMd
                                    SettingsToggle { id: characterEnabled; label: qsTr("Mostrar personaje"); description: qsTr("Integración heredada opcional; puede requerir reiniciar la aplicación."); onToggled: { active = !active; root.markDirty() } }
                                    SettingsToggle { id: characterMotion; label: qsTr("Reducir movimiento"); onToggled: { active = !active; root.markDirty() } }
                                    LabelText { text: qsTr("Presencia") }
                                    SettingsChoice { id: characterModeChoice; onActivated: root.markDirty(); objectName: "settingsCharacterMode"; model: [qsTr("Mínima"), qsTr("Reactiva"), qsTr("Compañero"), qsTr("Cinemática")] }
                                    LabelText { text: qsTr("Forma") }
                                    SettingsChoice { id: characterShapeChoice; onActivated: root.markDirty(); objectName: "settingsCharacterShape"; model: [qsTr("Cápsula"), qsTr("Redonda"), qsTr("Piedra"), qsTr("Compacta")] }
                                    DraftField { id: characterSizeField; label: qsTr("Tamaño (px)"); validator: IntValidator { bottom: 64; top: 320 } }
                                    DraftField { id: characterSleepField; label: qsTr("Inactividad antes de dormir (s)"); validator: IntValidator { bottom: 15; top: 3600 } }
                                    DraftField { id: characterColorField; label: qsTr("Color del personaje (#RRGGBB)"); validator: RegularExpressionValidator { regularExpression: /^#[0-9a-fA-F]{6}$/ } }
                                }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "voz"
                                Layout.fillWidth: true
                                spacing: Theme.spacingMd
                                Help { text: (root.rawConfig.capabilities && root.rawConfig.capabilities.detail) || qsTr("Voz opcional diferida") }
                                SettingsToggle { id: autoSpeak; label: qsTr("Leer respuestas en voz alta"); onToggled: { active = !active; root.markDirty() } }
                                SettingsToggle {
                                    id: fastPiper; objectName: "settingsPiperFast"
                                    label: qsTr("Voz rápida (experimental)")
                                    description: qsTr("Mantiene temporalmente preparada la voz para responder más rápido. Puede usar aproximadamente 310 MB adicionales de memoria. No activa la lectura automática.")
                                    enabled: active || !!(root.rawConfig.capabilities && root.rawConfig.capabilities.piper_fast_supported && root.rawConfig.capabilities.piper_binary_present)
                                    onToggled: { active = !active; root.markDirty() }
                                }
                                Help { visible: !(root.rawConfig.capabilities && root.rawConfig.capabilities.piper_fast_supported && root.rawConfig.capabilities.piper_binary_present); text: qsTr("Voz rápida requiere el entorno y la voz Piper locales compatibles. Configúralos y guarda antes de activarla.") }
                                SettingsToggle { id: autoListen; enabled: false; label: qsTr("Escucha continua pendiente de validación F4"); onToggled: { active = !active; root.markDirty() } }
                                SettingsToggle { id: chimes; enabled: false; label: qsTr("Sonidos de activación (no disponibles en dictado)"); onToggled: { active = !active; root.markDirty() } }
                                DraftField { id: piperRuntimeField; objectName: "settingsPiperRuntime"; label: qsTr("Entorno Piper local"); placeholder: qsTr("Ruta absoluta del entorno privado evaluado") }
                                DraftField { id: piperField; objectName: "settingsPiperModel"; label: qsTr("Voz Piper local"); placeholder: "es_ES-sharvard-medium" }
                                LabelText { text: qsTr("Hablante de la voz española") }
                                SettingsChoice { id: piperSpeakerChoice; objectName: "settingsPiperSpeaker"; model: [qsTr("Hablante 0"), qsTr("Hablante 1")]; onActivated: root.markDirty() }
                                Help { text: qsTr("Piper 1.8.0 y voz española verificados. No se descargan recursos automáticamente. Sin entorno y voz instalados, la lectura no estará disponible.") }
                                LabelText { text: qsTr("Duración de voz · %1×").arg(speedSlider.value.toFixed(2)) }
                                Slider { id: speedSlider; onMoved: root.markDirty(); Layout.fillWidth: true; from: 0.5; to: 2; stepSize: 0.05; Accessible.name: qsTr("Duración de voz") }
                                DraftField { id: languageField; label: qsTr("Idioma de transcripción"); placeholder: "auto / es / en" }
                                DraftField { id: micField; label: qsTr("Micrófono preferido"); placeholder: qsTr("Vacío: predeterminado de PipeWire; vuelve a consultar nombres antiguos") }
                                SettingsButton { text: qsTr("Consultar micrófonos"); onClicked: root.fetchAudioDevices() }
                                SettingsChoice { visible: root.audioDevices.length > 0; objectName: "settingsMicrophone"; model: root.audioDevices; onActivated: micField.value = root.audioDevices[currentIndex] }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "herramientas"
                                Layout.fillWidth: true
                                spacing: Theme.spacingMd
                                SettingsToggle { id: toolCalling; label: qsTr("Permitir llamadas a herramientas"); description: qsTr("El modelo también debe admitir tool calling. Las operaciones sensibles siguen requiriendo consentimiento individual."); onToggled: { active = !active; root.markDirty() } }
                                Help { text: qsTr("Las herramientas restringidas permanecen deshabilitadas por F2. Este panel conserva la selección individual y las rutas autorizadas existentes; no concede permisos nuevos.") }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "privacidad"
                                Layout.fillWidth: true
                                spacing: Theme.spacingMd
                                Help { text: qsTr("Las claves se almacenan en archivos privados (0600). Nunca se devuelven a este formulario. Al conversar se envían mensajes y contexto al proveedor seleccionado; los historiales y la memoria permanecen en SQLite local.") }
                                SettingsToggle { id: memoryEnabled; label: qsTr("Usar memoria local"); description: qsTr("Los datos usados como contexto pueden incluirse en el prompt enviado al proveedor."); onToggled: { active = !active; root.markDirty() } }
                                SettingsToggle { id: autoSummarize; label: qsTr("Resumir conversaciones largas"); description: qsTr("Generar resúmenes utiliza el proveedor de IA y puede consumir cuota."); onToggled: { active = !active; root.markDirty() } }
                                Help { text: qsTr("Eliminar una clave desactiva su uso actual. config.json.bak puede conservar claves anteriores; no es un borrado forense.") }
                                ColumnLayout {
                                    visible: !!root.rawConfig.ai && root.rawConfig.ai.has_unassigned_legacy_key === true
                                    Layout.fillWidth: true
                                    Help { text: qsTr("Se conservó una clave antigua sin proveedor inequívoco. Está inactiva y no se envía. Puedes reemplazar la clave de cada proveedor en IA y proveedores.") }
                                    SettingsButton {
                                        text: root.deleteLegacyKey ? qsTr("Deshacer eliminación de clave antigua") : qsTr("Eliminar clave antigua sin asignar")
                                        destructive: true
                                        onClicked: { root.deleteLegacyKey = !root.deleteLegacyKey; root.markDirty() }
                                    }
                                }
                                SettingsButton { text: qsTr("Consultar memoria guardada"); onClicked: root.fetchFacts() }
                                ColumnLayout {
                                    visible: root.factsLoaded
                                    Layout.fillWidth: true
                                    spacing: Theme.spacingMd
                                    Help { visible: root.memoryFacts.length === 0; text: qsTr("No hay datos de memoria guardados.") }
                                    Repeater {
                                        model: root.memoryFacts
                                        ColumnLayout {
                                            required property var modelData
                                            Layout.fillWidth: true
                                            LabelText { text: modelData.key; textFormat: Text.PlainText }
                                            Help { text: modelData.value }
                                            SettingsButton { text: qsTr("Eliminar dato ahora"); destructive: true; onClicked: root.deleteFact(modelData.key) }
                                        }
                                    }
                                    SettingsField { id: factKeyField; label: qsTr("Etiqueta de memoria") }
                                    SettingsField { id: factValueField; label: qsTr("Dato que deseas recordar") }
                                    SettingsButton { text: qsTr("Guardar dato ahora"); onClicked: root.addFact() }
                                    Help { text: qsTr("Las acciones de memoria se aplican inmediatamente, por separado del botón Guardar cambios.") }
                                }
                                Help { text: qsTr("El borrado de conversaciones y la revisión de resultados inciertos se realizan desde el historial. Una configuración corrupta bloquea el arranque y se conserva para recuperación manual con config.json.bak.") }
                            }
                            ColumnLayout {
                                visible: root.activeTab === "avanzado"
                                Layout.fillWidth: true
                                spacing: Theme.spacingMd
                                DraftField { id: temperatureField; label: qsTr("Temperatura (0–2)"); validator: DoubleValidator { bottom: 0; top: 2; locale: "C"; notation: DoubleValidator.StandardNotation } }
                                DraftField { id: tokensField; label: qsTr("Máximo de tokens de salida"); validator: IntValidator { bottom: 1; top: 131072 } }
                                Help { text: qsTr("El modelo puede imponer límites adicionales. Groq usa max_completion_tokens. La prueba de conexión limita su salida a 64 tokens.") }
                                DraftField { id: maxRecordingField; label: qsTr("Duración máxima de dictado (1–120 s)"); validator: IntValidator { bottom: 1; top: 120 } }
                                DraftField { id: sttTimeoutField; label: qsTr("Tiempo máximo de STT (1–300 s)"); validator: IntValidator { bottom: 1; top: 300 } }
                                Help { text: qsTr("Dictado local: pulsa para iniciar y otra vez para transcribir. El texto se añade al borrador sin enviarlo. Hey Jarvis se activa una vez desde el robot o la bandeja. Mantiene el micrófono abierto en segundo plano al ocultar/minimizar. Di Hey Jarvis seguido de la orden; el texto queda en el borrador. Cancelar permite rearmar. Pausar y Desactivar cierran el micrófono. No se descargan modelos ni voces.") }
                                DraftField { id: sttField; visible: sttBackendChoice.currentIndex === 0; label: qsTr("Modelo Whisper local"); placeholder: "tiny / base / ruta absoluta GGML" }
                                DraftField { id: wakeField; enabled: false; label: qsTr("Palabra de activación (Hey Jarvis)") }
                                Help { text: root.microphoneActive ? qsTr("Micrófono abierto en segundo plano. Pausar o Desactivar cierran la captura.") : root.backgroundEnabled ? qsTr("Hey Jarvis pausado. Micrófono cerrado.") : qsTr("Hey Jarvis está desactivado. No escucha al arrancar.") }
                                SettingsButton { objectName: "settingsBackgroundWake"; text: !root.backgroundEnabled ? qsTr("Activar escucha en segundo plano") : root.backgroundPaused ? qsTr("Reanudar Hey Jarvis") : qsTr("Pausar Hey Jarvis"); onClicked: root.backgroundControlRequested(!root.backgroundEnabled ? "enable" : root.backgroundPaused ? "resume" : "pause") }
                                SettingsButton { visible: root.backgroundEnabled; text: qsTr("Desactivar Hey Jarvis"); onClicked: root.backgroundControlRequested("disable") }
                                DraftField { id: wakeModelsField; objectName: "settingsWakeModels"; label: qsTr("Directorio local de modelos Hey Jarvis"); placeholder: "/ruta/wakeword (vacío: directorio de datos)" }
                                Help { text: qsTr("Modelo de frase inglesa Hey Jarvis: di la frase con pronunciación inglesa y espera a Grabando. openWakeWord: código Apache-2.0; modelos CC BY-NC-SA 4.0 (uso no comercial). OFF al arrancar; no se descargan recursos.") }
                                DraftField { id: greetingField; enabled: false; label: qsTr("Saludo de activación (pendiente)") }
                                LabelText { text: qsTr("Umbral de activación · %1").arg(thresholdSlider.value.toFixed(2)) }
                                Slider { id: thresholdSlider; objectName: "settingsWakeThreshold"; enabled: true; onMoved: root.markDirty(); Layout.fillWidth: true; from: 0.1; to: 1; stepSize: 0.05; Accessible.name: qsTr("Umbral de confianza de Hey Jarvis") }
                                LabelText { text: qsTr("Motor de transcripción") }
                                SettingsChoice { id: sttBackendChoice; onActivated: root.markDirty(); objectName: "settingsSttBackend"; model: [qsTr("Whisper local"), qsTr("Vosk español · experimental")] }
                                DraftField { id: voskLibraryField; objectName: "settingsVoskLibrary"; visible: sttBackendChoice.currentIndex === 1; label: qsTr("Biblioteca Vosk local"); placeholder: "/ruta/libvosk.so" }
                                DraftField { id: voskModelField; objectName: "settingsVoskModel"; visible: sttBackendChoice.currentIndex === 1; label: qsTr("Directorio del modelo Vosk español"); placeholder: "/ruta/vosk-model-small-es-0.42" }
                                Help { visible: sttBackendChoice.currentIndex === 1; text: qsTr("Evaluación inicial: Vosk 0.3.45 y small español 0.42. Requiere idioma es; puede confundir nombres de aplicaciones. Revisa el dictado antes de enviarlo. Sin instalación ni descarga automática.") }
                                Help { text: qsTr("Audio remoto deshabilitado en F4. Los valores API heredados se conservan sin enviar grabaciones. Una clave de chat no autoriza subir audio.") }
                                DraftField { id: sttApiField; enabled: false; label: qsTr("Modelo de transcripción API") }
                                LabelText { text: qsTr("Atajos guardados") }
                                Help { text: qsTr("El portal sólo registra Mostrar/Ocultar cuando se inicia con --enable-shortcuts. Editar no concede acceso al teclado; requiere reiniciar y autorización del sistema. Los demás valores heredados se conservan sin prometer registro global.") }
                                DraftField { id: toggleField; label: qsTr("Mostrar / ocultar"); placeholder: "Super+Shift+A" }
                                DraftField { id: pttField; label: qsTr("Pulsar para hablar (heredado)") }
                                DraftField { id: newSessionField; label: qsTr("Nueva sesión") }
                                DraftField { id: menuField; label: qsTr("Menú de bandeja") }
                            }
                        }
                    }
                }
            }
            Text {
                visible: root.dirty
                Layout.fillWidth: true
                text: qsTr("Cambios pendientes de guardar")
                color: Theme.inkMuted80
                font: Theme.font(Theme.fontSizeBodySmall)
            }
            Rectangle {
                visible: root.statusText.length > 0
                Layout.fillWidth: true
                implicitHeight: statusLabel.implicitHeight + Theme.spacingSm * 2
                radius: Theme.radiusMd
                color: root.statusKind === "error" ? Theme.errorTint : root.statusKind === "success" ? Theme.successTint : Theme.surfacePearl
                Text {
                    id: statusLabel
                    objectName: "settingsStatus"
                    anchors.fill: parent; anchors.margins: Theme.spacingSm
                    text: root.statusText
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font: Theme.font(Theme.fontSizeBodySmall)
                    color: Theme.ink
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: text
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: Theme.spacingSm
                SettingsButton { objectName: "settingsReload"; text: panel.width < 500 ? qsTr("Recargar") : qsTr("Volver a cargar"); enabled: !root.busy; onClicked: root.loadConfig() }
                Item { Layout.fillWidth: true }
                SettingsButton { id: saveButton; KeyNavigation.tab: closeButton; KeyNavigation.priority: KeyNavigation.BeforeItem; objectName: "settingsSave"; text: root.busy ? qsTr("Espera…") : panel.width < 500 ? qsTr("Guardar") : qsTr("Guardar cambios"); primary: true; enabled: root.loaded && !root.busy; onClicked: root.saveConfig() }
            }
        }
    }
    Connections {
        target: root.Window.window
        function onActiveFocusItemChanged() {
            var item = root.Window.window.activeFocusItem
            if (!item || !root.inside(item, formContent)) return
            var point = item.mapToItem(formContent, 0, 0)
            var flick = formScroll.contentItem
            if (point.y < flick.contentY) flick.contentY = Math.max(0, point.y - Theme.spacingXs)
            else if (point.y + item.height > flick.contentY + flick.height)
                flick.contentY = Math.min(Math.max(0, flick.contentHeight - flick.height), point.y + item.height - flick.height + Theme.spacingXs)
        }
    }
    component DraftField: SettingsField {
        onValueChanged: root.markDirty()
    }
    component Help: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        color: Theme.inkMuted80
        font: Theme.font(Theme.fontSizeBodySmall)
    }
    component LabelText: Text {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        color: Theme.ink
        font: Theme.font(Theme.fontSizeBodySmall, Theme.weightBold)
    }
}
