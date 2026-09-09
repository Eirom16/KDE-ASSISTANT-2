// WelcomeScreen.qml - Pantalla de bienvenida
// Logo, titulo, sugerencia, pills de sugerencias

import QtQuick
import qml 1.0

Item {
    id: root

    signal suggestionClicked(string text)

    Column {
        anchors.centerIn: parent
        spacing: Theme.spacingMd
        width: Math.min(parent.width - Theme.spacingLg * 2, 380)

        // Logo + orbe en gradient
        Rectangle {
            width: Theme.iconSizeHero
            height: width
            radius: width / 2
            gradient: Gradient {
                GradientStop { position: 0.0; color: Theme.orbStart }
                GradientStop { position: 0.5; color: Theme.orbMid }
                GradientStop { position: 1.0; color: Theme.orbEnd }
            }
            anchors.horizontalCenter: parent.horizontalCenter

            Octicon {
                anchors.centerIn: parent
                name: "hubot-16"
                size: 32
                color: Theme.inkOnPrimary
            }

            // Glow
            Rectangle {
                anchors.fill: parent
                anchors.margins: -12
                z: -1
                color: "transparent"
                radius: parent.radius + 12
            }
        }

        // Title
        Text {
            text: qsTr("KDE Assistant")
            font: Theme.font(Theme.fontSizeHero, Theme.weightBold, Theme.lsHero)
            color: Theme.ink
            horizontalAlignment: Text.AlignHCenter
            anchors.horizontalCenter: parent.horizontalCenter
        }

        // Subtitle
        Text {
            text: qsTr("Escribe un mensaje para comenzar.\nPuedo ayudarte con preguntas, tareas\ndel sistema, y mas.")
            font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
            color: Theme.inkMuted
            lineHeight: Theme.lhBody
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            width: parent.width
        }

        // Sugerencias (pills)
        Item {
            width: parent.width
            height: suggestFlow.implicitHeight

            Flow {
                id: suggestFlow
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.spacingXs
                width: parent.width

                PillButton {
                    text: qsTr("Abre Firefox")
                    iconName: "rocket-16"
                    onClicked: root.suggestionClicked("Abre Firefox")
                }
                PillButton {
                    text: qsTr("Busca el clima")
                    iconName: "search-16"
                    onClicked: root.suggestionClicked("Busca el clima de hoy")
                }
                PillButton {
                    text: qsTr("Crea un archivo")
                    iconName: "file-added-16"
                    onClicked: root.suggestionClicked("Crea un archivo notas.md en ~/Documentos con la lista del super")
                }
                PillButton {
                    text: qsTr("Historia de Rust")
                    iconName: "light-bulb-16"
                    onClicked: root.suggestionClicked("Cuentame la historia del lenguaje Rust")
                }
            }
        }
    }
}
