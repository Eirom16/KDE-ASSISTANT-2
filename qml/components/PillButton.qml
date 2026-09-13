// PillButton.qml - Boton capsula compacto para uso frecuente
// Variante mas simple y pequena que AppleButton, ideal para chips y sugerencias

import QtQuick
import qml 1.0

Rectangle {
    id: root

    property string text: ""
    property string iconName: ""
    property bool active: false
    property color activeColor: Theme.primary
    // elideText: si el texto no cabe, se recorta con "…" y al pasar el raton
    // se desplaza en marquesina para mostrarlo completo. Usarlo solo cuando
    // la capsula tiene un ancho impuesto por el padre (ej. Layout.fillWidth).
    property bool elideText: false
    signal clicked()

    property int iconSize: Theme.iconSizeSm
    property int textSize: Theme.fontSizeCaption
    property int paddingH: Theme.spacingSm + 4
    property int paddingV: Theme.spacingXs

    implicitHeight: row.implicitHeight + paddingV * 2
    implicitWidth: row.implicitWidth + paddingH * 2
    radius: Theme.radiusPill
    color: {
        if (active) return activeColor
        if (mouseArea.pressed) return Theme.surfacePressed
        if (mouseArea.containsMouse) return Theme.surfaceHover
        return "transparent"
    }
    border.width: active ? 0 : 1
    border.color: Theme.hairline

    Behavior on color {
        ColorAnimation { duration: Theme.animFast }
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: Theme.spacingXs

        Octicon {
            id: icon
            visible: root.iconName !== ""
            name: root.iconName
            size: root.iconSize
            color: root.active ? Theme.inkOnPrimary : Theme.ink
            anchors.verticalCenter: parent.verticalCenter
        }

        // Contenedor con clip: recorta el texto al ancho disponible y permite
        // la marquesina en hover sin que el texto pinte fuera de la capsula.
        Item {
            id: textClip
            visible: root.text !== ""
            anchors.verticalCenter: parent.verticalCenter
            clip: true

            readonly property int maxTextWidth: root.elideText
                ? Math.max(0, root.width - root.paddingH * 2 - (icon.visible ? icon.width + row.spacing : 0))
                : label.implicitWidth
            readonly property bool overflow: label.implicitWidth > width + 1

            width: Math.max(0, Math.min(label.implicitWidth, maxTextWidth))
            height: label.implicitHeight

            Text {
                id: label
                text: root.text
                font: Theme.font(root.textSize, Theme.weightBold, -0.12)
                color: root.active ? Theme.inkOnPrimary : Theme.ink

                // Con overflow + hover: ancho completo y marquesina; con
                // overflow sin hover: elide; si cabe justo: sin elide.
                readonly property bool marquee: root.elideText && textClip.overflow && mouseArea.containsMouse
                width: marquee ? implicitWidth : textClip.width
                elide: !marquee && textClip.overflow ? Text.ElideRight : Text.ElideNone

                onMarqueeChanged: {
                    if (marquee) {
                        marqueeAnim.restart()
                    } else {
                        marqueeAnim.stop()
                        x = 0
                    }
                }

                SequentialAnimation {
                    id: marqueeAnim
                    loops: Animation.Infinite
                    PauseAnimation { duration: 450 }
                    NumberAnimation {
                        target: label
                        property: "x"
                        to: -(label.implicitWidth - textClip.width)
                        duration: Math.max(900, (label.implicitWidth - textClip.width) * 40)
                        easing.type: Easing.InOutSine
                    }
                    PauseAnimation { duration: 900 }
                    NumberAnimation {
                        target: label
                        property: "x"
                        to: 0
                        duration: 500
                        easing.type: Easing.InOutSine
                    }
                }
            }
        }
    }

    MouseArea {
        id: mouseArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    scale: mouseArea.pressed ? 0.97 : 1.0
    Behavior on scale {
        NumberAnimation { duration: Theme.animFast }
    }
}
