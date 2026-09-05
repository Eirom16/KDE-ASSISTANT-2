// ImageCard.qml - Tarjeta para imagenes en chat
// Soporta URLs remotas (http/https) y paths locales (file://)
// Efecto hover con scale sutil, click para abrir en visor

import QtQuick
import QtQuick.Controls
import qml 1.0

Rectangle {
    id: root

    property string source: ""
    property int maxHeight: 320
    property int maxWidth: 480
    property string caption: ""

    signal clicked()

    width: Math.min(implicitWidth, maxWidth)
    height: Math.min(implicitHeight, maxHeight + (caption ? Theme.fontSizeCaption + Theme.spacingXs : 0))
    radius: Theme.radiusLg
    color: Theme.surface
    border.width: 1
    border.color: Theme.hairline

    // Sombra elevada
    Rectangle {
        anchors.fill: parent
        anchors.margins: -2
        z: -1
        color: Theme.shadowCard
        opacity: 0.4
        radius: parent.radius + 2
    }

    // Imagen
    Image {
        id: img
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 4
        height: Math.min(sourceSize.height, root.maxHeight - (root.caption ? Theme.fontSizeCaption + Theme.spacingXs : 0) - 8)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        cache: true
        smooth: true
        source: root.source.startsWith("http") || root.source.startsWith("file") || root.source.startsWith("/")
               ? root.source : "file://" + root.source

        // Loading placeholder
        Rectangle {
            anchors.fill: parent
            color: Theme.surfacePearl
            radius: Theme.radiusMd
            visible: img.status === Image.Loading
            Octicon {
                anchors.centerIn: parent
                name: "image-16"
                size: 32
                color: Theme.inkMuted
                opacity: 0.5
            }
        }

        // Error placeholder
        Rectangle {
            anchors.fill: parent
            color: Theme.errorTint
            radius: Theme.radiusMd
            visible: img.status === Image.Error
            Text {
                anchors.centerIn: parent
                text: qsTr("No se pudo cargar la imagen")
                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                color: Theme.error
            }
        }
    }

    // Caption
    Text {
        visible: root.caption !== ""
        anchors.top: img.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.spacingMd
        anchors.rightMargin: Theme.spacingMd
        anchors.topMargin: Theme.spacingXs
        text: root.caption
        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
        color: Theme.inkMuted
        wrapMode: Text.Wrap
        maximumLineCount: 3
        elide: Text.ElideRight
    }

    // Click handler
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        hoverEnabled: true
        onClicked: root.clicked()
    }

    // Highlight on hover
    opacity: hoverArea.containsMouse ? 0.95 : 1.0
    Behavior on opacity {
        NumberAnimation { duration: Theme.animFast }
    }

    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
    }
}
