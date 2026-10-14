import QtQuick
import QtTest
import qml 1.0 as App
import qml.components 1.0 as Components

Item {
    width: 600
    height: 700

    App.ChatView {
        id: chat
        width: 500
        height: 500
    }
    Components.InputBar {
        id: input
        y: 510
        width: 500
    }
    Components.TrayMenu {
        id: tray
        backendUrl: "http://127.0.0.1:1"
    }

    SignalSpy { id: shown; target: tray; signalName: "showRequested" }
    SignalSpy { id: hidden; target: tray; signalName: "hideRequested" }
    SignalSpy { id: dictated; target: tray; signalName: "dictateRequested" }

    TestCase {
        name: "GateW1R"
        when: windowShown

        function init() {
            chat.messages = []
            input.voiceEnabled = false
            tray.voiceEnabled = false
            tray.windowVisible = true
            shown.clear(); hidden.clear(); dictated.clear()
        }
        function row(role, text, streaming) {
            return {role: role, authorLabel: role, content: text, raw: text,
                    timestamp: "12:00", isStreaming: streaming, toolCalls: []}
        }
        function list() { return findChild(chat, "messageList") }

        function test_stream_updates_only_active_delegate() {
            chat.messages = [row("user", "áéíóú ñ 😀", false), row("assistant", "", true)]
            tryCompare(list(), "count", 2)
            wait(100)
            var user = list().itemAtIndex(0)
            var assistant = list().itemAtIndex(1)
            verify(user !== null && assistant !== null)
            for (var i = 1; i <= 50; ++i) {
                var next = chat.messages.slice()
                next[1] = row("assistant", "ñ😀".repeat(i), true)
                chat.messages = next
            }
            compare(list().itemAtIndex(0), user)
            compare(list().itemAtIndex(1), assistant)
            compare(list().itemAtIndex(1).modelData.raw, "ñ😀".repeat(50))
            var done = chat.messages.slice()
            done[1] = row("assistant", "ñ😀".repeat(50), false)
            chat.messages = done
            compare(list().itemAtIndex(1).modelData.isStreaming, false)
        }

        function test_cancel_switch_error_and_recovery() {
            chat.messages = [row("user", "pedido", false), row("assistant", "Cancelando…", true)]
            tryCompare(list(), "count", 2)
            wait(100)
            var user = list().itemAtIndex(0)
            var next = chat.messages.slice()
            next[1] = row("assistant", "Cancelado", false)
            chat.messages = next
            compare(list().itemAtIndex(0), user)
            compare(list().itemAtIndex(1).modelData.raw, "Cancelado")
            chat.messages = [row("user", "otra sesión", false)]
            tryCompare(list(), "count", 1)
            tryVerify(function() { return list().itemAtIndex(0) && list().itemAtIndex(0).modelData.raw === "otra sesión" }, 500)
            chat.messages = [row("user", "fallo", false), row("assistant", "⚠ proveedor 503", false)]
            tryCompare(list(), "count", 2)
            tryVerify(function() { return list().itemAtIndex(1) && list().itemAtIndex(1).modelData.raw === "⚠ proveedor 503" }, 500)
            chat.messages = [row("user", "historial", false), row("assistant", "recuperado", false)]
            tryVerify(function() { return list().itemAtIndex(1) && list().itemAtIndex(1).modelData.raw === "recuperado" }, 500)
        }

        function test_tool_approval_badge_update() {
            var pending = row("assistant", "esperando", true)
            pending.toolCalls = [{id: "op-1", name: "test", status: "confirm", result: "", imageUrl: "", caption: ""}]
            chat.messages = [pending]
            tryCompare(list(), "count", 1)
            tryVerify(function() { return list().itemAtIndex(0) && list().itemAtIndex(0).modelData.raw === "esperando" }, 500)
            var bubble = list().itemAtIndex(0)
            var approved = Object.assign({}, pending, {toolCalls: [Object.assign({}, pending.toolCalls[0], {status: "running"})]})
            chat.messages = [approved]
            compare(list().itemAtIndex(0), bubble)
            tryVerify(function() { return list().itemAtIndex(0).modelData.toolCalls[0].status === "running" }, 500)
        }

        function test_large_model_and_repeated_turns() {
            var rows = []
            for (var i = 0; i < 120; ++i) rows.push(row(i % 2 ? "assistant" : "user", "mensaje " + i, false))
            chat.messages = rows
            tryCompare(list(), "count", 120)
            for (var n = 0; n < 20; ++n) {
                var updated = chat.messages.slice()
                updated[119] = row("assistant", "turno " + n, false)
                chat.messages = updated
            }
            compare(list().count, 120)
            chat.messages = []
            tryCompare(list(), "count", 0)
            compare(chat.showWelcome, true)
        }

        function test_insert_remove_and_equal_length_reload() {
            chat.messages = [row("user", "primero", false), row("assistant", "último", false)]
            tryCompare(list(), "count", 2)
            wait(100)
            var first = list().itemAtIndex(0)
            verify(first !== null)

            var inserted = [chat.messages[0], row("user", "intermedio ñ😀", false), chat.messages[1]]
            chat.messages = inserted
            tryCompare(list(), "count", 3)
            compare(list().itemAtIndex(0), first)
            tryVerify(function() { return list().itemAtIndex(1) && list().itemAtIndex(1).modelData.raw === "intermedio ñ😀" }, 500)
            compare(list().model.get(2).modelData.raw, "último")

            chat.messages = [inserted[0], inserted[2]]
            tryCompare(list(), "count", 2)
            tryVerify(function() { return list().itemAtIndex(1) && list().itemAtIndex(1).modelData.raw === "último" }, 500)

            chat.messages = [row("user", "otra sesión", false), row("assistant", "completado", false)]
            tryVerify(function() { return list().itemAtIndex(0) && list().itemAtIndex(0).modelData.raw === "otra sesión" }, 500)
            tryVerify(function() { return list().itemAtIndex(1) && list().itemAtIndex(1).modelData.raw === "completado" }, 500)
        }

        function test_stream_does_not_take_scroll_from_reader() {
            var rows = []
            for (var i = 0; i < 80; ++i) rows.push(row(i % 2 ? "assistant" : "user", "línea " + i, false))
            rows[79] = row("assistant", "", true)
            chat.messages = rows
            tryCompare(list(), "count", 80)
            tryVerify(function() { return list().contentHeight > list().height }, 1000)
            wait(250)
            list().follow = false
            list().positionViewAtBeginning()
            wait(50)
            tryVerify(function() { return !list().atYEnd }, 500)
            var top = list().contentY
            for (var n = 0; n < 20; ++n) {
                var updated = chat.messages.slice()
                updated[79] = row("assistant", "token ".repeat(n + 1), true)
                chat.messages = updated
            }
            wait(100)
            verify(list().contentY <= top + 2)
        }

        function test_tray_toggle_and_deferred_voice() {
            tray.rebuildRows()
            compare(tray.rows[0].label, "Ocultar ventana")
            verify(!tray.rows.some(function(r) { return r.act === "dictate" }))
            tray.handleRow("dictate", "")
            compare(dictated.count, 0)
            tray.handleRow("toggle", "")
            compare(hidden.count, 1)
            tray.windowVisible = false
            tray.rebuildRows()
            compare(tray.rows[0].label, "Mostrar ventana")
            tray.handleRow("toggle", "")
            compare(shown.count, 1)
            tray.voiceEnabled = true
            tray.rebuildRows()
            verify(tray.rows.some(function(r) { return r.act === "dictate" }))
        }

        function test_rapid_tray_toggle() {
            for (var i = 0; i < 20; ++i) {
                tray.windowVisible = (i % 2 === 0)
                tray.rebuildRows()
                tray.handleRow("toggle", "")
            }
            compare(hidden.count, 10)
            compare(shown.count, 10)
        }

        function test_input_voice_capability() {
            var mic = findChild(input, "dictationButton")
            verify(mic !== null)
            compare(mic.visible, false)
            compare(mic.enabled, false)
            input.voiceEnabled = true
            compare(mic.visible, true)
            compare(mic.enabled, true)
            input.streaming = true
            compare(mic.enabled, false)
            input.streaming = false
        }
    }
}
