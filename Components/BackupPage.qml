import QtQuick
import QtQuick.Layouts
import QtQuick.Controls

Item {
    id: backupRoot

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
                    text: "Backup"
                    font.family: "Inter"; font.pixelSize: 22; font.bold: true
                    color: AppTheme.textPrimary
                }
                Text {
                    text: "Protect your snippets — locally or in the cloud"
                    font.family: "Inter"; font.pixelSize: 12
                    color: AppTheme.textSecondary
                }
            }

            Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.textSecondary; opacity: 0.15; Layout.topMargin: 20; Layout.bottomMargin: 20 }

            // ── Sections ──────────────────────────────────────────────────────
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 24

                // Section: LIBRARY
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "LIBRARY"
                        font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary
                        Layout.leftMargin: 48
                    }
                    SettingsRow {
                        icon: "upload_file"
                        title: "Export Library"
                        description: "Save all snippets as a JSON backup"
                        activeColor: AppTheme.primary
                        isButton: true; buttonText: "EXPORT"
                        onToggled: {
                            var r = snippetViewModel.exportSnippets()
                            if (r !== "") backupRoot._toast(r)
                        }
                    }
                    SettingsRow {
                        icon: "download"
                        title: "Import Library"
                        description: "Restore snippets from a JSON file"
                        activeColor: AppTheme.primary
                        isButton: true; buttonText: "IMPORT"
                        onToggled: {
                            var r = snippetViewModel.importSnippets()
                            if (r !== "") backupRoot._toast(r)
                        }
                    }
                }

                // Section: GOOGLE BACKUP
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "GOOGLE BACKUP"
                        font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary
                        Layout.leftMargin: 48
                    }

                    // ── Account status card ───────────────────────────────────
                    Rectangle {
                        Layout.fillWidth: true; height: 60; radius: 14
                        color: AppTheme.surface
                        Rectangle {
                            width: 32; height: 32; radius: 8
                            anchors.left: parent.left; anchors.leftMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            color: driveViewModel.loggedIn ? AppTheme.primaryLight : AppTheme.surface
                            Text {
                                anchors.centerIn: parent
                                text: driveViewModel.loggedIn ? "cloud_done" : "cloud"
                                font.family: "Material Symbols Outlined"; font.pixelSize: 18
                                color: driveViewModel.loggedIn ? "#22c55e" : AppTheme.textSecondary
                            }
                        }
                        ColumnLayout {
                            anchors.left: parent.left; anchors.leftMargin: 48
                            anchors.right: accountBtn.left; anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 1
                            Text {
                                text: driveViewModel.loggedIn ? "Google Drive connected" : "Connect Google account"
                                font.family: "Inter"; font.pixelSize: 13; font.bold: true
                                color: AppTheme.textPrimary; elide: Text.ElideRight; Layout.fillWidth: true
                            }
                            Text {
                                text: driveViewModel.loggedIn
                                      ? (driveViewModel.accountName
                                         + (driveViewModel.lastBackup
                                            ? ("  ·  " + "Last backup: " + driveViewModel.lastBackup)
                                            : "  ·  No backup yet"))
                                      : "Back up and sync your snippets securely"
                                font.family: "Inter"; font.pixelSize: 11
                                color: AppTheme.textSecondary; elide: Text.ElideRight; maximumLineCount: 1; Layout.fillWidth: true
                            }
                        }
                        Rectangle {
                            id: accountBtn
                            width: 92; height: 30; radius: 8
                            anchors.right: parent.right; anchors.rightMargin: 12
                            anchors.verticalCenter: parent.verticalCenter
                            color: accountBtnHover.hovered ? AppTheme.primaryHover : AppTheme.primary
                            HoverHandler { id: accountBtnHover }
                            TapHandler {
                                onTapped: driveViewModel.loggedIn ? driveViewModel.logout() : driveViewModel.login()
                            }
                            Text {
                                anchors.centerIn: parent
                                text: driveViewModel.loggedIn ? "SIGN OUT" : "SIGN IN"
                                font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: "white"
                            }
                        }
                    }

                    // ── Backup / Restore buttons ──────────────────────────────
                    Rectangle {
                        Layout.fillWidth: true; height: 56; radius: 14
                        color: AppTheme.surface
                        visible: driveViewModel.loggedIn
                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Rectangle {
                                id: backupBtn
                                Layout.fillWidth: true; Layout.fillHeight: true
                                radius: 9
                                color: backupHover.hovered ? AppTheme.primaryHover : AppTheme.primary
                                enabled: !driveViewModel.busy
                                opacity: enabled ? 1 : 0.5
                                HoverHandler { id: backupHover }
                                TapHandler { onTapped: backupConfirmDialog.open() }
                                ColumnLayout {
                                    anchors.centerIn: parent; spacing: 1
                                    Text {
                                        text: "Backup to Drive"
                                        Layout.alignment: Qt.AlignHCenter
                                        font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white"
                                    }
                                    Text {
                                        text: "Upload snippets"
                                        Layout.alignment: Qt.AlignHCenter
                                        font.family: "Inter"; font.pixelSize: 9
                                        color: "white"; opacity: 0.75
                                    }
                                }
                            }

                            Rectangle {
                                id: restoreBtn
                                Layout.fillWidth: true; Layout.fillHeight: true
                                radius: 9
                                color: restoreHover.hovered ? AppTheme.primaryHover : AppTheme.primary
                                enabled: !driveViewModel.busy
                                opacity: enabled ? 1 : 0.5
                                HoverHandler { id: restoreHover }
                                TapHandler { onTapped: { driveViewModel.listBackups(); restorePicker.open() } }
                                ColumnLayout {
                                    anchors.centerIn: parent; spacing: 1
                                    Text {
                                        text: "Restore from Drive"
                                        Layout.alignment: Qt.AlignHCenter
                                        font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white"
                                    }
                                    Text {
                                        text: "Download backup"
                                        Layout.alignment: Qt.AlignHCenter
                                        font.family: "Inter"; font.pixelSize: 9
                                        color: "white"; opacity: 0.75
                                    }
                                }
                            }
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

    // ── Confirmation dialog before backup ────────────────────────────────────
    Dialog {
        id: backupConfirmDialog
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 380; height: contentHeight + 40
        modal: true; dim: true; closePolicy: Popup.CloseOnEscape
        background: Rectangle { radius: 16; color: AppTheme.surface; border.color: AppTheme.hoverBg; border.width: 1 }
        contentItem: ColumnLayout {
            spacing: 16; anchors.margins: 20
            Text {
                text: "Overwrite Backup?"
                font.family: "Inter"; font.pixelSize: 16; font.bold: true
                color: AppTheme.textPrimary
            }
            Text {
                text: "This will replace your existing backup on Google Drive. Your previous backup will be saved as a history copy, but to be safe, are you sure?"
                font.family: "Inter"; font.pixelSize: 12
                color: AppTheme.textSecondary; wrapMode: Text.Wrap; Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true; spacing: 10
                Item { Layout.fillWidth: true }
                Rectangle {
                    width: 80; height: 32; radius: 8
                    color: cancelHover.hovered ? AppTheme.hoverBg : "transparent"
                    border.color: AppTheme.textSecondary; border.width: 1
                    HoverHandler { id: cancelHover }
                    TapHandler { onTapped: backupConfirmDialog.close() }
                    Text { anchors.centerIn: parent; text: "Cancel"; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary }
                }
                Rectangle {
                    width: 100; height: 32; radius: 8
                    color: confirmHover.hovered ? AppTheme.primaryHover : AppTheme.primary
                    HoverHandler { id: confirmHover }
                    TapHandler { onTapped: { backupConfirmDialog.close(); driveViewModel.backup() } }
                    Text { anchors.centerIn: parent; text: "Yes, Backup"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: "white" }
                }
            }
        }
    }

    // ── Restore picker dialog ────────────────────────────────────────────────
    Dialog {
        id: restorePicker
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 420; height: Math.min(500, backupList.contentHeight + 120)
        modal: true; dim: true; closePolicy: Popup.CloseOnEscape
        background: Rectangle { radius: 16; color: AppTheme.surface; border.color: AppTheme.hoverBg; border.width: 1 }
        onOpened: { backupListModel.clear() }
        contentItem: ColumnLayout {
            spacing: 12; anchors.margins: 20
            Text {
                text: "Choose Backup to Restore"
                font.family: "Inter"; font.pixelSize: 16; font.bold: true
                color: AppTheme.textPrimary
            }
            Text {
                text: "Select which backup you want to restore:"
                font.family: "Inter"; font.pixelSize: 12
                color: AppTheme.textSecondary
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.textSecondary; opacity: 0.15 }
            ListView {
                id: backupList
                Layout.fillWidth: true; Layout.fillHeight: true
                clip: true; spacing: 4
                model: ListModel { id: backupListModel }
                delegate: Rectangle {
                    Layout.fillWidth: true; height: 44; radius: 8
                    color: itemHover.hovered ? AppTheme.primaryLight : "transparent"
                    HoverHandler { id: itemHover }
                    RowLayout {
                        anchors.fill: parent; anchors.margins: 8; spacing: 8
                        Text {
                            text: "history"
                            font.family: "Material Symbols Outlined"; font.pixelSize: 18
                            color: index === 0 ? AppTheme.primary : AppTheme.textSecondary
                        }
                        Text {
                            text: model.label
                            font.family: "Inter"; font.pixelSize: 12
                            color: AppTheme.textPrimary; elide: Text.ElideRight
                            Layout.fillWidth: true
                        }
                    }
                    TapHandler {
                        onTapped: {
                            restorePicker.close()
                            if (index === 0) {
                                // Latest backup - use default restore
                                driveViewModel.restore()
                            } else {
                                // Historical backup - restore by file ID
                                driveViewModel.restoreFromFile(model.id)
                            }
                        }
                    }
                }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.textSecondary; opacity: 0.15 }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Rectangle {
                    width: 80; height: 32; radius: 8
                    color: restoreCancelHover.hovered ? AppTheme.hoverBg : "transparent"
                    border.color: AppTheme.textSecondary; border.width: 1
                    HoverHandler { id: restoreCancelHover }
                    TapHandler { onTapped: restorePicker.close() }
                    Text { anchors.centerIn: parent; text: "Cancel"; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary }
                }
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
            backupRoot._toast(msg)
        }
        function onLoginStateChanged(loggedIn) {
            backupRoot._toast(loggedIn ? "Signed in" : "Signed out")
        }
        function onBackupListReady(list) {
            backupListModel.clear()
            for (var i = 0; i < list.length; i++) {
                backupListModel.append({
                    "id": list[i].id,
                    "name": list[i].name,
                    "label": list[i].label
                })
            }
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
                color: AppTheme.textSecondary
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
                color: btnHover.hovered ? AppTheme.primaryHover : AppTheme.primary
                HoverHandler { id: btnHover }
                TapHandler { onTapped: rowRoot.toggled(true) }
                Text {
                    anchors.centerIn: parent; text: rowRoot.buttonText
                    font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: "white"
                }
            }
        }
    }
}
