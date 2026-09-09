// ImageCard.qml - Tarjeta para imagenes en chat
// Soporta URLs remotas (http/https) y paths locales (file://)
// F0-8: tamaño estable (no colapsa a 0), BusyIndicator, clip anti-panorámica.

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

    // Ancho estable: el padre (MessageBubble) limita con Layout.maximumWidth;
    // aquí ocupar todo el ancho disponible hasta maxWidth.
    width: Math.min(parent ? parent.width : maxWidth, maxWidth)
    implicitWidth: 320
    // Alto = imagen (fija 200 salvo caption) + caption. Cuando la imagen
    // carga, se ajusta por aspect sin empujar el layout (clip).
    height: imgBoxHeight + (captionText.visible ? captionText.implicitHeight + Theme.spacingXs : 0) + 8
    property int imgBoxHeight: {
        if (img.status === Image.Ready && img.sourceSize.height > 0) {
            var captionReserve = root.caption !== "" ? Theme.fontSizeCaption + Theme.spacingXs : 0
            return Math.min(img.sourceSize.height, root.maxHeight - captionReserve - 8)
        }
        return 200
    }
    radius: Theme.radiusLg
    color: Theme.surface
    border.width: 1
    border.color: Theme.hairline
    clip: true

    // Sombra elevada
    Rectangle {
        anchors.fill: parent
        anchors.margins: -2
        z: -1
        color: Theme.shadowCard
        opacity: 0.4
        radius: parent.radius + 2
    }

    // Caja de imagen con clip (panorámicas no empujan el layout)
    Item {
        id: imgBox
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 4
        height: root.imgBoxHeight
        clip: true

        Image {
            id: img
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            smooth: true
            source: root.source.startsWith("http") || root.source.startsWith("file") || root.source.startsWith("/")
                   ? root.source : "file://" + root.source
        }

        // Loading: spinner + icono
        Item {
            anchors.fill: parent
            visible: img.status === Image.Loading
            Rectangle {
                anchors.fill: parent
                color: Theme.surfacePearl
                radius: Theme.radiusMd
            }
            BusyIndicator {
                anchors.centerIn: parent
                running: parent.visible
                width: 32
                height: 32
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
                width: parent.width - 32
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: qsTr("No se pudo cargar la imagen")
                font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
                color: Theme.error
            }
        }
    }

    // Caption
    Text {
        id: captionText
        visible: root.caption !== ""
        anchors.top: imgBox.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: Theme.spacingMd
        anchors.rightMargin: Theme.spacingMd
        anchors.topMargin: Theme.spacingXs
        text: root.caption
        font: Theme.font(Theme.fontSizeCaption, Theme.weightNormal, 0)
        color: Theme.inkMuted
        wrapMode: Text.WrapAnywhere
        maximumLineCount: 3
        elide: Text.ElideRight
    }

    // Click + hover handler
    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }

    // Highlight on hover
    opacity: hoverArea.containsMouse ? 0.95 : 1.0
    Behavior on opacity {
        NumberAnimation { duration: Theme.animFast }
    }
}
