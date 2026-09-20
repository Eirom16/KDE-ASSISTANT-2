import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0
import qml.auth 1.0

Popup {
    id: root
    property string backendUrl: "http://127.0.0.1:8765"
    property var stats: ({active:[], recent:[], tool_usage:[], completed:0, errors:0, cancelled:0})
    property string errorText: ""
    property bool loading: false
    property bool autoRefresh: true
    property string view: "runs"
    objectName: "agentDashboard"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(parent.width - 24, 740)
    height: Math.min(parent.height - 32, 720)
    padding: Theme.spacingMd
    modal: true
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle { color: Theme.surface; radius: Theme.radiusLg; border.color: Theme.hairline }
    onOpened: reload()
    function reload() {
        if (loading) return
        loading = true
        var req = new XMLHttpRequest()
        req.open("GET", backendUrl + "/api/agent/dashboard")
        req.setRequestHeader("Authorization", "Bearer " + AuthToken.token)
        req.onreadystatechange = function() {
            if (req.readyState !== XMLHttpRequest.DONE) return
            loading = false
            if (req.status === 200) {
                try { stats = JSON.parse(req.responseText); errorText = "" }
                catch (e) { errorText = qsTr("No se pudo leer la actividad") }
            } else errorText = qsTr("No se pudo actualizar. Comprueba la conexión al asistente.")
        }
        req.send()
    }
    function statusText(s) {
        return s === "completed" ? qsTr("Completado") : s === "error" ? qsTr("Error") : s === "cancelled" ? qsTr("Cancelado") : qsTr("En curso")
    }
    Timer { interval: 2000; repeat: true; running: root.visible && root.autoRefresh; onTriggered: root.reload() }
    contentItem: ColumnLayout {
        spacing: Theme.spacingSm
        RowLayout {
            Layout.fillWidth: true
            Text { text: qsTr("Actividad de agentes"); color: Theme.ink; font.pixelSize: Theme.fontSizeTagline; Layout.fillWidth: true; elide: Text.ElideRight }
            IconButton { iconName: "sync-16"; accessibleName: qsTr("Actualizar actividad"); onClicked: root.reload() }
            IconButton { iconName: "x-16"; accessibleName: qsTr("Cerrar dashboard"); onClicked: root.close() }
        }
        Text { text: qsTr("Ejecuciones desde el arranque del asistente"); color: Theme.inkMuted; font.pixelSize: Theme.fontSizeCaption; Layout.fillWidth: true; wrapMode: Text.Wrap }
        GridLayout {
            Layout.fillWidth: true
            columns: root.width < 480 ? 2 : 4
            Repeater {
                model: [
                    {label:qsTr("En curso"), count:root.stats.active.length},
                    {label:qsTr("Completados"), count:root.stats.completed},
                    {label:qsTr("Errores"), count:root.stats.errors},
                    {label:qsTr("Cancelados"), count:root.stats.cancelled}
                ]
                delegate: Rectangle {
                    required property var modelData
                    Layout.fillWidth: true; Layout.preferredHeight: 76
                    radius: Theme.radiusMd; color: Theme.surfacePearl
                    Column {
                        anchors.fill: parent; anchors.margins: Theme.spacingSm
                        Text { text: modelData.count; color: Theme.ink; font.pixelSize: Theme.fontSizeHero }
                        Text { text: modelData.label; color: Theme.inkMuted; font.pixelSize: Theme.fontSizeCaption }
                    }
                }
            }
        }
        RowLayout {
            PillButton { text: qsTr("Ejecuciones"); active: root.view === "runs"; onClicked: root.view = "runs" }
            PillButton { text: qsTr("Herramientas"); active: root.view === "tools"; onClicked: root.view = "tools" }
        }
        Text {
            Layout.fillWidth: true; wrapMode: Text.Wrap; color: Theme.inkMuted; font.pixelSize: Theme.fontSizeCaption
            text: root.view === "runs" ? qsTr("Voz y chat · últimas 100 ejecuciones · duración del agente") : qsTr("Historial acumulado en este equipo · éxitos, denegaciones y tiempo medio")
        }
        Text { visible: root.errorText !== ""; text: root.errorText; color: Theme.error; Layout.fillWidth: true; wrapMode: Text.Wrap }
        ListView {
            id: entries
            Layout.fillWidth: true; Layout.fillHeight: true
            clip: true; spacing: Theme.spacingXs
            model: root.view === "runs" ? root.stats.active.concat(root.stats.recent) : root.stats.tool_usage
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                required property var modelData
                width: entries.width; height: detail.implicitHeight + title.implicitHeight + Theme.spacingLg
                radius: Theme.radiusMd; color: Theme.surfacePearl
                Column {
                    anchors.fill: parent; anchors.margins: Theme.spacingSm; spacing: 4
                    Text {
                        id: title
                        width: parent.width; elide: Text.ElideMiddle; color: Theme.ink; font.pixelSize: Theme.fontSizeBody
                        text: root.view === "runs" ? (modelData.channel === "voice" ? qsTr("Voz") : qsTr("Chat")) + " · " + modelData.model : modelData.tool
                    }
                    Text {
                        id: detail
                        width: parent.width; wrapMode: Text.Wrap; color: Theme.inkMuted; font.pixelSize: Theme.fontSizeCaption
                        text: root.view === "runs"
                            ? root.statusText(modelData.status) + " · " + (modelData.duration_ms / 1000).toFixed(1) + " s · " + modelData.tools + qsTr(" herramientas") + " · " + Qt.formatDateTime(new Date(modelData.started_at), "dd/MM hh:mm")
                            : modelData.total + qsTr(" usos · ") + modelData.success + qsTr(" éxitos · ") + modelData.denied + qsTr(" denegados · ") + Math.round(modelData.avg_ms) + " ms"
                    }
                }
            }
            Text {
                anchors.centerIn: parent; width: parent.width; wrapMode: Text.Wrap; horizontalAlignment: Text.AlignHCenter
                visible: entries.count === 0 && !root.loading
                text: qsTr("Aún no hay actividad. Inicia una conversación o pide una acción.")
                color: Theme.inkMuted; font.pixelSize: Theme.fontSizeBody
            }
        }
    }
}
