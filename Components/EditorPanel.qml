import QtQuick
import QtQuick.Layouts
import QtQuick.Controls

Rectangle {
    id: root
    radius: 12
    color: "transparent"
    border.color: "transparent"

    property string currentFolder:  ""
    property string currentAbbrev:  ""
    property string currentContent: ""
    
    // Track if folder was changed via dropdown but not yet saved to backend
    property string pendingFolder: ""

    property var folderNames: []
    function refreshFolderNames() { folderNames = snippetViewModel.folders }
    Component.onCompleted: refreshFolderNames()

    Connections {
        target: snippetViewModel
        function onSnippetSelected(folder, abbrev, content) {
            root.currentFolder  = folder
            root.pendingFolder  = folder
            root.currentAbbrev  = abbrev
            root.currentContent = content
            mainTextArea.text   = content
            abbrevInput.text    = abbrev
        }
        function onFoldersChanged() { root.refreshFolderNames() }
    }

    // ── Timer for button flash feedback ──────────────────────────────────
    Timer {
        id: saveFeedbackTimer
        interval: 2000; repeat: false
        onTriggered: { saveBtnText.text = "SAVE CHANGES"; saveBtnRect.color = saveHover.hovered ? AppTheme.primary : AppTheme.primaryHover }
    }

    function doSave() {
        if (root.currentAbbrev === "") return
        
        var finalAbbrev = abbrevInput.text.trim()
        var finalContent = mainTextArea.text
        
        // 1. If folder changed, move first
        if (root.pendingFolder !== "" && root.pendingFolder !== root.currentFolder) {
            var moved = snippetViewModel.moveSnippet(root.currentAbbrev, root.currentFolder, root.pendingFolder)
            if (moved) root.currentFolder = root.pendingFolder
        }
        
        // 2. Update snippet content/abbreviation
        var saved = snippetViewModel.updateSnippet(
            root.currentFolder,
            root.currentAbbrev,
            finalAbbrev,
            finalContent
        )
        
        if (saved) {
            root.currentAbbrev = finalAbbrev
            root.currentContent = finalContent
            // Visual feedback inside button
            saveBtnText.text = "SAVED!"
            saveBtnRect.color = "#10b981" // Success green
            saveFeedbackTimer.restart()
            savePulse.start()
        }
    }

    // ── Delete confirmation dialog ─────────────────────────────────────────
    Rectangle {
        id: deleteDialog
        visible: false; anchors.fill: parent; color: "#80000000"; radius: 12; z: 100
        Rectangle {
            anchors.centerIn: parent; width: 320; height: 160; radius: 16; color: AppTheme.surface
            ColumnLayout {
                anchors.fill: parent; anchors.margins: 24; spacing: 16
                ColumnLayout {
                    spacing: 6
                    Text { text: "Delete Snippet?"; font.family: "Inter"; font.pixelSize: 15; font.bold: true; color: AppTheme.textPrimary }
                    Text { text: "This action cannot be undone."; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                }
                RowLayout {
                    Layout.fillWidth: true; spacing: 10
                    Rectangle {
                        Layout.fillWidth: true; height: 36; radius: 10
                        color: cancelDialogHover.hovered ? AppTheme.hoverBg : AppTheme.surface
                        border.color: AppTheme.textSecondary; border.width: 1
                        HoverHandler { id: cancelDialogHover }
                        TapHandler { onTapped: deleteDialog.visible = false }
                        Text { anchors.centerIn: parent; text: "Cancel"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary }
                    }
                    Rectangle {
                        Layout.fillWidth: true; height: 36; radius: 10
                        color: confirmDelHover.hovered ? AppTheme.danger : AppTheme.dangerLight
                        HoverHandler { id: confirmDelHover }
                        TapHandler {
                            onTapped: {
                                deleteDialog.visible = false
                                if (root.currentFolder !== "" && root.currentAbbrev !== "") {
                                    snippetViewModel.deleteSnippet(root.currentFolder, root.currentAbbrev)
                                    root.currentAbbrev = ""; root.currentContent = ""; root.currentFolder = ""; root.pendingFolder = ""
                                    mainTextArea.text = ""; abbrevInput.text = ""
                                }
                            }
                        }
                        Text { anchors.centerIn: parent; text: "Delete"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: confirmDelHover.hovered ? "white" : AppTheme.danger }
                    }
                }
            }
        }
    }

    // ── Folder picker popup ────────────────────────────────────────────────
    Popup {
        id: folderDropdown
        z: 99
        width: 180; height: Math.min(root.folderNames.length * 36 + 8, 200)
        closePolicy: Popup.CloseOnPressOutside | Popup.CloseOnEscape
        background: Rectangle { radius: 10; color: AppTheme.surface; border.color: AppTheme.hoverBg; border.width: 1 }

        // Position below the folder picker button
        x: folderPickerBtn.mapToItem(root, 0, folderPickerBtn.height + 4).x
        y: folderPickerBtn.mapToItem(root, 0, folderPickerBtn.height + 4).y

        contentItem: ListView {
            anchors.fill: parent; anchors.margins: 4; model: root.folderNames; spacing: 2; clip: true
            delegate: Rectangle {
                width: parent.width; height: 36; radius: 8
                color: dFolderHover.hovered || modelData === root.pendingFolder ? AppTheme.hoverBg : "transparent"
                HoverHandler { id: dFolderHover }
                TapHandler {
                    onTapped: {
                        root.pendingFolder = modelData
                        folderDropdown.close()
                    }
                }
                RowLayout {
                    anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12
                    Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: modelData === root.pendingFolder ? AppTheme.primary : AppTheme.textSecondary }
                    Text { Layout.fillWidth: true; text: modelData; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: modelData === root.pendingFolder ? AppTheme.primary : AppTheme.textPrimary; elide: Text.ElideRight }
                    Text { text: "check"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.primary; visible: modelData === root.pendingFolder }
                }
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent; anchors.margins: 16; spacing: 16

        // ── Header row ────────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true; spacing: 12
            Text { text: "Snippet Editor"; font.family: "Inter"; font.pixelSize: 16; font.bold: true; color: AppTheme.textPrimary; Layout.fillWidth: true }

            Rectangle {
                id: folderPickerBtn
                Layout.preferredWidth: 160; Layout.preferredHeight: 34; radius: 10
                color: folderBtnHover.hovered ? AppTheme.hoverBg : AppTheme.surface
                HoverHandler { id: folderBtnHover }
                TapHandler { onTapped: { if (root.currentAbbrev !== "") { folderDropdown.visible ? folderDropdown.close() : folderDropdown.open() } } }
                RowLayout {
                    anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                    Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.pendingFolder !== "" ? AppTheme.primary : AppTheme.textSecondary }
                    Text { Layout.fillWidth: true; text: root.pendingFolder !== "" ? root.pendingFolder : "— no folder —"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: root.pendingFolder !== "" ? AppTheme.textPrimary : AppTheme.textSecondary; elide: Text.ElideRight }
                    Text { text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.textSecondary; visible: root.currentAbbrev !== "" }
                }
            }

            Rectangle {
                Layout.preferredWidth: 130; Layout.preferredHeight: 34; radius: 10; color: AppTheme.surface
                RowLayout {
                    anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
                    Image {
                        source: "../app-icon.svg"
                        Layout.preferredWidth: 16; Layout.preferredHeight: 16
                        sourceSize.width: 16; sourceSize.height: 16
                        fillMode: Image.PreserveAspectFit
                        opacity: 0.7
                    }
                    TextInput {
                        id: abbrevInput; Layout.fillWidth: true
                        verticalAlignment: Qt.AlignVCenter; color: AppTheme.primary; font.family: "Courier New"; font.pixelSize: 12; font.bold: true
                        selectionColor: AppTheme.primary; selectedTextColor: "white"
                        Text { anchors.verticalCenter: parent.verticalCenter; text: "trigger word"; font: parent.font; color: AppTheme.textSecondary; visible: parent.text === "" && !parent.activeFocus }
                    }
                }
            }

            Rectangle {
                Layout.preferredWidth: 34; Layout.preferredHeight: 34; radius: 10
                color: delHover.hovered ? AppTheme.danger : AppTheme.dangerLight
                HoverHandler { id: delHover }
                TapHandler { onTapped: { if (root.currentAbbrev !== "") deleteDialog.visible = true } }
                Text { anchors.centerIn: parent; text: "delete"; font.family: "Material Symbols Outlined"; font.pixelSize: 18; color: delHover.hovered ? "white" : AppTheme.danger }
            }
        }

        // ── Token insertion + SAVE row ──────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true; spacing: 8
            Text { text: "INSERT"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
            Item { Layout.preferredWidth: 2 }
            TokenBtn { iconText: "content_copy"; labelText: "CLIP"; onTapped: insertToken("{clipboard}") }
            TokenBtn { iconText: "calendar_today"; labelText: "DATE"; onTapped: insertToken("{date}") }
            TokenBtn { iconText: "schedule"; labelText: "TIME"; onTapped: insertToken("{time}") }
            TokenBtn { iconText: "input"; labelText: "FIELD"; highlight: true; onTapped: insertToken("{field:Label}") }
            TokenBtn { iconText: "text_fields"; labelText: "CURSOR"; highlight: true; onTapped: insertToken("{cursor}") }
            
            Item { Layout.fillWidth: true }

            Rectangle {
                id: saveBtnRect
                width: 130; height: 32; radius: 8
                color: saveHover.hovered ? AppTheme.primary : (saveFeedbackTimer.running ? "#10b981" : AppTheme.primaryHover)
                
                SequentialAnimation { id: savePulse
                    NumberAnimation { target: saveBtnRect; property: "scale"; from: 1.0; to: 0.95; duration: 60 }
                    NumberAnimation { target: saveBtnRect; property: "scale"; from: 0.95; to: 1.0; duration: 100; easing.type: Easing.OutBack }
                }

                HoverHandler { id: saveHover }
                TapHandler { onTapped: root.doSave() }
                
                Text {
                    id: saveBtnText; anchors.centerIn: parent; text: "SAVE CHANGES"
                    font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: "white"
                }
            }
        }

        // ── Editor area ───────────────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true; Layout.fillHeight: true; radius: 16; color: AppTheme.surface; clip: true
            ScrollView {
                id: scrollView; anchors.fill: parent; anchors.margins: 4; clip: true
                ScrollBar.vertical: ModernScrollBar { parent: scrollView; anchors.right: scrollView.right; anchors.rightMargin: 4 }
                TextArea { id: mainTextArea; color: AppTheme.textPrimary; font.family: "Inter"; font.pixelSize: 13; wrapMode: TextArea.Wrap; leftPadding: 16; rightPadding: 24; topPadding: 16; bottomPadding: 16; background: null; selectByMouse: true; selectionColor: AppTheme.primary; placeholderText: "Select a snippet from the library to edit it…"; placeholderTextColor: AppTheme.textSecondary }
            }
        }

        // ── Live expansion preview ────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 96
            radius: 12; color: AppTheme.surface; clip: true
            visible: root.currentAbbrev !== ""

            ColumnLayout {
                anchors.fill: parent; anchors.margins: 10; spacing: 6
                RowLayout {
                    Layout.fillWidth: true; spacing: 6
                    Text { text: "preview"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.primary }
                    Text { text: "LIVE PREVIEW"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; font.letterSpacing: 1; color: AppTheme.textSecondary }
                    Item { Layout.fillWidth: true }
                }
                Rectangle {
                    Layout.fillWidth: true; Layout.fillHeight: true; radius: 8; color: AppTheme.editorBg
                    ScrollView {
                        id: previewScroll; anchors.fill: parent; anchors.margins: 2; clip: true
                        ScrollBar.vertical: ModernScrollBar { parent: previewScroll; anchors.right: previewScroll.right; anchors.rightMargin: 4 }
                        TextArea {
                            id: previewArea
                            readOnly: true
                            text: root.currentAbbrev === "" ? "" : (snippetViewModel ? snippetViewModel.previewExpansion(mainTextArea.text) : "")
                            font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary
                            wrapMode: TextArea.Wrap; background: null; padding: 8
                            selectionColor: AppTheme.primary
                        }
                    }
                }
            }
        }
    }

    function insertToken(token) {
        var pos = mainTextArea.cursorPosition; mainTextArea.insert(pos, token); mainTextArea.cursorPosition = pos + token.length
    }

    component TokenBtn: RowLayout {
        property string iconText; property string labelText; property bool highlight: false; signal tapped(); spacing: 4
        HoverHandler { id: tokenHover }
        TapHandler { onTapped: parent.tapped() }
        Text { text: iconText; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: tokenHover.hovered ? AppTheme.primary : (highlight ? AppTheme.primary : AppTheme.textSecondary) }
        Text { text: labelText; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: tokenHover.hovered ? AppTheme.primary : (highlight ? AppTheme.primary : AppTheme.textSecondary) }
    }
}
