import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0

Item {
    id: root
    objectName: "modelPicker"
    property alias editText: input.text
    property var models: []
    implicitHeight: 40
    RowLayout {
        anchors.fill: parent
        spacing: Theme.spacingXs
        TextField {
            id: input
            objectName: "modelInput"
            Layout.fillWidth: true
            Layout.fillHeight: true
            color: Theme.ink
            placeholderText: qsTr("Identificador del modelo")
            Accessible.name: qsTr("Modelo de IA")
            selectByMouse: true
            background: Rectangle { color: Theme.surfacePearl; radius: Theme.radiusMd; border.color: input.activeFocus ? Theme.primary : Theme.hairline }
            Keys.onDownPressed: picker.open()
        }
        IconButton {
            iconName: "search-16"
            buttonSize: 36
            accessibleName: qsTr("Ver y buscar modelos de IA")
            onClicked: picker.open()
        }
    }
    Popup {
        id: picker
        objectName: "modelPopup"
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(parent.width - 24, 560)
        height: Math.min(parent.height - 40, 480)
        padding: Theme.spacingMd
        z: 1100
        modal: true
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        onOpened: { search.text = ""; search.forceActiveFocus() }
        background: Rectangle { color: Theme.surface; radius: Theme.radiusLg; border.color: Theme.hairline }
        contentItem: ColumnLayout {
            spacing: Theme.spacingSm
            Text { text: qsTr("Seleccionar modelo"); color: Theme.ink; font.pixelSize: Theme.fontSizeTagline }
            TextField {
                id: search
                objectName: "modelSearch"
                Layout.fillWidth: true
                placeholderText: qsTr("Buscar por nombre…")
                Accessible.name: qsTr("Filtrar modelos")
                color: Theme.ink
                background: Rectangle { radius: Theme.radiusSm; color: Theme.surfacePearl; border.color: Theme.hairline }
                Keys.onDownPressed: { choices.currentIndex = 0; choices.forceActiveFocus() }
                onAccepted: { if (choices.count > 0) root.selectModel(choices.model[Math.max(0, choices.currentIndex)]) }
            }
            ListView {
                id: choices
                objectName: "modelChoices"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: root.models.filter(function(m) { return String(m).toLowerCase().indexOf(search.text.toLowerCase()) >= 0 })
                currentIndex: 0
                ScrollBar.vertical: ScrollBar {}
                Keys.onReturnPressed: { if (currentIndex >= 0 && count) root.selectModel(model[currentIndex]) }
                delegate: ItemDelegate {
                    required property string modelData
                    required property int index
                    width: choices.width
                    height: 48
                    text: modelData
                    highlighted: choices.currentIndex === index
                    Accessible.name: modelData
                    onClicked: root.selectModel(modelData)
                    contentItem: Text { text: parent.text; color: Theme.ink; elide: Text.ElideMiddle; verticalAlignment: Text.AlignVCenter }
                    background: Rectangle { color: parent.highlighted || parent.hovered ? Theme.surfaceHover : "transparent"; radius: Theme.radiusSm }
                }
            }
            Text {
                visible: choices.count === 0
                text: root.models.length ? qsTr("Sin coincidencias") : qsTr("No hay modelos cargados. Puedes escribir el identificador manualmente.")
                color: Theme.inkMuted; wrapMode: Text.Wrap; Layout.fillWidth: true
            }
            AppleButton { text: qsTr("Cerrar"); size: "small"; variant: "ghost"; onClicked: picker.close() }
        }
    }
    function selectModel(name) { input.text = name; picker.close(); input.forceActiveFocus() }
    function openPicker() { picker.open() }
    function popupVisible() { return picker.opened }
}
