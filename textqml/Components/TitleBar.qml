import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic
import QtQuick.Layouts

import pyobjects

import "."

Item {
    id: root

    required property MyAppHeaderViewModel viewModel
    
    height: 48

    signal showTutorial()
    signal showAbout()
    signal viewAll()
    signal importLibrary()

    // Background Drag Area (So it doesn't block children like TextInput)
    Item {
        anchors.fill: parent
        z: -1
        
        // This MouseArea will trigger the drag but allow children to capture events first
        MouseArea {
            anchors.fill: parent
            onPressed: root.viewModel.requestWindowDrag()
            onDoubleClicked: root.viewModel.requestToggleMaximize()
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 12
        anchors.rightMargin: 0
        spacing: 8

        // 1. App Icon
        Image {
            source: "../app-icon.svg"
            Layout.preferredWidth: 28
            Layout.preferredHeight: 28
            sourceSize.width: 28
            sourceSize.height: 28
            fillMode: Image.PreserveAspectFit
        }
        // 2. Navigation Menus
        MenuBar {
            id: menuBar
            Layout.alignment: Qt.AlignVCenter
            background: null
            
            delegate: MenuBarItem {
                id: menuBarItem
                contentItem: Text {
                    text: menuBarItem.text
                    font: menuBarItem.font
                    opacity: enabled ? 1.0 : 0.3
                    color: menuBarItem.highlighted ? AppTheme.primary : AppTheme.textPrimary
                    horizontalAlignment: Qt.AlignLeft
                    verticalAlignment: Qt.AlignVCenter
                    elide: Text.ElideRight
                }
                background: Rectangle {
                    implicitWidth: contentItem.implicitWidth + 20
                    implicitHeight: 32
                    opacity: enabled ? 1 : 0.3
                    color: menuBarItem.highlighted ? AppTheme.hoverBg : "transparent"
                    radius: 4
                }
            }

            AppMenu {
                title: qsTr("File")
                Action { text: qsTr("New Snippet"); shortcut: "CTRL+N" }
                Action { text: qsTr("Save All"); shortcut: "CTRL+S" }
                MenuSeparator {}
                Action { text: qsTr("Exit"); onTriggered: root.viewModel.requestClose() }
            }
            AppMenu {
                title: qsTr("Tasks")
                Action { text: qsTr("View All"); onTriggered: root.viewAll() }
                Action { text: qsTr("Import..."); onTriggered: root.importLibrary() }
            }
            AppMenu {
                title: qsTr("Tools")
                Action { text: qsTr("Backup") }
                Action { text: qsTr("Analyze") }
            }
            AppMenu {
                title: qsTr("Help")
                Action { text: qsTr("Docs"); onTriggered: root.showTutorial() }
                Action { text: qsTr("About"); onTriggered: root.showAbout() }
            }
        }


        Item { Layout.fillWidth: true } // Spacer

        // Theme Toggle Button
        Rectangle {
            id: themeToggleBtn
            width: 32; height: 32; radius: 8
            color: themeHover.hovered ? AppTheme.hoverBg : "transparent"
            Layout.alignment: Qt.AlignVCenter
            Layout.rightMargin: 4

            HoverHandler { id: themeHover }
            ToolTip {
                id: themeToolTip
                visible: themeHover.hovered
                text: AppTheme.isDark ? "Switch to Light Mode" : "Switch to Dark Mode"
                delay: 400
                contentItem: Text { text: themeToolTip.text; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textPrimary }
                background: Rectangle { 
                    implicitWidth: 120; implicitHeight: 32
                    color: AppTheme.surface; radius: 6; border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"; border.width: 1 
                }
            }

            Text {
                anchors.centerIn: parent
                text: AppTheme.isDark ? "light_mode" : "dark_mode"
                font.family: "Material Symbols Outlined"
                font.pixelSize: 18
                color: AppTheme.isDark ? "#fbbf24" : "#7c3aed"

                Behavior on color { ColorAnimation { duration: 200 } }
            }

            TapHandler { onTapped: snippetViewModel.setThemeMode(AppTheme.isDark ? "light" : "dark") }

            Behavior on color { ColorAnimation { duration: 150 } }
        }

        // 5. Window Control Buttons
        Row {
            Layout.fillHeight: true
            spacing: 0
            
            ToolButton {
                id: _minimizeButton
                height: 48; width: 46
                icon.source: "../icons/minimize_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg"
                icon.width: 24
                icon.height: 24
                icon.color: AppTheme.textPrimary
                background: Rectangle { color: _minimizeButton.hovered ? AppTheme.hoverBg : "transparent" }
                onClicked: root.viewModel.requestMinimize()
            }

            ToolButton {
                id: _maximizeButton
                height: 48; width: 46
                icon.source: (root.viewModel && root.viewModel.isMaximized) ? 
                    "../icons/close_fullscreen_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg" : 
                    "../icons/open_in_full_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg"
                icon.width: 24
                icon.height: 24
                icon.color: AppTheme.textPrimary
                background: Rectangle { color: _maximizeButton.hovered ? AppTheme.hoverBg : "transparent" }
                onClicked: root.viewModel.requestToggleMaximize()
            }

            ToolButton {
                id: _closeButton
                height: 48; width: 46
                icon.source: "../icons/close_24dp_1F1F1F_FILL0_wght400_GRAD0_opsz24.svg"
                icon.width: 24
                icon.height: 24
                icon.color: _closeButton.hovered ? "white" : AppTheme.textPrimary
                background: Rectangle { color: _closeButton.hovered ? (AppTheme.isDark ? "#c42c1e" : "#e81123") : "transparent" }
                onClicked: root.viewModel.requestClose()
            }
        }
    }

    Label {
        text: "Shobdo Calok"
        anchors.centerIn: parent
        font.family: "Inter"
        font.pixelSize: 12
        font.bold: true
        color: AppTheme.textPrimary
        visible: root.width > 500
    }
}
