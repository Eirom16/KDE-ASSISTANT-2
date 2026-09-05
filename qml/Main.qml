// Main.qml - Ventana principal de KDE Assistant v2
// Fase 1: estructura minima. La integracion real con Rust via cxx-qt
// se completara en Fase 3.

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import QtQuick.Layouts

ApplicationWindow {
    id: root

    title: "KDE Assistant v2"
    width: Theme.windowDefaultWidth
    height: Theme.windowDefaultHeight
    minimumWidth: Theme.windowMinWidth
    minimumHeight: Theme.windowMinHeight
    maximumWidth: Theme.windowMaxWidth
    maximumHeight: Theme.windowMaxHeight

    // Tema (sera controlado desde Rust en Fase 3)
    color: Theme.canvas

    // Flags para ventana flotante translucida (Fase 6)
    // flags: Qt.Window | Qt.WindowStaysOnTopHint

    visible: true

    // Welcome screen placeholder
    Item {
        anchors.fill: parent
        anchors.margins: Theme.spacingLg

        ColumnLayout {
            anchors.centerIn: parent
            spacing: Theme.spacingMd

            // Logo
            Octicon {
                name: "hubot-16"
                size: Theme.iconSizeHero
                color: Theme.primary
                Layout.alignment: Qt.AlignHCenter
            }

            // Title
            Text {
                text: "KDE Assistant"
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSizeHero
                font.weight: Theme.weightBold
                color: Theme.ink
                lineHeight: Theme.lhHero
                Layout.alignment: Qt.AlignHCenter
            }

            // Subtitle
            Text {
                text: "Fase 1: Scaffold completado.\nEl backend Rust esta listo.\nLa UI se conectara en Fase 3."
                font.family: Theme.fontFamily
                font.pixelSize: Theme.fontSizeBody
                color: Theme.inkMuted
                lineHeight: Theme.lhBody
                horizontalAlignment: Text.AlignHCenter
                Layout.alignment: Qt.AlignHCenter
            }

            // Status pill
            Rectangle {
                Layout.alignment: Qt.AlignHCenter
                color: Theme.surfacePearl
                radius: Theme.radiusPill
                implicitHeight: 36
                implicitWidth: statusRow.implicitWidth + Theme.spacingLg

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: Theme.spacingXs

                    Octicon {
                        name: "check-16"
                        size: Theme.iconSizeSm
                        color: Theme.success
                    }
                    Text {
                        text: "Backend online"
                        font.family: Theme.fontFamily
                        font.pixelSize: Theme.fontSizeCaption
                        color: Theme.ink
                    }
                }
            }
        }
    }
}
