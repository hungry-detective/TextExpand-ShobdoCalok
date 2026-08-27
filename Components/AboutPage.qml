import QtQuick
import QtQuick.Layouts

Item {
    id: aboutRoot

    // ── Animated background circles ───────────────────────────────────────
    Rectangle {
        id: bgCircle1
        width: 320; height: 320; radius: 160
        color: AppTheme.primary; opacity: 0.04
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: -80
        anchors.verticalCenterOffset: -60

        SequentialAnimation on scale {
            loops: Animation.Infinite
            NumberAnimation { to: 1.08; duration: 3000; easing.type: Easing.InOutSine }
            NumberAnimation { to: 0.96; duration: 3000; easing.type: Easing.InOutSine }
        }
    }

    Rectangle {
        id: bgCircle2
        width: 200; height: 200; radius: 100
        color: "#38bdf8"; opacity: 0.05
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: 100
        anchors.verticalCenterOffset: 80

        SequentialAnimation on scale {
            loops: Animation.Infinite
            NumberAnimation { to: 1.12; duration: 4000; easing.type: Easing.InOutSine }
            NumberAnimation { to: 0.9;  duration: 4000; easing.type: Easing.InOutSine }
        }
    }

    // ── Main content ──────────────────────────────────────────────────────
    ColumnLayout {
        anchors.centerIn: parent
        spacing: 20
        width: Math.min(420, parent.width * 0.85)

        // ── Animated Logo ─────────────────────────────────────────────────
        Item {
            Layout.alignment: Qt.AlignHCenter
            width: 100; height: 100

            // Outer glow ring
            Rectangle {
                id: glowRing
                anchors.centerIn: parent
                width: 100; height: 100; radius: 50
                color: "transparent"
                border.color: AppTheme.primary
                border.width: 2
                opacity: 0.5

                SequentialAnimation on opacity {
                    loops: Animation.Infinite
                    NumberAnimation { to: 0.8; duration: 1200; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 0.2; duration: 1200; easing.type: Easing.InOutSine }
                }

                SequentialAnimation on scale {
                    loops: Animation.Infinite
                    NumberAnimation { to: 1.12; duration: 1200; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 0.95; duration: 1200; easing.type: Easing.InOutSine }
                }
            }

            // Inner icon background
            Rectangle {
                id: iconBg
                anchors.centerIn: parent
                width: 72; height: 72; radius: 22
                color: AppTheme.primary

                layer.enabled: true
                layer.effect: null

                // Gentle float animation
                SequentialAnimation on anchors.verticalCenterOffset {
                    loops: Animation.Infinite
                    NumberAnimation { to: -6;  duration: 1800; easing.type: Easing.InOutSine }
                    NumberAnimation { to:  6;  duration: 1800; easing.type: Easing.InOutSine }
                }

                // Icon glyph
                Text {
                    anchors.centerIn: parent
                    text: "bolt"
                    font.family: "Material Symbols Outlined"
                    font.pixelSize: 36
                    color: "white"

                    // Spin animation
                    RotationAnimation on rotation {
                        loops: Animation.Infinite
                        from: -8; to: 8
                        duration: 2400
                        direction: RotationAnimation.Shortest
                        easing.type: Easing.InOutSine
                    }
                }
            }
        }

        // ── App Name ──────────────────────────────────────────────────────
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "Shobdo Calok"
            font.family: "Inter"
            font.pixelSize: 28
            font.bold: true
            color: AppTheme.textPrimary
        }

        // ── Version badge ─────────────────────────────────────────────────
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            width: versionText.implicitWidth + 24
            height: 26; radius: 13
            color: AppTheme.primaryLight

            Text {
                id: versionText
                anchors.centerIn: parent
                text: "Version " + (updaterViewModel ? updaterViewModel.currentVersion : "1.0.0")
                font.family: "Inter"
                font.pixelSize: 12
                font.bold: true
                color: AppTheme.primary
            }
        }

        // ── Divider ───────────────────────────────────────────────────────
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            width: 200; height: 1
            color: AppTheme.textSecondary
            opacity: 0.2
        }

        // ── Description ───────────────────────────────────────────────────
        Text {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            horizontalAlignment: Qt.AlignHCenter
            wrapMode: Text.WordWrap
            text: "Shobdo Calok is a fast, keyboard-friendly text expander that\nlets you define short abbreviations which automatically expand\ninto full phrases, code blocks, or dynamic templates.\n\nSupports date/time tokens, clipboard insertion, fill-in fields,\nand folder-based snippet organisation — all running silently\nin your system tray."
            font.family: "Inter"
            font.pixelSize: 13
            color: AppTheme.textSecondary
            lineHeight: 1.5
        }

        // ── Divider ───────────────────────────────────────────────────────
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            width: 200; height: 1
            color: AppTheme.textSecondary
            opacity: 0.2
        }

        // ── Check for Updates button ─────────────────────────────────────
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: 260
            height: 38; radius: 12
            color: updateMa.pressed ? AppTheme.primaryDark
                 : updateMa.containsMouse ? AppTheme.primaryLight
                 : AppTheme.isDark ? "#1a1d23" : "#f1f5f9"
            border.color: AppTheme.primary
            border.width: 1
            clip: true

            // Progress fill behind text
            Rectangle {
                id: updateProgressFill
                visible: updaterViewModel && (updaterViewModel.downloading || updaterViewModel.applying)
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: parent.width * (updaterViewModel ? updaterViewModel.progress : 0)
                radius: 12
                color: updaterViewModel.applying ? "#22c55e" : AppTheme.primary
                opacity: 0.2
                Behavior on width { NumberAnimation { duration: 100 } }
            }

            Row {
                id: updateBtnRow
                anchors.centerIn: parent
                spacing: 8

                Text {
                    text: "system_update"
                    font.family: "Material Symbols Outlined"
                    font.pixelSize: 18
                    color: AppTheme.primary
                    anchors.verticalCenter: parent.verticalCenter
                }

                Text {
                    id: updateBtnLabel
                    text: {
                        if (!updaterViewModel) return "Check for Updates"
                        if (updaterViewModel.applying) return "Applying…"
                        if (updaterViewModel.downloading)
                            return "Downloading… " + Math.round(updaterViewModel.progress * 100) + "%"
                        if (updaterViewModel.checking) return "Checking…"
                        if (updaterViewModel.updateAvailable)
                            return "Update v" + updaterViewModel.latestVersion + " (you have " + (typeof updaterViewModel.currentVersion !== "undefined" ? updaterViewModel.currentVersion : "?") + ")"
                        return "Check for Updates"
                    }
                    font.family: "Inter"
                    font.pixelSize: 12
                    font.bold: true
                    color: AppTheme.primary
                    anchors.verticalCenter: parent.verticalCenter
                }
            }

            MouseArea {
                id: updateMa
                anchors.fill: parent
                hoverEnabled: true
                enabled: !updaterViewModel || (!updaterViewModel.downloading && !updaterViewModel.applying && !updaterViewModel.checking)
                onClicked: {
                    if (!updaterViewModel) return
                    if (updaterViewModel.updateAvailable)
                        updaterViewModel.downloadAndInstall()
                    else
                        updaterViewModel.checkForUpdate()
                }
            }

            Connections {
                target: updaterViewModel
                function onUpdateAvailableChanged() {
                    if (updaterViewModel.updateAvailable) {
                        updateBtnLabel.text = "Update v" + updaterViewModel.latestVersion + " (you have " + updaterViewModel.currentVersion + ")"
                        updateStatusLabel.text = ""
                    } else if (!updaterViewModel.checking) {
                        updateStatusLabel.text = "You're up to date"
                        updateStatusLabel.color = "#22c55e"
                        upToDateTimer.restart()
                    }
                }
                function onStatusMessage(msg) {
                    if (!msg) return
                    if (msg.indexOf("failed") !== -1 || msg.indexOf("error") !== -1) {
                        updateStatusLabel.text = msg
                        updateStatusLabel.color = "#ef4444"
                    } else if (msg.indexOf("up to date") !== -1) {
                        updateStatusLabel.text = "You're up to date"
                        updateStatusLabel.color = "#22c55e"
                    } else if (msg.indexOf("available") !== -1) {
                        updateStatusLabel.text = msg
                        updateStatusLabel.color = AppTheme.primary
                    } else {
                        updateStatusLabel.text = msg
                        updateStatusLabel.color = AppTheme.textSecondary
                    }
                    statusResetTimer.restart()
                }
            }
        }

        // ── Update status text ────────────────────────────────────────────
        Text {
            id: updateStatusLabel
            Layout.alignment: Qt.AlignHCenter
            visible: text.length > 0
            text: ""
            font.family: "Inter"
            font.pixelSize: 11
            color: AppTheme.textSecondary
            height: visible ? implicitHeight : 0

            Timer {
                id: statusResetTimer
                interval: 5000
                onTriggered: updateStatusLabel.text = ""
            }
            Timer {
                id: upToDateTimer
                interval: 4000
                onTriggered: updateStatusLabel.text = ""
            }
        }

        // ── Footer ────────────────────────────────────────────────────────
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: "© 2026 Shobdo Calok. All rights reserved."
            font.family: "Inter"
            font.pixelSize: 11
            color: AppTheme.textSecondary
            opacity: 0.6
        }
    }
}
