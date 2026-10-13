import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0

Rectangle {
    id: root
    objectName: "recoveryHistory"
    property bool open_: false
    property string backendUrl: ""
    property string authToken: ""
    property string sessionId: ""
    property var entries: []
    property string status: "idle"
    property int generation: 0
    property bool hasOlder: false
    property string deletionSession: ""
    property bool confirmingDeletion: false
    readonly property string uncertainExplanation: qsTr("La operación pudo haberse realizado, pero no se pudo confirmar su resultado.")
    readonly property string deletionExplanation: qsTr("Eliminará la conversación y la evidencia de operaciones inciertas. Esa información no podrá utilizarse después para investigar la operación. Borrar filas SQLite no garantiza destruir copias, WAL o backups.")
    signal eraseConfirmed(string sessionId)
    signal closed()
    function explanation(state) {
        if (state === "unknown_outcome") return uncertainExplanation
        if (state === "failed") return qsTr("La operación falló; pudo dejar efectos parciales.")
        if (state === "abandoned") return qsTr("El intento se abandonó antes de registrar el despacho.")
        return qsTr("Estado histórico sin información completa.")
    }
    function show(session) {
        sessionId = session || ""
        confirmingDeletion = false
        open_ = true
        load(null)
    }
    function confirmDeletion(session) {
        deletionSession = session
        sessionId = session
        confirmingDeletion = true
        open_ = true
        load(null)
    }
    function hide() {
        generation++
        open_ = false
        confirmingDeletion = false
        closed()
    }
    function load(before) {
        var requestGeneration = ++generation
        status = "loading"
        entries = []
        hasOlder = false
        var url = backendUrl + "/api/tools/operations?recovery_only=true&limit=50"
        if (sessionId) url += "&session_id=" + encodeURIComponent(sessionId)
        if (before !== null) url += "&before=" + before
        var xhr = new XMLHttpRequest()
        xhr.open("GET", url)
        if (authToken) xhr.setRequestHeader("Authorization", "Bearer " + authToken)
        xhr.onreadystatechange = function() {
            if (xhr.readyState !== XMLHttpRequest.DONE || requestGeneration !== generation || !open_) return
            if (xhr.status !== 200) { status = "unavailable"; return }
            try {
                var records = JSON.parse(xhr.responseText)
                if (!Array.isArray(records)) throw new Error("invalid records")
                entries = records
                hasOlder = records.length === 50
                status = entries.length ? "ready" : "empty"
            } catch (e) { status = "unavailable" }
        }
        xhr.send()
    }
    focus: open_
    Keys.onEscapePressed: hide()
    visible: open_
    color: Theme.overlayBackdrop
    z: 999
    MouseArea { anchors.fill: parent; onClicked: root.hide() }
    Rectangle {
        anchors.centerIn: parent
        width: Math.min(parent.width - Theme.spacingLg * 2, Theme.windowDefaultWidth)
        height: Math.min(parent.height - Theme.spacingLg * 2, Theme.windowDefaultHeight)
        color: Theme.surface
        radius: Theme.radiusLg
        border.color: Theme.hairline
        MouseArea { anchors.fill: parent }
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: Theme.spacingLg
            spacing: Theme.spacingMd
            Text {
                text: qsTr("Historial de recuperación")
                color: Theme.ink
                font: Theme.font(Theme.fontSizeTagline, Theme.weightBold, Theme.lsBody)
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }
            Text {
                text: qsTr("Inspecciona el posible efecto antes de repetir una operación. Para repetirla, envía una solicitud nueva y explícita.")
                color: Theme.inkMuted
                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, Theme.lsBody)
                Layout.fillWidth: true
                wrapMode: Text.Wrap
            }
            Text {
                visible: root.status !== "ready"
                text: root.status === "loading" ? qsTr("Cargando…") : root.status === "unavailable" ? qsTr("No se pudo consultar la recuperación. No se ha confirmado la ausencia de registros.") : qsTr("Sin registros de recuperación. La sesión puede haberse eliminado o no tener operaciones registradas.")
                color: root.status === "unavailable" ? Theme.error : Theme.inkMuted
                font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, Theme.lsBody)
                wrapMode: Text.Wrap
                Layout.fillWidth: true
            }
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                Column {
                    width: parent.width
                    spacing: Theme.spacingMd
                    Repeater {
                        model: root.entries
                        delegate: Column {
                            required property var modelData
                            width: parent.width
                            spacing: Theme.spacingXs
                            Text {
                                width: parent.width
                                textFormat: Text.PlainText
                                text: (modelData.tool || qsTr("Herramienta desconocida")) + " · " + (modelData.prepared_at || qsTr("Fecha desconocida")) + " · " + (modelData.state || qsTr("Estado desconocido"))
                                wrapMode: Text.Wrap
                                color: Theme.ink
                                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, Theme.lsBody)
                            }
                            Text {
                                width: parent.width
                                text: root.explanation(modelData.state)
                                color: modelData.state === "unknown_outcome" ? Theme.warn : Theme.inkMuted
                                wrapMode: Text.Wrap
                                font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, Theme.lsBody)
                            }
                            Text {
                                width: parent.width
                                textFormat: Text.PlainText
                                text: qsTr("Sesión: ") + (modelData.session_id || qsTr("Sin asociación")) + "\n" + qsTr("Operación: ") + (modelData.id || qsTr("Identidad no disponible"))
                                wrapMode: Text.WrapAnywhere
                                color: Theme.inkMuted
                                font: Theme.font(Theme.fontSizeMicro, Theme.weightNormal, Theme.lsBody)
                            }
                            PillButton {
                                visible: !!modelData.session_id && !root.confirmingDeletion
                                width: parent.width
                                elideText: true
                                text: qsTr("Eliminar conversación y metadatos…")
                                onClicked: root.confirmDeletion(modelData.session_id)
                            }

                        }
                    }
                }
            }
            Text {
                visible: root.confirmingDeletion
                text: root.deletionExplanation
                color: Theme.error
                wrapMode: Text.Wrap
                Layout.fillWidth: true
                font: Theme.font(Theme.fontSizeBodySmall, Theme.weightNormal, Theme.lsBody)
            }
            RowLayout {
                visible: !root.confirmingDeletion
                Layout.fillWidth: true
                spacing: Theme.spacingXs
                PillButton { text: qsTr("Actualizar"); onClicked: root.load(null) }
                PillButton {
                    visible: root.hasOlder
                    text: qsTr("Anteriores")
                    onClicked: root.load(root.entries[root.entries.length - 1].audit_id)
                }
                Item { Layout.fillWidth: true }
                PillButton { text: qsTr("Cerrar"); onClicked: root.hide() }
            }
            RowLayout {
                visible: root.confirmingDeletion
                Layout.fillWidth: true
                spacing: Theme.spacingXs
                PillButton {
                    Layout.fillWidth: true
                    elideText: true
                    activeFocusOnTab: true
                    Keys.onReturnPressed: clicked()
                    Keys.onSpacePressed: clicked()
                    objectName: "eraseEvidenceButton"
                    text: qsTr("Eliminar evidencia")
                    activeColor: Theme.error
                    active: true
                    onClicked: {
                        var session = root.deletionSession
                        root.hide()
                        root.eraseConfirmed(session)
                    }
                }
                PillButton { text: qsTr("Cancelar"); activeFocusOnTab: true; Keys.onReturnPressed: root.hide(); Keys.onSpacePressed: root.hide(); onClicked: root.hide() }
            }
        }
    }
}
