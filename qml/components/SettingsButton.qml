import QtQuick
import QtQuick.Controls
import qml 1.0

Button {
    id: control
    property bool primary: false
    property bool destructive: false
    implicitHeight: Theme.buttonIconSize
    implicitWidth: Math.max(Theme.buttonIconSize * 2, label.implicitWidth + Theme.spacingLg * 2)
    font: Theme.font(Theme.fontSizeButtonUtility, Theme.weightBold, 0)
    Accessible.name: text
    opacity: enabled ? 1 : 0.5
    contentItem: Text {
        id: label
        text: control.text
        font: control.font
        color: control.primary ? Theme.inkOnPrimary : control.destructive ? Theme.error : Theme.ink
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: Theme.radiusMd
        color: control.primary ? (control.down ? Qt.darker(Theme.primaryFocus, 1.15) : Theme.primaryFocus) : control.hovered ? Theme.surfaceHover : Theme.surfacePearl
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? Theme.primary : Theme.hairline
    }
}
