// Theme.qml - Sistema de diseno Apple (Cupertino) adaptado para KDE Assistant v2
// Los tokens aqui definidos son la fuente unica de verdad para toda la UI.
// Ver apple-DESIGN.md para documentacion completa.

pragma Singleton
import QtQuick

QtObject {
    id: theme

    // ===== COLOR PALETTE (Apple Design, auto dark/light) =====
    // Sera controlado desde Rust segun el tema del sistema (Breeze Dark/Light)

    property bool isDark: true   // Default dark, Rust lo actualizara

    // Canvas (fondo ventana)
    readonly property color canvas: isDark ? "#1d1d1f" : "#f5f5f7"
    readonly property color canvasParchment: isDark ? "#1d1d1f" : "#f5f5f7"

    // Surface (cards, burbujas asistente)
    readonly property color surface: isDark ? "#2a2a2c" : "#ffffff"
    readonly property color surfaceHover: isDark ? "#3a3a3c" : "#e8e9eb"
    readonly property color surfacePearl: isDark ? "#272729" : "#fafafc"

    // Primary (Action Blue - unico color interactivo)
    readonly property color primary: isDark ? "#2997ff" : "#0066cc"
    readonly property color primaryFocus: isDark ? "#0071e3" : "#0071e3"
    readonly property color onPrimary: "#ffffff"

    // Ink (texto)
    readonly property color ink: isDark ? "#ffffff" : "#1d1d1f"
    readonly property color inkMuted: isDark ? "#7a7a7a" : "#6d6f72"
    readonly property color inkMuted80: isDark ? "#cccccc" : "#333333"
    readonly property color onDark: "#ffffff"

    // Hairlines y dividers
    readonly property color hairline: isDark ? "#3a3a3c" : "#e0e0e0"
    readonly property color dividerSoft: isDark ? "#272729" : "#f0f0f0"

    // Surface chip (boton mic circular)
    readonly property color surfaceChip: isDark ? "#3a3a3c" : "#d2d2d7"

    // Estado
    readonly property color error: isDark ? "#ff453a" : "#ff3b30"
    readonly property color success: isDark ? "#30d158" : "#34c759"
    readonly property color warn: isDark ? "#ff9f0a" : "#ff9500"

    // Voice Orb gradient
    readonly property color orbStart: "#2997ff"
    readonly property color orbMid: "#5e5ce6"
    readonly property color orbEnd: "#bf5af2"

    // ===== TYPOGRAPHY =====
    readonly property string fontFamily: "Inter, Noto Sans, system-ui, sans-serif"

    readonly property int fontSizeHero: 28
    readonly property int fontSizeTagline: 21
    readonly property int fontSizeBodyStrong: 17
    readonly property int fontSizeBody: 15
    readonly property int fontSizeBodySmall: 13
    readonly property int fontSizeCaption: 13
    readonly property int fontSizeFinePrint: 12
    readonly property int fontSizeMicro: 11
    readonly property int fontSizeButtonLarge: 18
    readonly property int fontSizeButtonUtility: 14
    readonly property int fontSizeNavLink: 12

    readonly property real lhHero: 1.14
    readonly property real lhBody: 1.47
    readonly property real lhBodySmall: 1.43
    readonly property real lsHero: -0.28
    readonly property real lsBody: -0.224
    readonly property real lsBodySmall: -0.12

    readonly property int weightBold: Font.DemiBold
    readonly property int weightNormal: Font.Normal

    // ===== SPACING =====
    readonly property int spacingXxs: 4
    readonly property int spacingXs: 8
    readonly property int spacingSm: 12
    readonly property int spacingMd: 17
    readonly property int spacingLg: 24
    readonly property int spacingXl: 32
    readonly property int spacingXxl: 48
    readonly property int spacingSection: 80

    // ===== RADII =====
    readonly property int radiusXs: 5
    readonly property int radiusSm: 8
    readonly property int radiusMd: 11
    readonly property int radiusLg: 18
    readonly property int radiusPill: 9999
    readonly property int radiusFull: 9999

    // ===== LAYOUT =====
    readonly property int windowMinWidth: 360
    readonly property int windowMinHeight: 500
    readonly property int windowMaxWidth: 800
    readonly property int windowMaxHeight: 900
    readonly property int windowDefaultWidth: 420
    readonly property int windowDefaultHeight: 680

    readonly property int drawerWidth: 220
    readonly property int maxContentWidth: 720
    readonly property int inputBarHeight: 56
    readonly property int buttonIconSize: 44
    readonly property int iconSizeSm: 16
    readonly property int iconSizeMd: 20
    readonly property int iconSizeLg: 24
    readonly property int iconSizeHero: 64

    // ===== ANIMATIONS =====
    readonly property int animFast: 100
    readonly property int animNormal: 200
    readonly property int animSlow: 400

    // ===== HELPERS =====

    function textRole(role, size, weight) {
        // Helper para construir font role segun tamano
        return {
            "family": fontFamily,
            "pixelSize": size,
            "weight": weight !== undefined ? weight : weightNormal,
        }
    }
}
