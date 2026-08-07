import QtQuick
import QtQuick.Controls

ScrollBar {
    id: control
    
    property bool isHovered: hoverHandler.hovered
    
    width: isHovered ? 8 : 4
    active: true
    orientation: Qt.Vertical
    
    Behavior on width {
        NumberAnimation { duration: 150 }
    }

    contentItem: Rectangle {
        implicitWidth: 4
        implicitHeight: 100
        radius: width / 2
        color: control.isHovered ? AppTheme.primary : AppTheme.textSecondary
        opacity: control.isHovered ? 0.8 : 0.4
        
        Behavior on color { ColorAnimation { duration: 150 } }
        Behavior on opacity { NumberAnimation { duration: 150 } }
    }

    background: Rectangle {
        implicitWidth: 4
        implicitHeight: 100
        color: "transparent"
    }
    
    HoverHandler {
        id: hoverHandler
    }
}
