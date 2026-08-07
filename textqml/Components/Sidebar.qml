import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    width: 64 // w-16
    color: AppTheme.background
    property alias activeIndex: mainLayout.activeIndex
    
    ColumnLayout {
        id: mainLayout
        anchors.fill: parent
        spacing: 0
        property int activeIndex: 1
        
        
        // Nav items
        ColumnLayout {
            Layout.fillWidth: true
            Layout.topMargin: 16
            Layout.leftMargin: 10
            Layout.rightMargin: 10
            spacing: 8
            
            SidebarItem {
                iconText: "school"
                iconColor: "#a3e635" // lime-400
                tooltipText: "Tutorial"
                isActive: mainLayout.activeIndex === 0
                onClicked: mainLayout.activeIndex = 0
            }
            SidebarItem {
                iconText: "code_blocks"
                tooltipText: "Editor & Library"
                isActive: mainLayout.activeIndex === 1
                onClicked: mainLayout.activeIndex = 1
            }
            SidebarItem {
                iconText: "info"
                iconColor: "#38bdf8" // sky-400
                tooltipText: "About"
                isActive: mainLayout.activeIndex === 2
                onClicked: mainLayout.activeIndex = 2
            }
            SidebarItem {
                iconText: "backup"
                tooltipText: "Backup"
                isActive: mainLayout.activeIndex === 3
                onClicked: mainLayout.activeIndex = 3
            }
            
            Item { Layout.fillHeight: true } // spacer
            
            SidebarItem {
                iconText: "settings"
                tooltipText: "Settings"
                Layout.bottomMargin: 16
                isActive: mainLayout.activeIndex === 4
                onClicked: mainLayout.activeIndex = 4
            }
        }
    }
    
    component SidebarItem: Rectangle {
        id: itemRoot
        property string iconText
        property string tooltipText: ""
        property color iconColor: isActive ? AppTheme.primary : AppTheme.textSecondary
        property bool isActive: false
        signal clicked()
        
        Layout.fillWidth: true
        Layout.preferredHeight: 44
        radius: 12
        color: isActive ? AppTheme.navigationActiveBg : 
               (itemHover.hovered ? AppTheme.hoverBg : "transparent")
               
        // Projectline Vertical Indicator
        Rectangle {
            width: 3
            height: 20
            radius: 2
            color: AppTheme.primary
            anchors.left: parent.left
            anchors.leftMargin: -10 // Offset back into the margin area
            anchors.verticalCenter: parent.verticalCenter
            visible: isActive
        }

        HoverHandler { id: itemHover }
        TapHandler { onTapped: itemRoot.clicked() }
        
        Text {
            anchors.centerIn: parent
            text: itemRoot.iconText
            font.family: "Material Symbols Outlined"
            font.pixelSize: 20
            color: itemRoot.isActive ? AppTheme.primary : AppTheme.textSecondary
        }

        ToolTip {
            parent: itemRoot
            visible: itemHover.hovered && itemRoot.tooltipText !== ""
            text: itemRoot.tooltipText
            delay: 450
            x: itemRoot.width + 8
            y: (itemRoot.height - height) / 2
            contentItem: Text { text: itemRoot.tooltipText; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textPrimary }
            background: Rectangle {
                implicitWidth: 120; implicitHeight: 30
                color: AppTheme.surface; radius: 6
                border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"; border.width: 1
            }
        }
    }
}
