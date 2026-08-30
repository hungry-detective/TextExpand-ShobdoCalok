import QtQuick
import QtQuick.Layouts
import QtQuick.Controls

Item {
    id: settingsRoot

    // ── Background subtle decoration ──────────────────────────────────────
    Rectangle {
        width: 280; height: 280; radius: 140
        color: AppTheme.primary; opacity: 0.03
        anchors { right: parent.right; bottom: parent.bottom }
        anchors.rightMargin: -60; anchors.bottomMargin: -60
    }

    Flickable {
        anchors.fill: parent
        contentHeight: mainCol.height + 64
        clip: true

        ColumnLayout {
            id: mainCol
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 32
            spacing: 0

            // ── Page Header ───────────────────────────────────────────────────
            ColumnLayout {
                spacing: 4
                Text {
                    text: "Settings"
                    font.family: "Inter"; font.pixelSize: 22; font.bold: true
                    color: AppTheme.textPrimary
                }
                Text {
                    text: "Configure your Shobdo Calok preferences"
                    font.family: "Inter"; font.pixelSize: 12
                    color: AppTheme.textSecondary
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.textSecondary; opacity: 0.15; Layout.topMargin: 20; Layout.bottomMargin: 20 }

            // ── Settings Sections ──────────────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 24

                // Section: SYSTEM
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "SYSTEM"
                        font.family: "Inter"
                        font.pixelSize: 11
                        font.bold: true
                        color: AppTheme.primary
                        Layout.leftMargin: 48 // Locked 48px offset baseline
                    }
                    SettingsRow {
                        icon: "rocket_launch"
                        title: "Run at Startup"
                        description: "Launch automatically when Windows starts"
                        activeColor: AppTheme.primary
                        checked: !!snippetViewModel && snippetViewModel.startupEnabled
                        onToggled: (value) => {
                            if (snippetViewModel) snippetViewModel.setStartup(value)
                        }
                    }
                }

                // Section: APPEARANCE
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "APPEARANCE"
                        font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary
                        Layout.leftMargin: 48 // Locked 48px offset baseline
                    }
                    Rectangle {
                        Layout.fillWidth: true; height: 60; radius: 14
                        color: AppTheme.surface
                        Rectangle {
                            width: 32; height: 32; radius: 8
                            anchors.left: parent.left; anchors.leftMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            color: AppTheme.primaryLight
                            Text {
                                anchors.centerIn: parent
                                text: AppTheme.isDark ? "dark_mode" : "light_mode"
                                font.family: "Material Symbols Outlined"; font.pixelSize: 18
                                color: AppTheme.isDark ? "#818cf8" : "#f59e0b"
                            }
                        }
                        ColumnLayout {
                            anchors.left: parent.left; anchors.leftMargin: 48
                            anchors.right: segRow.left; anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 1
                            Text {
                                text: "Theme"
                                font.family: "Inter"; font.pixelSize: 13; font.bold: true
                                color: AppTheme.textPrimary; elide: Text.ElideRight; Layout.fillWidth: true
                            }
                            Text {
                                text: snippetViewModel.themeMode === "system"
                                      ? "Follows Windows (dark/light)"
                                      : (snippetViewModel.themeMode === "dark" ? "Dark Mode is active" : "Light Mode is active")
                                font.family: "Inter"; font.pixelSize: 11
                                color: AppTheme.textSecondary; elide: Text.ElideRight; maximumLineCount: 1; Layout.fillWidth: true
                            }
                        }
                        RowLayout {
                            id: segRow
                            anchors.right: parent.right; anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 4
                            property var modes: ["system", "dark", "light"]
                            property var labels: ["System", "Dark", "Light"]
                            Repeater {
                                model: 3
                                Rectangle {
                                    width: 52; height: 28; radius: 8
                                    color: (snippetViewModel.themeMode === segRow.modes[index])
                                           ? AppTheme.primary : AppTheme.hoverBg
                                    Behavior on color { ColorAnimation { duration: 150 } }
                                    Text {
                                        anchors.centerIn: parent
                                        text: segRow.labels[index]
                                        font.family: "Inter"; font.pixelSize: 10; font.bold: true
                                        color: (snippetViewModel.themeMode === segRow.modes[index])
                                               ? "white" : AppTheme.textSecondary
                                    }
                                    TapHandler {
                                        onTapped: snippetViewModel.setThemeMode(segRow.modes[index])
                                    }
                                }
                            }
                        }
                    }
                }

                // Section: UPDATE
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "UPDATE"
                        font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary
                        Layout.leftMargin: 48 // Locked 48px offset baseline
                    }
                    SettingsRow {
                        icon: "system_update"
                        title: "Check for Updates"
                        description: updaterViewModel.updateAvailable
                                   ? "Version " + updaterViewModel.latestVersion + " is available"
                                   : "Installed: v" + updaterViewModel.currentVersion
                        descriptionColor: updaterViewModel.updateAvailable ? "#22c55e" : AppTheme.textSecondary
                        activeColor: "#22c55e"
                        checked: !!updaterViewModel && updaterViewModel.updateAvailable
                        isButton: true
                        buttonText: updaterViewModel.applying
                                   ? "Applying…"
                                   : (updaterViewModel.downloading
                                      ? Math.round(updaterViewModel.progress * 100) + "%"
                                      : (updaterViewModel.updateAvailable
                                         ? (updaterViewModel.downloadReady ? "Install" : "Download")
                                         : "Check"))
                        buttonEnabled: !updaterViewModel.downloading && !updaterViewModel.checking && !updaterViewModel.applying
                        onToggled: (value) => {
                            if (!updaterViewModel) return
                            if (updaterViewModel.updateAvailable)
                                updaterViewModel.downloadAndInstall()
                            else
                                updaterViewModel.checkForUpdate()
                        }
                    }
                    // Download / apply progress bar
                    Rectangle {
                        Layout.fillWidth: true
                        height: 20
                        visible: updaterViewModel.downloading || updaterViewModel.applying
                        radius: 10
                        color: AppTheme.isDark ? "#1a1d23" : "#f1f5f9"
                        clip: true
                        Rectangle {
                            width: parent.width * updaterViewModel.progress
                            height: parent.height
                            radius: 10
                            color: updaterViewModel.applying ? "#22c55e" : AppTheme.primary
                            Behavior on width { NumberAnimation { duration: 100 } }
                        }
                        Text {
                            anchors.centerIn: parent
                            text: updaterViewModel.applying
                                 ? "Applying update…"
                                 : (updaterViewModel.statusMessage_text || ("Downloading… " + Math.round(updaterViewModel.progress * 100) + "%"))
                            font.family: "Inter"; font.pixelSize: 10; font.bold: true
                            color: AppTheme.textPrimary
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: 40 } // Spacer

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: "Shobdo Calok  v" + (updaterViewModel ? updaterViewModel.currentVersion : "1.0.0")
                font.family: "Inter"; font.pixelSize: 11
                color: AppTheme.textSecondary; opacity: 0.5
            }
        }
    }

    // ── Action feedback toast ────────────────────────────────────────────────
    Rectangle {
        id: toastBox
        visible: false
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 20
        width: Math.min(toastText.implicitWidth + 40, parent.width - 80)
        height: 36
        radius: 10
        color: AppTheme.isDark ? "#1a1d23" : "#0f172a"
        border.color: AppTheme.hoverBg; border.width: 1
        z: 50
        Text {
            id: toastText
            anchors.centerIn: parent
            width: parent.width - 20
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideMiddle
            text: ""
            font.family: "Inter"; font.pixelSize: 11; color: "white"
        }
        opacity: 0
        Behavior on opacity { NumberAnimation { duration: 200 } }
        Timer { id: toastTimer; interval: 2600; onTriggered: { toastBox.opacity = 0 } }
    }

    function _toast(msg) {
        toastText.text = msg
        toastBox.visible = true
        toastBox.opacity = 1
        toastTimer.restart()
    }

    Connections {
        target: driveViewModel
        function onStatusMessage(msg) {
            settingsRoot._toast(msg)
        }
        function onLoginStateChanged(loggedIn) {
            settingsRoot._toast(loggedIn ? "Signed in" : "Signed out")
        }
    }

    Connections {
        target: updaterViewModel
        function onStatusMessage(msg) {
            settingsRoot._toast(msg)
        }
    }

    // ── Reusable Row Component ─────────────────────────────────────────────
    component SettingsRow: Rectangle {
        id: rowRoot
        property string icon
        property string title
        property string description
        property color activeColor: AppTheme.primary
        property bool checked: false
        property bool isButton: false
        property string buttonText: ""
        property bool buttonEnabled: true
        property color descriptionColor: AppTheme.textSecondary
        signal toggled(bool value)

        Layout.fillWidth: true; height: 60; radius: 14
        color: AppTheme.surface

        // 1. Icon Section (Locked to left: 12px)
        Rectangle {
            id: iconRect
            width: 32; height: 32; radius: 8
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            color: rowRoot.checked ? AppTheme.primaryLight : AppTheme.surface
            Behavior on color { ColorAnimation { duration: 200 } }
            
            Text {
                anchors.centerIn: parent; text: rowRoot.icon
                font.family: "Material Symbols Outlined"; font.pixelSize: 18
                color: rowRoot.checked ? rowRoot.activeColor : AppTheme.textSecondary
                Behavior on color { ColorAnimation { duration: 200 } }
            }
        }

        // 2. Text Column (FORCED VERTICAL ALIGNMENT AT 48px)
        // Offset Calculation: RowMargin(12) + IconWidth(32) + Gap(4) = 48
        ColumnLayout {
            id: textCol
            anchors.left: parent.left
            anchors.leftMargin: 48
            anchors.right: toggleItem.left
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            spacing: 1
            
            Text { 
                text: rowRoot.title
                font.family: "Inter"; font.pixelSize: 13; font.bold: true
                color: AppTheme.textPrimary
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text { 
                text: rowRoot.description
                font.family: "Inter"; font.pixelSize: 11
                color: rowRoot.descriptionColor
                elide: Text.ElideRight
                maximumLineCount: 1
                Layout.fillWidth: true
            }
        }

        // 3. Toggle / Button Section (Locked to right)
        Item {
            id: toggleItem
            width: rowRoot.isButton ? 90 : 50
            height: parent.height
            anchors.right: parent.right
            anchors.rightMargin: 12

            Rectangle {
                id: pill
                anchors.centerIn: parent
                width: 44; height: 24; radius: 12
                visible: !rowRoot.isButton
                color: rowRoot.checked ? AppTheme.primary : (AppTheme.isDark ? "#3a3d47" : "#d1d5db")
                Behavior on color { ColorAnimation { duration: 200 } }
                
                Rectangle {
                    width: 18; height: 18; radius: 9; color: "white"
                    anchors.verticalCenter: parent.verticalCenter
                    x: rowRoot.checked ? parent.width - width - 3 : 3
                    Behavior on x { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
                }
                
                TapHandler { onTapped: rowRoot.toggled(!rowRoot.checked) }
            }

            Rectangle {
                anchors.centerIn: parent
                width: 82; height: 30; radius: 8
                visible: rowRoot.isButton
                color: !rowRoot.buttonEnabled ? (AppTheme.isDark ? "#3a3d47" : "#d1d5db")
                       : (btnHover.hovered ? AppTheme.primaryHover : AppTheme.primary)
                HoverHandler { id: btnHover }
                TapHandler {
                    enabled: rowRoot.buttonEnabled
                    onTapped: rowRoot.toggled(true)
                }
                Text {
                    anchors.centerIn: parent; text: rowRoot.buttonText
                    font.family: "Inter"; font.pixelSize: 10; font.bold: true
                    color: rowRoot.buttonEnabled ? "white" : AppTheme.textSecondary
                }
            }
        }
    }
}
