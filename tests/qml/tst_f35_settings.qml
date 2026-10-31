import QtQuick
import QtQuick.Window
import QtQuick.Controls
import QtTest
import qml 1.0
import qml.components 1.0 as Components

Item {
    id: scene
    width: 1366
    height: 768
    TextField { id: behindSettings; text: "background"; opacity: 0 }
    Components.SettingsDialog { id: settings; anchors.fill: parent; backendUrl: "F35_BACKEND"; authToken: "synthetic-ui-token" }
    SignalSpy { id: saved; target: settings; signalName: "saved" }
    TestCase {
        name: "F35Settings"
        when: windowShown
        function field(name) { return findChild(settings, name) }
        function init() {
            if (settings.open_) settings.hide()
            saved.clear()
            settings.activeTab = "ia"
            settings.show()
            tryCompare(settings, "busy", false, 5000)
            verify(settings.loaded)
        }
        function test_credential_and_provider_drafts() {
            compare(field("settingsApiKey").value, "")
            verify(field("settingsCredentialState").text.indexOf("Clave configurada") >= 0)
            field("settingsApiKey").value = "synthetic-replacement"
            field("settingsBaseUrl").value = "http://127.0.0.1:1234/v1"
            field("settingsModel").editText = "synthetic-groq"
            settings.switchProvider("openrouter")
            compare(field("settingsApiKey").value, "")
            settings.switchProvider("groq")
            compare(field("settingsApiKey").value, "synthetic-replacement")
            compare(field("settingsModel").editText, "synthetic-groq")
            settings.removeCredential()
            verify(settings.deletedKeys.groq)
            compare(field("settingsApiKey").value, "")
        }
        function test_hey_jarvis_settings_roundtrip_never_arms_capture() {
            settings.activeTab = "avanzado"
            field("settingsWakeModels").value = "/synthetic/local/wakeword"
            field("settingsWakeThreshold").value = 0.8
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.rawConfig.speech.wake_word_model_path, "/synthetic/local/wakeword")
            compare(settings.rawConfig.speech.wake_word_threshold, 0.8)
            verify(settings.rawConfig.speech.wake_word_enabled !== true)
            settings.loadConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(field("settingsWakeModels").value, "/synthetic/local/wakeword")
            compare(field("settingsWakeThreshold").value, 0.8)
        }
        function test_failed_save_stays_open_then_success() {
            field("settingsModel").editText = "stale-revision"
            settings.saveConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(saved.count, 0)
            verify(settings.open_)
            compare(settings.statusKind, "error")
            compare(field("settingsModel").editText, "stale-revision")
            field("settingsModel").editText = "fail-write"
            settings.saveConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(saved.count, 0)
            verify(settings.open_)
            compare(settings.statusKind, "error")
            field("settingsModel").editText = "fake-success"
            settings.saveConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(saved.count, 0)
            compare(settings.statusKind, "error")
            verify(settings.open_)
            field("settingsModel").editText = "fixture-chat"
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.statusKind, "success")
            verify(settings.open_)
        }
        function test_models_and_connection_buttons_do_not_save() {
            var modelsButton = field("settingsModels")
            modelsButton.forceActiveFocus()
            wait(40)
            mouseClick(modelsButton)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.modelList.length, 2)
            compare(field("settingsModel").editText, "fixture-chat")
            compare(saved.count, 0)
            var testButton = field("settingsTestConnection")
            testButton.forceActiveFocus()
            wait(40)
            mouseClick(testButton)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.statusKind, "success")
            compare(saved.count, 0)
            compare(settings.rawConfig.ai.model, "fixture-chat")
            grabImage(scene).save("F35_IMAGES/desktop-dark-ia-actions.png")
        }
        function test_f35_audit_pending_after_save() {
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            var input = field("settingsApiKey").inputItem
            input.forceActiveFocus()
            keyClick(Qt.Key_X)
            compare(settings.statusKind, "info", "editing after save must clear the success state")
        }
        function test_f35_audit_destination_requires_explicit_decision() {
            var approvals = settings.cloneMap(settings.endpointApprovals)
            delete approvals.groq
            settings.endpointApprovals = approvals
            settings.testConnection()
            tryCompare(settings, "busy", false, 5000)
            compare(settings.statusKind, "error")
            compare(saved.count, 0)
            var approval = field("settingsApproveEndpointControl")
            approval.forceActiveFocus()
            keyClick(Qt.Key_Space)
            compare(settings.endpointApprovals.groq, field("settingsBaseUrl").value)
            settings.testConnection()
            tryCompare(settings, "busy", false, 5000)
            compare(settings.statusKind, "success")
            field("settingsBaseUrl").value = "http://127.0.0.1:1235/v1"
            verify(!field("settingsApproveEndpoint").active)
        }
        function test_f35_audit_timeout_invalidates_late_ack() {
            var watchdog = field("settingsWatchdog")
            watchdog.interval = 50
            field("settingsModel").editText = "slow-save"
            settings.saveConfig()
            tryCompare(settings, "busy", false, 2000)
            watchdog.interval = 30000
            compare(saved.count, 0)
            compare(settings.statusKind, "error")
            field("settingsModel").editText = "fixture-chat"
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            wait(400)
            compare(saved.count, 1)
            compare(settings.statusKind, "success")
        }
        function test_f35_audit_compact_error_footer() {
            scene.width = 360; scene.height = 500
            field("settingsModel").editText = "fail-write"
            settings.saveConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(settings.statusKind, "error")
            verify(settings.dirty)
            var panel = field("settingsPanel"), save = field("settingsSave")
            var position = save.mapToItem(panel, 0, 0)
            verify(position.y + save.height <= panel.height, "compact error hides the footer")
            verify(field("settingsScroll").height > 0, "compact error removes the viewport")
            scene.width = 1366; scene.height = 768
        }
        function test_read_failure_disables_save() {
            settings.backendUrl = "http://127.0.0.1:1"
            settings.loadConfig()
            tryCompare(settings, "busy", false, 5000)
            verify(!settings.loaded)
            verify(!field("settingsSave").enabled)
            compare(settings.statusKind, "error")
            verify(settings.open_)
            settings.backendUrl = "F35_BACKEND"
        }
        function test_f4r2_local_engine_is_explicit_and_survives_save() {
            settings.activeTab = "avanzado"
            var choice = field("settingsSttBackend")
            compare(choice.currentIndex, 0)
            choice.currentIndex = 1
            field("settingsVoskLibrary").value = "/nonexistent/libvosk.so"
            field("settingsVoskModel").value = "/nonexistent/vosk-model-small-es-0.42"
            verify(field("settingsVoskModel").visible)
            wait(40) // Let the layout place newly visible fields before focusing.
            for (var dark of [true, false]) {
                Theme.isDark = dark
                field("settingsVoskLibrary").inputItem.forceActiveFocus()
                field("settingsVoskModel").inputItem.forceActiveFocus()
                wait(40)
                var modelPosition = field("settingsVoskModel").inputItem.mapToItem(field("settingsScroll"), 0, 0)
                verify(modelPosition.y >= 0)
                verify(modelPosition.y + field("settingsVoskModel").inputItem.height <= field("settingsScroll").height,
                    "Model input outside viewport: y="+modelPosition.y+", viewport="+field("settingsScroll").height)
                grabImage(scene).save("F35_IMAGES/r2-vosk-"+(dark ? "dark" : "light")+".png")
            }
            Theme.isDark = true
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.rawConfig.speech.stt_engine, "vosk")
            compare(settings.rawConfig.speech.stt_backend, "local")
            compare(settings.rawConfig.speech.vosk_library, "/nonexistent/libvosk.so")
            settings.loadConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(choice.currentIndex, 1)
            compare(field("settingsVoskModel").value, "/nonexistent/vosk-model-small-es-0.42")
        }
        function test_f4r3_piper_runtime_and_speaker_survive_save() {
            settings.activeTab = "voz"
            field("settingsPiperRuntime").value = "/nonexistent/private-piper-venv"
            field("settingsPiperModel").value = "/nonexistent/es_ES-sharvard-medium.onnx"
            field("settingsPiperSpeaker").currentIndex = 1
            settings.saveConfig()
            tryCompare(saved, "count", 1, 5000)
            tryCompare(settings, "busy", false, 5000)
            compare(settings.rawConfig.speech.piper_runtime, "/nonexistent/private-piper-venv")
            compare(settings.rawConfig.speech.piper_speaker, 1)
            settings.loadConfig()
            tryCompare(settings, "busy", false, 5000)
            compare(field("settingsPiperRuntime").value, "/nonexistent/private-piper-venv")
            compare(field("settingsPiperSpeaker").currentIndex, 1)
        }
        function test_f4r5_fast_voice_off_migration_roundtrip_and_keyboard() {
            settings.activeTab = "voz"
            var fast = field("settingsPiperFast")
            var originalAutoRead = settings.rawConfig.speech.auto_speak
            verify(!!fast)
            verify(!fast.active || settings.rawConfig.speech.piper_fast === true)
            verify(fast.description.indexOf("memoria") >= 0)
            // Availability is supplied by Rust; synthetic capabilities never load Piper.
            var raw = JSON.parse(JSON.stringify(settings.rawConfig))
            raw.capabilities = {piper_fast_supported:true,piper_binary_present:true}
            settings.rawConfig = raw
            verify(fast.enabled)
            var control = field("settingsPiperFastControl")
            wait(40)
            control.forceActiveFocus()
            for (var dark of [true,false]) {
                Theme.isDark = dark
                fast.active = false
                keyClick(Qt.Key_Space)
                compare(fast.active,true)
                keyClick(Qt.Key_Space)
                compare(fast.active,false)
            }
            Theme.isDark=true
            fast.active=true
            settings.saveConfig()
            tryCompare(saved,"count",1,5000)
            tryCompare(settings,"busy",false,5000)
            compare(settings.rawConfig.speech.piper_fast,true)
            compare(settings.rawConfig.speech.auto_speak,originalAutoRead)
            settings.loadConfig()
            tryCompare(settings,"busy",false,5000)
            compare(fast.active,true)
            fast.active=false
            settings.saveConfig()
            tryCompare(saved,"count",2,5000)
        }
        function test_keyboard_and_scroll() {
            settings.activeTab = "ia"
            var api = field("settingsApiKey").inputItem
            api.forceActiveFocus()
            keyClick(Qt.Key_Tab)
            verify(!api.activeFocus)
            verify(settings.inside(scene.Window.window.activeFocusItem, field("settingsPanel")))
            var fields = []
            function walk(item) {
                if (item.inputItem && item.visible) fields.push(item.inputItem)
                for (var i=0;i<item.children.length;i++) walk(item.children[i])
            }
            for (var section of ["voz", "avanzado"]) {
                settings.activeTab = section
                wait(30)
                fields = []
                walk(field("settingsPanel"))
                verify(fields.length >= 3)
                fields[fields.length-1].forceActiveFocus()
                wait(30)
                verify(field("settingsScroll").contentItem.contentY > 0, section + " must scroll to its last field")
            }
            field("settingsSave").forceActiveFocus()
            keyClick(Qt.Key_Tab)
            verify(settings.inside(scene.Window.window.activeFocusItem, field("settingsPanel")))
            for (var i=0;i<80;i++) {
                keyClick(Qt.Key_Tab)
                verify(settings.inside(scene.Window.window.activeFocusItem, field("settingsPanel")), "Tab escaped settings")
            }
            field("settingsClose").forceActiveFocus()
            keyClick(Qt.Key_Backtab)
            verify(settings.inside(scene.Window.window.activeFocusItem, field("settingsPanel")))
            keyClick(Qt.Key_Escape)
            verify(!settings.open_)
        }
        function test_layout_data() {
            return [{tag:"desktop-dark",w:1366,h:768,dark:true}, {tag:"desktop-light",w:1366,h:768,dark:false}, {tag:"compact",w:575,h:680,dark:true}, {tag:"minimum",w:360,h:500,dark:true}]
        }
        function test_layout(data) {
            scene.width=data.w; scene.height=data.h
            Theme.isDark=data.dark
            for (var i=0;i<settings.navigation.length;i++) {
                settings.activeTab=settings.navigation[i].key
                wait(40)
                var panel=field("settingsPanel"), save=field("settingsSave"), scroll=field("settingsScroll")
                verify(panel.x >= 0 && panel.y >= 0)
                verify(panel.x+panel.width <= scene.width && panel.y+panel.height <= scene.height)
                var pos=save.mapToItem(panel,0,0)
                verify(pos.x >= 0 && pos.x+save.width <= panel.width)
                verify(pos.y >= 0 && pos.y+save.height <= panel.height)
                verify(scroll.width > 0 && scroll.height > 0)
                if (data.tag === "desktop-dark" || (data.tag === "desktop-light" && settings.activeTab === "ia"))
                    grabImage(scene).save("F35_IMAGES/"+data.tag+"-"+settings.activeTab+".png")
            }
            scene.width=1366; scene.height=768; Theme.isDark=true
        }
    }
}
