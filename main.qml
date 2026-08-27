import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtQuick.Controls as T
import QtQuick.Controls

import pyobjects
import Components

Window {
    id: mainWindow
    width: 900
    height: 650
    minimumWidth: 800
    minimumHeight: 550
    visible: true
    title: "Shobdo Calok"
    
    flags: Qt.Window | Qt.FramelessWindowHint
    color: AppTheme.background

    MyAppHeaderViewModel {
        id: _headerViewModel
        onWindowDragRequested: mainWindow.startSystemMove()
        onMinimizeAppRequested: mainWindow.hide()
        onToggleMaximizeAppRequested: {
            if (_private.maximized) {
                mainWindow.showNormal();
            } else {
                mainWindow.showMaximized();
            }
        }
        onCloseAppRequested: mainWindow.hide()
    }

    // ── Global ToolTip Style ──────────────────────────────────────────────
    T.ToolTip {
        id: globalToolTip
        parent: mainWindow.contentItem
        delay: 500
        timeout: 3000
        
        contentItem: Text {
            text: globalToolTip.text || ""
            font.family: "Inter"
            font.pixelSize: 11
            color: AppTheme.textPrimary
        }
        
        background: Rectangle {
            color: AppTheme.surface
            border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"
            border.width: 1
            radius: 6
        }
    }

    QtObject {
        id: _private
        readonly property bool maximized: mainWindow.visibility === Window.Maximized
        readonly property bool fullscreen: mainWindow.visibility === Window.FullScreen
    }

    // The actual window frame
    Rectangle {
        id: appFrame
        anchors.fill: parent
        color: "transparent"
        clip: true
        
        opacity: 0
        Behavior on opacity {
            NumberAnimation { duration: 500; easing.type: Easing.InOutQuad }
        }
        Component.onCompleted: appFrame.opacity = 1
        
        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            TitleBar {
                id: titleBar
                Layout.preferredHeight: 48
                Layout.fillWidth: true
                viewModel: _headerViewModel
                z: 10
                onShowTutorial: sidebar.activeIndex = 0
                onShowAbout: sidebar.activeIndex = 2
                onViewAll: sidebar.activeIndex = 1, libraryPanel.toggleAll()
                onImportLibrary: sidebar.activeIndex = 1, snippetViewModel.importSnippets()
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0
                
                Rectangle {
                    Layout.preferredWidth: 64
                    Layout.fillHeight: true
                    color: AppTheme.sidebarBg
                    
                    Sidebar {
                        id: sidebar
                        objectName: "sidebar"
                        anchors.fill: parent
                    }
                }
                
                // Added Item container for StackLayout to ensure better clipping control
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    
                    StackLayout {
                        id: contentStack
                        anchors.fill: parent
                        anchors.rightMargin: 16 // Prevent edge clipping
                        anchors.bottomMargin: 16 // Prevent edge clipping
                        currentIndex: sidebar.activeIndex
                        
                        // Page 0: Tutorial
                        TutorialPage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                        }
                        
                        // Page 1: Editor & Library
                        RowLayout {
                            spacing: 0
                            
                            Rectangle {
                                Layout.preferredWidth: 260
                                Layout.fillHeight: true
                                Layout.topMargin: 8
                                Layout.leftMargin: 8
                                Layout.bottomMargin: 8
                                color: AppTheme.libraryBg
                                radius: 20
                                clip: true
                                
                                LibraryPanel {
                                    id: libraryPanel
                                    anchors.fill: parent
                                    anchors.margins: 16
                                }
                            }
                            
                            Rectangle {
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                Layout.margins: 8
                                color: AppTheme.editorBg
                                radius: 24
                                clip: true
                                
                                EditorPanel {
                                    anchors.fill: parent
                                    anchors.margins: 8
                                }
                            }
                        }
                        
                        // Page 2: About
                        AboutPage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                        }
                        
                        // Page 3: Backup
                        BackupPage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                        }
                        
                        // Page 4: Settings
                        SettingsPage {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                        }
                    }
                }
            }
        }
    }

    // On startup: seed AppTheme from the persisted theme preference
    Component.onCompleted: {
        if (snippetViewModel) {
            AppTheme.isDark = snippetViewModel.isDark
        }
    }

    // Keep AppTheme in sync with the viewmodel (which owns themeMode + isDark)
    Binding {
        target: AppTheme
        property: "isDark"
        value: snippetViewModel.isDark
    }

    Connections {
        target: snippetViewModel
        function onThemeChanged(dark) { AppTheme.isDark = dark }
    }
}
