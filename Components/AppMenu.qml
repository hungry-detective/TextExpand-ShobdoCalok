import QtQuick
import QtQuick.Controls

Menu {
    id: root

    readonly property bool mMirrored: count > 0 && itemAt(0).mirrored

    x: mMirrored ? -width + parent.width : 0

    // Theme-aware background
    background: Rectangle {
        implicitWidth: 180
        radius: 10
        color: AppTheme.surface
        border.color: AppTheme.isDark ? "#2a2d35" : "#e2e8f0"
        border.width: 1

        // Drop shadow effect
        Rectangle {
            anchors.fill: parent
            anchors.margins: -1
            radius: 11
            color: "transparent"
            border.color: AppTheme.isDark ? "#00000060" : "#00000020"
            border.width: 1
            z: -1
        }
    }

    // Theme-aware delegate
    delegate: MenuItem {
        id: menuItem
        contentItem: Text {
            leftPadding: 8
            text: menuItem.text
            font.family: "Inter"
            font.pixelSize: 12
            color: menuItem.highlighted ? AppTheme.primary : AppTheme.textPrimary
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            implicitWidth: 160
            implicitHeight: 34
            radius: 6
            color: menuItem.highlighted ? AppTheme.hoverBg : "transparent"
            anchors.margins: 4
        }
    }

    width: {
        let result = 0
        let padding = 0
        for (let i = 0; i < root.count; ++i) {
            let item = root.itemAt(i)
            if (!isMenuSeparator(item)) {
                result = Math.max(item.contentItem.implicitWidth, result)
                padding = Math.max(item.padding, padding)
            }
        }
        return result + padding * 2 + 48
    }

    function isMenuSeparator(item) {
        return item instanceof MenuSeparator
    }
}
