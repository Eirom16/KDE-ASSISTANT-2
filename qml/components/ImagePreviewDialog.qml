// ImagePreviewDialog.qml - Dialog modal para ver imagen a tamano completo
// Cierra con click fuera, tecla Escape o boton X

import QtQuick
import QtQuick.Controls
import qml 1.0
import "."

Rectangle {
    id: root

    property string source: ""
    property string caption: ""
    property bool open_: false

    signal closed()

    visible: open_
    color: Theme.overlayImagePreview
    z: 999

    function show(url, cap) {
        root.source = url
        root.caption = cap || ""
        root.open_ = true
    }
    function hide() {
        root.open_ = false
        root.closed()
    }

    // === Backdrop ===
    MouseArea {
        anchors.fill: parent
        onClicked: root.hide()
    }

    // === Top bar (close button) ===
    Item {
        id: topBar
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Theme.spacingMd
        height: 44

        IconButton {
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            iconName: "x-16"
            iconSize: Theme.iconSizeMd
            buttonSize: 44
            backgroundColor: "transparent"
            iconColor: Theme.onDark
            onClicked: root.hide()
        }
    }

    // === Imagen centrada ===
    Flickable {
        id: flick
        anchors.fill: parent
        anchors.topMargin: 60
        anchors.bottomMargin: captionText.visible ? 80 : 24
        contentWidth: img.width
        contentHeight: img.height
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: img.width > width || img.height > height

        Image {
            id: img
            // Tamano encajado en el viewport preservando aspecto.
            // Se calcula desde flick (estable) y sourceSize, sin anchors
            // que puedan realimentarse con contentWidth/contentHeight.
            property real maxW: Math.max(0, flick.width - 40)
            property real maxH: Math.max(0, flick.height - 40)
            property real fitScale: {
                if (sourceSize.width <= 0 || sourceSize.height <= 0) return 0
                return Math.min(1, Math.min(maxW / sourceSize.width, maxH / sourceSize.height))
            }
            width: sourceSize.width > 0 ? sourceSize.width * fitScale : 0
            height: sourceSize.height > 0 ? sourceSize.height * fitScale : 0
            x: (flick.width - width) / 2
            y: (flick.height - height) / 2
            fillMode: Image.PreserveAspectFit
            asynchronous: true
            cache: true
            smooth: true
            source: root.source
        }
    }

    // === Caption ===
    Text {
        id: captionText
        visible: root.caption !== ""
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottomMargin: Theme.spacingLg
        anchors.leftMargin: Theme.spacingLg
        anchors.rightMargin: Theme.spacingLg
        text: root.caption
        font: Theme.font(Theme.fontSizeBody, Theme.weightNormal, Theme.lsBody)
        color: Theme.onDark
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        maximumLineCount: 3
        elide: Text.ElideRight
    }

    // === Keys ===
    Keys.onEscapePressed: root.hide()
    focus: open_
}
