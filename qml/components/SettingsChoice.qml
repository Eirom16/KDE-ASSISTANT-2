import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import qml 1.0

ComboBox {
    id: control
    Layout.fillWidth: true
    implicitHeight: Theme.buttonIconSize
    implicitWidth: Theme.drawerWidth
    Layout.minimumWidth: 0
    rightPadding: Theme.spacingXl
    contentItem: TextInput {
        text: control.editable ? control.editText : control.displayText
        font: control.font
        color: Theme.ink
        selectionColor: Theme.primary
        selectedTextColor: Theme.inkOnPrimary
        leftPadding: Theme.spacingSm
        verticalAlignment: TextInput.AlignVCenter
        readOnly: !control.editable
        selectByMouse: control.editable
        activeFocusOnPress: control.editable
        clip: true
        onTextEdited: control.editText = text
    }
    indicator: Octicon {
        x: control.width - width - Theme.spacingSm
        y: (control.height - height) / 2
        name: "arrow-up-16"
        rotation: 180
        color: Theme.inkMuted80
        size: Theme.iconSizeSm
    }
    font: Theme.font(Theme.fontSizeBody)
    selectTextByMouse: true
    Accessible.name: objectName
    background: Rectangle {
        radius: Theme.radiusMd
        color: Theme.surfacePearl
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Theme.primary : Theme.hairline
    }
    palette.text: Theme.ink
    palette.buttonText: Theme.ink
    palette.base: Theme.surfacePearl
    palette.window: Theme.surface
    palette.highlight: Theme.primary
    palette.highlightedText: Theme.inkOnPrimary
}
