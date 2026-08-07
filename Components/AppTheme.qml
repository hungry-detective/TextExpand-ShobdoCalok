pragma Singleton
import QtQuick

QtObject {
    id: theme

    property bool isDark: true

    // Colors matching Projectline aesthetic
    property color background: isDark ? "#111318" : "#fefefe"
    property color surface: isDark ? "#24282e" : "#f8f9fa" // Better contrast
    property color surfaceTranslucent: isDark ? "#601a1d21" : "#60ffffff" 
    property color navigationActiveBg: isDark ? "#1a1d23" : "#f1f5f9" // Tiered Library Bg
    property color hoverBg: isDark ? "#2a2d35" : "#f1f5f9" // Unified highlight color
    
    // Tiered background steps
    property color sidebarBg: isDark ? "#111318" : "transparent"
    property color libraryBg: isDark ? "#1a1d23" : "#f1f5f9"
    property color editorBg: isDark ? "#1a1d23" : "#ffffff"
    // Borders (setting to transparent for minimal look)
    property color border: "transparent"
    property color borderLight: "transparent"

    // Text
    property color textPrimary: isDark ? "#f1f5f9" : "#0f172a"
    property color textSecondary: isDark ? "#94a3b8" : "#64748b"

    // Brand/Accent (Projectline Purple)
    property color primary: "#7c3aed" // Violet 600
    property color primaryHover: "#8b5cf6" // Violet 500
    property color primaryLight: isDark ? "#2d1b4e" : "#ede9fe"

    // States
    property color danger: "#ef4444"
    property color dangerHover: "#dc2626"
    property color dangerLight: isDark ? "#451a1e" : "#fef2f2"
    
    // Font
    property font mainFont
    
    Component.onCompleted: {
        mainFont.family = "Inter" // Fallback to system sans-serif if not installed, but try Inter
        mainFont.pixelSize = 12
    }
}
