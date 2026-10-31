import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/components" as Components
import "../../qml" as Design
Item {
    id: scene
    width: 720; height: 200
    property var callbacks: []
    property var calls: []
    Components.InputBar {
        id: bar
        anchors.fill: parent
        voiceEnabled: true
        voiceStatus: controller.state
        voiceBusy: controller.busy
        recording: controller.state === "recording"
        onVoiceCancelClicked: controller.cancel()
        text: "Borrador previo"
    }
    Components.VoiceController {
        id: controller
        sessionId: "synthetic-session"
        onTranscriptReady: function(text) { bar.text = bar.text + " " + text }
        function request(path, body, callback) {
            scene.calls.push({path: path, body: body})
            scene.callbacks.push(callback)
        }
    }
    Components.MessageBubble { id: voiceMessage; width: 400; role: "assistant"; rawContent: "Respuesta sintética"; content: "Respuesta sintética"; visible: false }
    SignalSpy { id: sendSpy; target: bar; signalName: "sendClicked" }
    SignalSpy { id: transcriptSpy; target: controller; signalName: "transcriptReady" }
    SignalSpy { id: wakeSpy; target: controller; signalName: "wakeDetected" }
    TestCase {
        name: "F4Voice"
        when: windowShown
        function reply(ok, value) { var cb = scene.callbacks.shift(); verify(!!cb); cb(ok,value) }
        function init() {
            controller.cancel()
            scene.callbacks=[]; scene.calls=[]
            controller.state="idle"; controller.identity=null; controller.sessionId="synthetic-session"
            controller.deliveredGeneration=""
            bar.voiceReady=true
            bar.text="Borrador previo"
            sendSpy.clear(); transcriptSpy.clear()
            wakeSpy.clear()
            controller.backgroundEnabled=false; controller.backgroundPaused=false; controller.microphoneActive=false
            controller.lastActivation="";controller.deliveredBackground=[];controller.backgroundRequest=false;controller.backgroundChanging=false
        }
        function begin() {
            controller.start()
            compare(controller.state,"preparing")
            reply(true,{session_id:"synthetic-session",scope:"scope-one"})
            reply(true,{state:"preparing",identity:{session_id:"synthetic-session",generation:"scope-one",turn_id:null}})
            // start() requests a status snapshot immediately.
            reply(true,{state:"recording",identity:{session_id:"synthetic-session",generation:"scope-one",turn_id:null}})
        }
        function test_manual_playback_requires_explicit_resource_availability() {
            var button = findChild(voiceMessage,"messagePlayButton")
            voiceMessage.speechAvailable = false
            verify(!button.enabled)
            verify(button.accessibleName.indexOf("no disponible") >= 0)
            voiceMessage.speechAvailable = true
            verify(button.enabled)
            compare(button.accessibleName,"Leer respuesta en voz alta")
            voiceMessage.speechAvailable = false
        }
        function test_toggle_states_and_editable_draft_never_autosends() {
            begin()
            compare(controller.state,"recording")
            verify(!bar.canSend)
            compare(findChild(bar,"dictationButton").accessibleName, "Terminar grabación y transcribir")
            verify(findChild(bar,"voiceCancelButton").visible)
            controller.applySnapshot({state:"transcribing",identity:controller.identity})
            verify(findChild(bar,"voiceStatusLabel").text.indexOf("Transcribiendo")>=0)
            controller.applySnapshot({state:"transcript_ready",identity:controller.identity,transcript:"texto dictado"})
            compare(bar.text,"Borrador previo texto dictado")
            compare(sendSpy.count,0)
            controller.applySnapshot({state:"transcript_ready",identity:controller.identity,transcript:"duplicado"})
            compare(transcriptSpy.count,1)
            verify(bar.canSend)
            bar.text="Corrección manual";compare(bar.text,"Corrección manual")
        }
        function test_wake_is_explicit_one_shot_and_never_sends() {
            bar.wakeAvailable = true
            compare(controller.state, "idle")
            verify(!findChild(bar, "wakeWordButton").active)
            controller.start(true)
            reply(true, {session_id:"synthetic-session", scope:"wake-scope"})
            compare(scene.calls[1].body.wake, true)
            reply(true, {state:"preparing",identity:{session_id:"synthetic-session",generation:"wake-scope",turn_id:null}})
            reply(true, {state:"wake_waiting",identity:controller.identity})
            verify(controller.busy)
            verify(!bar.canSend)
            verify(findChild(bar, "wakeWordButton").enabled)
            verify(findChild(bar, "voiceStatusLabel").text.indexOf("Hey Jarvis") >= 0)
            controller.start(true)
            compare(scene.calls.filter(function(c) { return c.path === "/api/voice/start" }).length, 1)
            controller.applySnapshot({state:"transcript_ready", identity:controller.identity, transcript:"Abre Dolphin"})
            compare(bar.text, "Borrador previo Abre Dolphin")
            compare(sendSpy.count, 0)
            verify(!controller.polling)
            verify(!findChild(bar, "wakeWordButton").active)
            bar.wakeAvailable = false
        }
        function test_wake_cancel_revokes_late_detection() {
            controller.start(true)
            reply(true, {session_id:"synthetic-session", scope:"wake-scope"})
            reply(true, {state:"preparing",identity:{session_id:"synthetic-session",generation:"wake-scope",turn_id:null}})
            reply(true, {state:"wake_waiting",identity:controller.identity})
            var old = controller.identity
            controller.cancel()
            reply(true, {state:"idle"})
            controller.applySnapshot({state:"transcript_ready",identity:old,transcript:"Obsoleto"})
            compare(transcriptSpy.count, 0)
            compare(bar.text, "Borrador previo")
            compare(sendSpy.count, 0)
        }
        function test_cancel_after_draft_is_available_and_preserves_manual_edit() {
            begin()
            controller.applySnapshot({state:"transcript_ready",identity:controller.identity,transcript:"Abre el navegador"})
            var cancelButton = findChild(bar,"voiceCancelButton")
            verify(cancelButton.visible)
            verify(cancelButton.accessibleName.indexOf("conservar") >= 0)
            bar.text = "Abre el navegador por favor"
            bar.voiceCancelClicked()
            compare(scene.calls[scene.calls.length - 1].path,"/api/voice/cancel")
            reply(true,{state:"idle"})
            compare(controller.state,"idle")
            compare(bar.text,"Abre el navegador por favor")
            verify(!cancelButton.visible)
            compare(sendSpy.count,0)
        }
        function test_background_wake_without_click_survives_hide_delivers_and_acks_once() {
            var id={session_id:"synthetic-session",generation:"00000000-0000-4000-8000-000000000001",turn_id:null}
            controller.consumeStatus({state:"recording",identity:id,background:{enabled:true,paused:false,state:"busy",microphone_active:true,activation:id,transcripts:[]}})
            compare(wakeSpy.count,1)
            verify(controller.backgroundEnabled)
            var count=scene.calls.length
            controller.hide()
            compare(scene.calls.length,count,"Hiding must not detach an enabled background session")
            controller.consumeStatus({state:"transcript_ready",identity:id,transcript:"Abre el navegador",background:{enabled:true,paused:false,state:"listening",microphone_active:true,activation:id,transcripts:[{identity:id,transcript:"Abre el navegador"}]}})
            compare(wakeSpy.count,1)
            compare(transcriptSpy.count,1)
            compare(sendSpy.count,0)
            compare(scene.calls[scene.calls.length-1].body.action,"ack")
            controller.consumeStatus({state:"transcript_ready",identity:id,transcript:"Abre el navegador",background:{enabled:true,paused:false,state:"listening",microphone_active:true,activation:id,transcripts:[{identity:id,transcript:"Abre el navegador"}]}})
            compare(transcriptSpy.count,1)
            controller.cancel()
            verify(controller.backgroundEnabled,"Cancel interaction is not Disable background")
        }
        function test_background_ui_privacy_pause_and_disable_are_available_while_busy() {
            bar.backgroundEnabled=true;bar.backgroundPaused=false;bar.microphoneActive=true;bar.backgroundState="listening"
            verify(findChild(bar,"backgroundDisableButton").visible)
            verify(findChild(bar,"wakeWordButton").accessibleName.indexOf("Pausar")>=0)
            verify(findChild(bar,"voiceStatusLabel").text.indexOf("Micrófono abierto")>=0)
            bar.backgroundPaused=true;bar.microphoneActive=false
            verify(findChild(bar,"wakeWordButton").accessibleName.indexOf("Reanudar")>=0)
            bar.backgroundEnabled=false;bar.backgroundPaused=false
        }
        function test_cancel_before_start_reply_never_opens_capture() {
            controller.start()
            controller.cancel()
            reply(true,{session_id:"synthetic-session",scope:"late"})
            compare(scene.calls.length,1)
            compare(controller.state,"idle")
            compare(transcriptSpy.count,0)
        }
        function test_cancel_pending_start_uses_known_server_scope() {
            controller.start()
            reply(true,{session_id:"synthetic-session",scope:"scope-one"})
            controller.cancel()
            compare(scene.calls[scene.calls.length-1].path,"/api/voice/cancel")
            compare(scene.calls[scene.calls.length-1].body.generation,"scope-one")
            reply(true,{state:"recording",identity:{session_id:"synthetic-session",generation:"scope-one",turn_id:null}})
            reply(true,{state:"idle"})
            compare(controller.state,"idle")
            compare(transcriptSpy.count,0)
        }
        function test_session_switch_discards_stale_transcript() {
            begin()
            var old=controller.identity
            controller.sessionId="new-session"
            controller.applySnapshot({state:"transcript_ready",identity:old,transcript:"old data"})
            compare(transcriptSpy.count,0)
            compare(bar.text,"Borrador previo")
            reply(true,{state:"idle"})
        }
        function test_error_and_tts_stop_do_not_claim_llm_cancel() {
            begin()
            controller.applySnapshot({state:"error",identity:controller.identity,error:"Modelo ausente"})
            compare(controller.state,"error")
            verify(findChild(bar,"voiceStatusLabel").text.length>0)
            compare(scene.calls.filter(function(c){return c.path==="/api/chat/cancel"}).length,0)
        }
        function test_config_revocation_clears_old_voice_identity() {
            begin()
            controller.applySnapshot({state:"idle",identity:null})
            compare(controller.identity,null)
            compare(controller.state,"idle")
            verify(!controller.busy)
            compare(sendSpy.count,0)
        }
        function test_missing_model_disables_microphone_but_preserves_text() {
            bar.voiceReady=false
            bar.voiceUnavailableText="Modelo STT no instalado"
            verify(!findChild(bar,"dictationButton").enabled)
            compare(findChild(bar,"voiceStatusLabel").text,"Modelo STT no instalado")
            verify(bar.canSend)
        }
    }
}
