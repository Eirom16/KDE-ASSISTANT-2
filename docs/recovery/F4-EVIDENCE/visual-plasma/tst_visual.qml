import QtQuick
import QtQuick.Controls
import QtTest
import "file:///home/eirom/KDE-ASSISTANT-2/qml/components" as Components
import "file:///home/eirom/KDE-ASSISTANT-2/qml" as Design
Rectangle {
    color: Design.Theme.canvasSolid
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
    SignalSpy { id: sendSpy; target: bar; signalName: "sendClicked" }
    SignalSpy { id: transcriptSpy; target: controller; signalName: "transcriptReady" }
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
        }
        function begin() {
            controller.start()
            compare(controller.state,"preparing")
            reply(true,{session_id:"synthetic-session",scope:"scope-one"})
            reply(true,{state:"preparing",identity:{session_id:"synthetic-session",generation:"scope-one",turn_id:null}})
            // start() requests a status snapshot immediately.
            reply(true,{state:"recording",identity:{session_id:"synthetic-session",generation:"scope-one",turn_id:null}})
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
        function test_missing_model_disables_microphone_but_preserves_text() {
            bar.voiceReady=false
            bar.voiceUnavailableText="Modelo STT no instalado"
            verify(!findChild(bar,"dictationButton").enabled)
            compare(findChild(bar,"voiceStatusLabel").text,"Modelo STT no instalado")
            verify(bar.canSend)
        }
        function test_visual_voice_states() {
            controller.polling=false
            for (var theme=0; theme<2; theme++) {
                Design.Theme.isDark = theme===0
                for (var i=0; i<3; i++) {
                    controller.state=["recording","transcribing","error"][i]
                    wait(50)
                    grabImage(scene).save("/home/eirom/KDE-ASSISTANT-2/docs/recovery/F4-EVIDENCE/visual-plasma" + "/" + (theme===0 ? "dark" : "light") + "-" + controller.state + ".png")
                }
            }
        }
    }
}
