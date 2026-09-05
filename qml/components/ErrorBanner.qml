// ErrorBanner.qml - Banner de error con boton de reintentar
// Usado para mostrar errores de conexion, tool call fallido, etc.

import QtQuick
import qml 1.0
import "."

Rectangle {
    id: root

    property string message: ""
    property string detail: ""
    property bool retryable: true

    signal retryClicked()
    signal dismissed()

    implicitHeight: column.implicitHeight + Theme.spacingMd * 2
    radius: Theme.radiusLg
    color: Theme.errorTint
    border.width: 1
    border.color: Qt.rgba(Theme.error.r, Theme.error.g, Theme.error.b, 0.3)

    Column {
        id: column
        anchors.fill: parent
        anchors.margins: Theme.spacingMd
        spacing: Theme.spacingXs

        Row {
            spacing: Theme.spacingXs
            width: parent.width

            Octicon {
                name: "alert-16"
                size: Theme.iconSizeSm
                color: Theme.error
                anchors.verticalCenter: parent.verticalCenter
            }

            Text {
                text: root.message || qsTr("Error")
                font: Theme.font(Theme.fontSizeCaption, Theme.weightBold, -0.05)
                color: Theme.error
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        Text {
            visible: root.detail !== ""
            text: root.detail
            font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, -0.05)
            color: Theme.ink
            wrapMode: Text.Wrap
            width: parent.width
        }

        Row {
            visible: root.retryable
            spacing: Theme.spacingXs
            Layout.alignment: Qt.AlignRight

            PillButton {
                text: qsTr("Reintentar")
                iconName: "sync-16"
                onClicked: root.retryClicked()
            }
            PillButton {
                text: qsTr("Cerrar")
                iconName: "x-16"
                onClicked: root.dismissed()
            }
        }
    }
}
