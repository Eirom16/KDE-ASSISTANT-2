// AppleButton.qml - Boton base estilo Apple con animaciones spring
// Tres variantes: primary (azul), secondary (outline), ghost (sin fondo)
// Preset: pill (default), capsule, square

import QtQuick
import QtQuick.Controls
import qml 1.0

Button {
    id: btn

    // === Props ===
    property string variant: "primary"   // "primary" | "secondary" | "ghost" | "destructive"
    property string size: "medium"       // "small" | "medium" | "large"
    property bool pill: true

    // Icono (opcional, antes del texto)
    property string iconName: ""
    property int iconSize: Theme.iconSizeSm

    // === Look ===
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    leftPadding: paddingH
    rightPadding: paddingH
    topPadding: paddingV
    bottomPadding: paddingV
    font: Theme.font(textSize, Theme.weightBold, -0.224)

    property int textSize: size === "small" ? Theme.fontSizeCaption
                          : size === "large" ? Theme.fontSizeButtonLarge
                          : Theme.fontSizeBodyStrong
    property int paddingH: size === "small" ? Theme.spacingMd
                        : size === "large" ? Theme.spacingLg
                        : 22
    property int paddingV: size === "small" ? Theme.spacingXs
                        : size === "large" ? Theme.spacingMd
                        : 11

    contentItem: Row {
        spacing: Theme.spacingXs
        anchors.centerIn: parent
        Octicon {
            visible: btn.iconName !== ""
            name: btn.iconName
            size: btn.iconSize
            color: btn.textColor
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: btn.text
            color: btn.textColor
            font: btn.font
            anchors.verticalCenter: parent.verticalCenter
        }
    }

    background: Rectangle {
        radius: btn.pill ? Theme.radiusPill : Theme.radiusMd
        color: {
            if (!btn.enabled) return Qt.darker(btn.bgColor, 1.4)
            if (btn.variant === "primary") {
                if (btn.pressed) return Theme.primaryPressed
                if (btn.hovered) return Theme.primaryFocus
                return Theme.primary
            }
            if (btn.variant === "destructive") {
                if (btn.pressed) return Qt.darker(Theme.error, 1.1)
                if (btn.hovered) return Theme.error
                return Theme.error
            }
            if (btn.variant === "secondary") {
                if (btn.pressed) return Theme.surfacePressed
                if (btn.hovered) return Theme.surfaceHover
                return Theme.surface
            }
            // ghost
            if (btn.pressed) return Theme.surfacePressed
            if (btn.hovered) return Theme.surfaceHover
            return "transparent"
        }
        border.width: btn.variant === "secondary" ? 1 : 0
        border.color: Theme.hairline

        Behavior on color {
            ColorAnimation { duration: Theme.animFast }
        }
    }

    // === Colores computados ===
    property color textColor: {
        if (!btn.enabled) return Theme.inkMuted
        if (btn.variant === "primary" || btn.variant === "destructive") return Theme.inkOnPrimary
        if (btn.variant === "secondary") return btn.pressed ? Theme.ink : Theme.primary
        return btn.pressed ? Theme.ink : Theme.primary
    }
    property color bgColor: Theme.primary

    // === Animaciones press ===
    scale: btn.pressed ? 0.97 : 1.0
    Behavior on scale {
        NumberAnimation { duration: Theme.animFast; easing.type: Easing.OutCubic }
    }
}
