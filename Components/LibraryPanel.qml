import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import QtQuick.Controls.Basic

Item {
    id: libraryRoot

    property string draggingFolderName:    ""
    property string draggingSnippetName:   ""
    property string draggingSnippetFolder: ""
    property string selectedFolder:        ""
    property string selectedAbbrev:        ""
    property string searchText:            ""
    property int    dragThreshold:         8
    // Tracks which folder currently holds the live drag-preview placeholder
    property string dragPreviewFolder:     ""

    property alias folderProxy:  folderProxy
    property alias snippetProxy: snippetProxy

    // ── Folder model (QML-side, built from Python) ────────────────────────
    ListModel { id: folderListModel }

    // ── Per-folder snippet models ─────────────────────────────────────────
    property var snippetModels: ({})

    function rebuildAll() {
        var names = snippetViewModel.folders
        if (!names || names.length === undefined) return

        folderListModel.clear()
        var newModels = {}

        for (var i = 0; i < names.length; i++) {
            var fname = names[i]
            folderListModel.append({ "folderName": fname, "isExpanded": false })

            var snData = snippetViewModel.snippetsForFolder(fname)
            var m = Qt.createQmlObject('import QtQuick; ListModel {}', libraryRoot)
            for (var j = 0; j < snData.length; j++) {
                m.append({ "name": snData[j].abbreviation, "content": snData[j].content || "" })
            }
            newModels[fname] = m
        }
        snippetModels = newModels
    }

    function refreshFolder(fname) {
        var snData = snippetViewModel.snippetsForFolder(fname)
        var newModels = Object.assign({}, snippetModels)
        // Reuse the existing model when possible to avoid recreating QML objects
        var m = newModels[fname]
        if (!m) m = Qt.createQmlObject('import QtQuick; ListModel {}', libraryRoot)
        m.clear()
        for (var j = 0; j < snData.length; j++) {
            m.append({ "name": snData[j].abbreviation, "content": snData[j].content || "" })
        }
        newModels[fname] = m
        snippetModels = newModels
    }

    // Number of real snippets in a folder (excludes the drag-preview placeholder)
    function realCount(fname) {
        var sm = libraryRoot.snippetModelFor(fname)
        if (!sm) return 0
        var c = 0
        for (var i = 0; i < sm.count; i++) {
            if (sm.get(i).name !== "__drag_preview__") c++
        }
        return c
    }

    function snippetModelFor(fname) {
        return snippetModels[fname] || null
    }

    // Remove the drag-preview placeholder from whatever folder currently holds it
    function clearDragPreview() {
        if (libraryRoot.dragPreviewFolder === "") return
        var pm = libraryRoot.snippetModelFor(libraryRoot.dragPreviewFolder)
        if (pm) {
            for (var j = pm.count - 1; j >= 0; j--) {
                if (pm.get(j).name === "__drag_preview__") { pm.remove(j); break }
            }
        }
        libraryRoot.dragPreviewFolder = ""
    }

    function moveSnippetBetweenFolders(snName, fromFolder, toFolder) {
        snippetViewModel.moveSnippet(snName, fromFolder, toFolder)
    }

    // ── Context menu state ────────────────────────────────────────────────────
    property var folderCtxItems: [
        { icon: "drive_file_rename_outline", label: "Rename Folder", action: "rename" },
        { icon: "delete_outline", label: "Delete Folder", action: "deleteFolder" }
    ]
    property var snippetCtxItems: [
        { icon: "content_copy", label: "Duplicate", action: "duplicate" },
        { icon: "drive_file_move", label: "Move to…", action: "move" },
        { icon: "delete_outline", label: "Delete", action: "delete" }
    ]
    property var moveFolderRows: []

    function showContextMenu(folder, snippet, x, y, target) {
        ctxMenu.folder = folder
        ctxMenu.snippet = snippet
        ctxMenu.isSnippetMenu = snippet !== ""
        ctxMenu.view = 0
        ctxMenu.updateHeight()
        var p = target.mapToItem(libraryRoot, x, y)
        ctxMenu.x = Math.max(0, Math.min(p.x, libraryRoot.width - ctxMenu.width - 8))
        ctxMenu.y = Math.max(0, Math.min(p.y, libraryRoot.height - ctxMenu.height - 8))
        ctxMenu.open()
    }

    function buildMoveRows() {
        var rows = []
        var folders = snippetViewModel.folders || []
        for (var i = 0; i < folders.length; i++) {
            if (folders[i] !== ctxMenu.folder)
                rows.push({ icon: "folder", label: folders[i], folderName: folders[i], action: "moveTo" })
        }
        moveFolderRows = rows
    }

    function ctxTrigger(item) {
        if (ctxMenu.view === 1) {
            snippetViewModel.moveSnippet(ctxMenu.snippet, ctxMenu.folder, item.folderName)
            ctxMenu.close()
            return
        }
        if (ctxMenu.isSnippetMenu) {
            if (item.action === "duplicate") {
                snippetViewModel.duplicateSnippet(ctxMenu.folder, ctxMenu.snippet)
                ctxMenu.close()
            } else if (item.action === "move") {
                libraryRoot.buildMoveRows()
                ctxMenu.view = 1
                ctxMenu.updateHeight()
            } else if (item.action === "delete") {
                snippetViewModel.deleteSnippet(ctxMenu.folder, ctxMenu.snippet)
                ctxMenu.close()
            }
        } else {
            if (item.action === "rename") {
                ctxMenu.close()
                promptPopup.openRename(ctxMenu.folder)
            } else if (item.action === "deleteFolder") {
                snippetViewModel.deleteFolder(ctxMenu.folder)
                ctxMenu.close()
            }
        }
    }

    // Single toggle for expand/collapse all
    property bool allExpanded: {
        if (folderListModel.count === 0) return false
        for (var i = 0; i < folderListModel.count; i++) {
            if (!folderListModel.get(i).isExpanded) return false
        }
        return true
    }

    function toggleAll() {
        var target = !allExpanded
        for (var i = 0; i < folderListModel.count; i++)
            folderListModel.setProperty(i, "isExpanded", target)
    }

    Component.onCompleted: rebuildAll()

    Connections {
        target: snippetViewModel
        function onSnippetsChanged(folder) {
            libraryRoot.refreshFolder(folder)
        }
        function onFoldersChanged() { libraryRoot.rebuildAll() }
    }

    // ── Invisible drag proxies ────────────────────────────────────────────
    Item {
        id: folderProxy
        visible: false; width: 0; height: 0; z: 2999
        Drag.active: false
        Drag.keys: ["folderDrag"]
    }
    Item {
        id: snippetProxy
        visible: false; width: 0; height: 0; z: 2999
        Drag.active: false
        Drag.keys: ["snippetDrag"]
    }

    // ── Visual ghosts ─────────────────────────────────────────────────────
    Rectangle {
        id: folderGhost
        visible: false; z: 3000; radius: 10; opacity: 0.95
        width: 240; height: 42
        x: libraryRoot.folderProxy.x - width/2; y: libraryRoot.folderProxy.y - height/2
        color: AppTheme.primary
        
        property string folderName: ""
        
        RowLayout {
            anchors.fill: parent; anchors.margins: 12; spacing: 10
            Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: "white" }
            Text { Layout.fillWidth: true; text: folderGhost.folderName; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white" }
        }
        
        // Scale animation when starting
        Behavior on scale { NumberAnimation { duration: 100 } }
        scale: visible ? 0.95 : 1.0
    }

    Rectangle {
        id: snippetGhost
        visible: false; z: 3000; radius: 10; opacity: 0.95
        width: 220; height: 38
        x: libraryRoot.snippetProxy.x - width/2; y: libraryRoot.snippetProxy.y - height/2
        color: AppTheme.surface; border.color: AppTheme.primary; border.width: 1.5
        
        property string snippetName: ""
        
        RowLayout {
            anchors.fill: parent; anchors.margins: 12
            Text { Layout.fillWidth: true; text: snippetGhost.snippetName; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.textPrimary }
            Text { text: "SNIPPET"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
        }
        
        Behavior on scale { NumberAnimation { duration: 100 } }
        scale: visible ? 0.95 : 1.0
    }

    // ── Right-click context menu ─────────────────────────────────────────────
    Popup {
        id: ctxMenu
        z: 5000
        width: 210
        height: ctxMenu.rowsHeight
        padding: 5
        closePolicy: Popup.CloseOnPressOutside | Popup.CloseOnEscape
        background: Rectangle { radius: 10; color: AppTheme.surface; border.color: AppTheme.hoverBg; border.width: 1 }

        property string folder: ""
        property string snippet: ""
        property bool isSnippetMenu: false
        property int view: 0
        property int itemHeight: 34
        property int rowsHeight: 10

        function updateHeight() {
            var count
            if (ctxMenu.view === 1) count = moveFolderRows.length + 1
            else count = (ctxMenu.isSnippetMenu ? snippetCtxItems.length : folderCtxItems.length)
            rowsHeight = 10 + count * (itemHeight + 2)
        }

        contentItem: Column {
            spacing: 2

            // Back row (only in "Move to…" view)
            Rectangle {
                width: ctxMenu.width - 10; height: ctxMenu.itemHeight; radius: 8
                visible: ctxMenu.view === 1
                color: backHover.hovered ? AppTheme.hoverBg : "transparent"
                HoverHandler { id: backHover }
                TapHandler { onTapped: { ctxMenu.view = 0; ctxMenu.updateHeight() } }
                RowLayout {
                    anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                    Text { text: "arrow_back"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: AppTheme.textSecondary }
                    Text { Layout.fillWidth: true; text: "Back"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary }
                }
            }

            Repeater {
                model: ctxMenu.view === 0 ? (ctxMenu.isSnippetMenu ? snippetCtxItems : folderCtxItems) : moveFolderRows
                delegate: Rectangle {
                    required property var modelData
                    width: ctxMenu.width - 10; height: ctxMenu.itemHeight; radius: 8
                    color: ctxRowHover.hovered ? AppTheme.hoverBg : "transparent"
                    HoverHandler { id: ctxRowHover }
                    TapHandler { onTapped: libraryRoot.ctxTrigger(modelData) }
                    RowLayout {
                        anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                        Text { text: modelData.icon; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: modelData.action === "delete" || modelData.action === "deleteFolder" ? AppTheme.danger : AppTheme.textSecondary }
                        Text { Layout.fillWidth: true; text: modelData.label; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary; elide: Text.ElideRight }
                    }
                }
            }
        }
    }

    // ── Inline prompt (rename folder) ────────────────────────────────────────
    Popup {
        id: promptPopup
        z: 5001
        modal: true
        width: 340; height: 170
        anchors.centerIn: parent
        closePolicy: Popup.CloseOnPressOutside | Popup.CloseOnEscape
        background: Rectangle { radius: 16; color: AppTheme.surface }

        property string mode: ""
        property string input: ""
        property string titleText: ""

        function openRename(folderName) {
            promptPopup.mode = "renameFolder"
            promptPopup.input = folderName
            promptPopup.titleText = "Rename Folder"
            nameInput.text = folderName
            promptPopup.open()
            nameInput.forceActiveFocus()
            nameInput.selectAll()
        }

        function doConfirm() {
            var val = nameInput.text.trim()
            if (promptPopup.mode === "renameFolder" && val !== "" && val !== promptPopup.input)
                snippetViewModel.renameFolder(promptPopup.input, val)
            promptPopup.close()
        }

        contentItem: ColumnLayout {
            anchors.fill: parent; anchors.margins: 22; spacing: 14
            Text { text: promptPopup.titleText; font.family: "Inter"; font.pixelSize: 15; font.bold: true; color: AppTheme.textPrimary }
            Rectangle {
                Layout.fillWidth: true; height: 36; radius: 10
                border.color: nameInput.activeFocus ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                TextInput {
                    id: nameInput; anchors.fill: parent; anchors.margins: 10
                    font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary
                    verticalAlignment: TextInput.AlignVCenter
                    Keys.onReturnPressed: promptPopup.doConfirm()
                    Keys.onEscapePressed: promptPopup.close()
                }
            }
            RowLayout {
                Layout.fillWidth: true; spacing: 10
                Item { Layout.fillWidth: true }
                Rectangle {
                    width: 90; height: 34; radius: 10; color: AppTheme.hoverBg
                    HoverHandler { id: pCancelHover }
                    TapHandler { onTapped: promptPopup.close() }
                    Text { anchors.centerIn: parent; text: "Cancel"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary }
                }
                Rectangle {
                    width: 90; height: 34; radius: 10; color: AppTheme.primary
                    HoverHandler { id: pOkHover }
                    TapHandler { onTapped: promptPopup.doConfirm() }
                    Text { anchors.centerIn: parent; text: "OK"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: "white" }
                }
            }
        }
    }

    // ── Empty library state ──────────────────────────────────────────────────
    Item {
        id: emptyState
        anchors { top: searchBar.bottom; topMargin: 16; bottom: parent.bottom; left: parent.left; right: parent.right }
        visible: folderListModel.count === 0
        ColumnLayout {
            anchors.centerIn: parent; spacing: 8
            Text { Layout.alignment: Qt.AlignHCenter; text: "inbox"; font.family: "Material Symbols Outlined"; font.pixelSize: 40; color: AppTheme.textSecondary; opacity: 0.4 }
            Text { Layout.alignment: Qt.AlignHCenter; text: "No folders yet"; font.family: "Inter"; font.pixelSize: 13; font.bold: true; color: AppTheme.textPrimary }
            Text { Layout.alignment: Qt.AlignHCenter; text: "Click + to create your first snippet"; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary }
        }
    }

    // ── Header ────────────────────────────────────────────────────────────
    RowLayout {
        id: headerRow
        width: parent.width; height: 32
        Text {
            Layout.fillWidth: true
            text: "LIBRARY"; font.family: "Inter"; font.pixelSize: 10; font.bold: true
            font.letterSpacing: 1.5; color: AppTheme.textSecondary
        }
        RowLayout {
            spacing: 4

            // Single Toggle Expansion Button
            Rectangle {
                width: 28; height: 28; radius: 14
                color: bulkHover.hovered ? AppTheme.hoverBg : AppTheme.surface
                Text {
                    anchors.centerIn: parent; text: libraryRoot.allExpanded ? "unfold_less" : "unfold_more"
                    font.family: "Material Symbols Outlined"; font.pixelSize: 18
                    color: bulkHover.hovered ? AppTheme.primary : AppTheme.textSecondary
                }
                HoverHandler { id: bulkHover }
                TapHandler { onTapped: libraryRoot.toggleAll() }
                ToolTip {
                    id: bulkToolTip
                    visible: bulkHover.hovered
                    text: libraryRoot.allExpanded ? "Collapse All" : "Expand All"
                    delay: 400
                    contentItem: Text { text: bulkToolTip.text; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textPrimary }
                    background: Rectangle { color: AppTheme.surface; radius: 6; border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"; border.width: 1 }
                }
            }

            // Add Folder
            Rectangle {
                width: 28; height: 28; radius: 14
                color: addFolderHover.hovered ? AppTheme.hoverBg : AppTheme.surface
                Text {
                    anchors.centerIn: parent; text: "create_new_folder"
                    font.family: "Material Symbols Outlined"; font.pixelSize: 18
                    color: addFolderHover.hovered ? AppTheme.primary : AppTheme.textSecondary
                }
                HoverHandler { id: addFolderHover }
                ToolTip {
                    id: addFolderToolTip
                    visible: addFolderHover.hovered
                    text: "New Folder"
                    delay: 400
                    contentItem: Text { text: addFolderToolTip.text; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textPrimary }
                    background: Rectangle { color: AppTheme.surface; radius: 6; border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"; border.width: 1 }
                }
                TapHandler { onTapped: snippetViewModel.addFolder("New Folder") }
            }

            // Add Snippet
            Rectangle {
                width: 28; height: 28; radius: 14
                color: addSnHover.hovered ? AppTheme.hoverBg : AppTheme.surface
                Text {
                    anchors.centerIn: parent; text: "add_box"
                    font.family: "Material Symbols Outlined"; font.pixelSize: 18
                    color: addSnHover.hovered ? AppTheme.primary : AppTheme.textSecondary
                }
                HoverHandler { id: addSnHover }
                TapHandler {
                    onTapped: {
                        var targetFolder = libraryRoot.selectedFolder !== "" ? libraryRoot.selectedFolder : ""
                        if (targetFolder === "" && folderListModel.count > 0)
                            targetFolder = folderListModel.get(0).folderName

                        if (targetFolder !== "") {
                            var sm = libraryRoot.snippetModels[targetFolder]
                            if (!sm) return
                            var newAbbrev = ".new"
                            var counter = 1
                            var exists = true
                            while (exists) {
                                exists = false
                                for (var i = 0; i < sm.count; i++) {
                                    if (sm.get(i).name === newAbbrev) {
                                        exists = true; newAbbrev = ".new" + counter; counter++; break
                                    }
                                }
                            }
                            var ok = snippetViewModel.addSnippet(targetFolder, newAbbrev)
                            if (ok) {
                                for (var j = 0; j < folderListModel.count; j++) {
                                    if (folderListModel.get(j).folderName === targetFolder) {
                                        folderListModel.setProperty(j, "isExpanded", true); break
                                    }
                                }
                                libraryRoot.selectedFolder = targetFolder; libraryRoot.selectedAbbrev = newAbbrev
                                snippetViewModel.selectSnippet(targetFolder, newAbbrev)
                            }
                        }
                    }
                }
                ToolTip {
                    id: addSnToolTip
                    visible: addSnHover.hovered
                    text: "New Snippet"
                    delay: 400
                    contentItem: Text { text: addSnToolTip.text; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textPrimary }
                    background: Rectangle { color: AppTheme.surface; radius: 6; border.color: AppTheme.isDark ? "#3f3f46" : "#e2e8f0"; border.width: 1 }
                }
            }
        }
    }

    // ── Search bar ────────────────────────────────────────────────────────
    Rectangle {
        id: searchBar
        anchors.top: headerRow.bottom; anchors.topMargin: 12
        width: parent.width; height: 32; radius: 8; color: AppTheme.surface
        RowLayout {
            anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
            Text { text: "search"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.textSecondary }
            TextInput {
                id: searchInput; Layout.fillWidth: true; font.family: "Inter"; font.pixelSize: 11
                color: AppTheme.textPrimary; verticalAlignment: Qt.AlignVCenter; clip: true
                onTextChanged: libraryRoot.searchText = text.toLowerCase()
                Text {
                    text: "Search snippets…"; font: parent.font; color: AppTheme.textSecondary
                    visible: parent.text === "" && !parent.activeFocus; anchors.verticalCenter: parent.verticalCenter
                }
            }
        }
        MouseArea { anchors.fill: parent; onClicked: searchInput.forceActiveFocus() }
    }

    // ── Folder list ───────────────────────────────────────────────────────
    ListView {
        id: folderListView
        anchors { top: searchBar.bottom; topMargin: 16; bottom: parent.bottom; left: parent.left; right: parent.right }
        clip: true
        spacing: 6; model: folderListModel
        interactive: !libraryRoot.folderProxy.Drag.active && !libraryRoot.snippetProxy.Drag.active

        move: Transition {
            NumberAnimation { properties: "y"; duration: 250; easing.type: Easing.OutCubic }
        }
        displaced: Transition {
            NumberAnimation { properties: "y"; duration: 250; easing.type: Easing.OutCubic }
        }

        delegate: Item {
            id: folderRow
            width: folderListView.width
            property string fName: model.folderName || ""
            property bool fExpanded: model.isExpanded || false
            
            visible: {
                if (libraryRoot.searchText === "") return true
                if (fName.toLowerCase().indexOf(libraryRoot.searchText) >= 0) return true
                var sm = libraryRoot.snippetModelFor(fName)
                if (!sm) return false
                for (var i = 0; i < sm.count; i++) {
                    if (String(sm.get(i).name).toLowerCase().indexOf(libraryRoot.searchText) >= 0) return true
                    if (String(sm.get(i).content || "").toLowerCase().indexOf(libraryRoot.searchText) >= 0) return true
                }
                return false
            }
            height: visible ? (folderHeader.height + (snListView.visible ? snListView.contentHeight + (libraryRoot.realCount(fName) === 0 ? 26 : 0) + 4 : 0)) : 0
            Behavior on height { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }

            Rectangle {
                id: folderHeader
                width: parent.width; height: 40; radius: 12
                opacity: libraryRoot.draggingFolderName === fName ? 0 : 1
                
                // DropArea for cross-folder snippet drop + live animated preview
                DropArea {
                    id: dropArea
                    anchors.fill: parent; keys: ["snippetDrag"]

                    onEntered: {
                        var sn = libraryRoot.draggingSnippetName
                        var from = libraryRoot.draggingSnippetFolder
                        if (sn === "" || from === fName) return   // same folder — handled by snippet DropAreas

                        // Clear preview from a different folder (not this one)
                        if (libraryRoot.dragPreviewFolder !== "" && libraryRoot.dragPreviewFolder !== fName)
                            libraryRoot.clearDragPreview()

                        // Auto-expand so the user can see the snippet list
                        for (var i = 0; i < folderListModel.count; i++) {
                            if (folderListModel.get(i).folderName === fName) {
                                folderListModel.setProperty(i, "isExpanded", true); break
                            }
                        }

                        // Only insert preview if not already in this folder
                        if (libraryRoot.dragPreviewFolder !== fName) {
                            var sm = libraryRoot.snippetModelFor(fName)
                            if (sm) {
                                sm.append({ "name": "__drag_preview__" })
                                libraryRoot.dragPreviewFolder = fName
                            }
                        }
                        // No onExited — it fires when entering child snippet rows, don't clear there
                    }

                    onDropped: function(drop) {
                        var sn = libraryRoot.draggingSnippetName
                        var from = libraryRoot.draggingSnippetFolder
                        libraryRoot.clearDragPreview()
                        if (from !== fName && sn !== "") {
                            Qt.callLater(function() {
                                libraryRoot.moveSnippetBetweenFolders(sn, from, fName)
                            })
                        }
                        drop.accept()
                    }
                }
                
                property bool isDropTarget: dropArea.containsDrag && libraryRoot.draggingSnippetFolder !== fName

                color: isDropTarget ? Qt.darker(AppTheme.primary, 1.1) : 
                       (libraryRoot.selectedFolder === fName) ? AppTheme.primary : 
                       (fHover.hovered ? AppTheme.hoverBg : AppTheme.surface)
                
                HoverHandler { id: fHover }
                
                MouseArea {
                    id: folderDragArea
                    anchors.fill: parent
                    
                    property point startPos
                    property bool dragTriggered: false

                    onPressed: (mouse) => {
                        startPos = Qt.point(mouse.x, mouse.y)
                        dragTriggered = false
                    }
                    
                    onPositionChanged: (mouse) => {
                        if (!dragTriggered) {
                            var delta = Math.sqrt(Math.pow(mouse.x - startPos.x, 2) + Math.pow(mouse.y - startPos.y, 2))
                            if (delta > libraryRoot.dragThreshold) {
                                dragTriggered = true
                                var p = folderHeader.mapToItem(libraryRoot, mouse.x, mouse.y)
                                libraryRoot.folderProxy.x = p.x; libraryRoot.folderProxy.y = p.y
                                folderGhost.folderName = fName; folderGhost.visible = true
                                libraryRoot.draggingFolderName = fName
                                libraryRoot.folderProxy.Drag.active = true
                                libraryRoot.folderProxy.Drag.source = folderHeader
                            }
                        } else {
                            // Update ghost position
                            var pr = folderHeader.mapToItem(libraryRoot, mouse.x, mouse.y)
                            libraryRoot.folderProxy.x = pr.x; libraryRoot.folderProxy.y = pr.y

                            // Live reorder: find index under the cursor using indexAt()
                            var lp = folderHeader.mapToItem(folderListView, mouse.x, mouse.y)
                            var targetIdx = folderListView.indexAt(lp.x, lp.y + folderListView.contentY)
                            // Clamp to last item when dragged past the end
                            if (targetIdx < 0) targetIdx = folderListModel.count - 1

                            var fromIdx = -1
                            for (var i = 0; i < folderListModel.count; i++) {
                                if (folderListModel.get(i).folderName === libraryRoot.draggingFolderName) { fromIdx = i; break }
                            }
                            if (fromIdx !== -1 && fromIdx !== targetIdx) {
                                folderListModel.move(fromIdx, targetIdx, 1)
                            }
                        }
                    }
                    onReleased: (mouse) => {
                        if (dragTriggered) {
                            libraryRoot.folderProxy.Drag.drop();
                            libraryRoot.folderProxy.Drag.active = false
                            folderGhost.visible = false;
                            libraryRoot.draggingFolderName = ""
                            // Persist the new folder order immediately
                            var names = []
                            for (var i = 0; i < folderListModel.count; i++)
                                names.push(folderListModel.get(i).folderName)
                            snippetViewModel.reorderFolders(names)
                        } else {
                            model.isExpanded = !fExpanded;
                            libraryRoot.selectedFolder = fName
                        }
                    }
                    onCanceled: {
                        if (dragTriggered) {
                            libraryRoot.folderProxy.Drag.active = false;
                            folderGhost.visible = false;
                            libraryRoot.draggingFolderName = ""
                        }
                    }
                }

                // Right-click → folder context menu
                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.RightButton
                    onClicked: (mouse) => libraryRoot.showContextMenu(fName, "", mouse.x, mouse.y, folderHeader)
                }

                RowLayout {
                    anchors.fill: parent; anchors.margins: 12; spacing: 10
                    Text {
                        text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 16
                        color: libraryRoot.selectedFolder === fName ? "white" : (fHover.hovered ? (AppTheme.isDark ? "white" : "#475569") : AppTheme.textSecondary)
                        rotation: folderRow.fExpanded ? 0 : -90
                        Behavior on rotation { NumberAnimation { duration: 180 } }
                    }
                    Text { 
                        text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; 
                        color: libraryRoot.selectedFolder === fName ? "white" : (fHover.hovered ? (AppTheme.isDark ? "white" : "#475569") : AppTheme.textSecondary)
                    }
                    Text { 
                        Layout.fillWidth: true; text: fName; font.family: "Inter"; font.pixelSize: 11; font.bold: true; 
                        color: libraryRoot.selectedFolder === fName ? "white" : AppTheme.textPrimary 
                    }
                    Rectangle {
                        width: 22; height: 18; radius: 9
                        color: libraryRoot.selectedFolder === fName ? Qt.rgba(1,1,1,0.25) : AppTheme.hoverBg
                        Text {
                            anchors.centerIn: parent
                            text: libraryRoot.realCount(fName)
                            font.family: "Inter"; font.pixelSize: 10; font.bold: true
                            color: libraryRoot.selectedFolder === fName ? "white" : AppTheme.textSecondary
                        }
                    }
                }
            }

            // ── Empty-folder hint ──────────────────────────────────────────────
            Item {
                anchors.top: snListView.top
                width: parent.width; height: 26
                visible: folderRow.fExpanded && libraryRoot.realCount(fName) === 0
                Text {
                    anchors.centerIn: parent
                    text: "No snippets yet — right-click for options"
                    font.family: "Inter"; font.pixelSize: 10; color: AppTheme.textSecondary; opacity: 0.7
                }
            }

            // ── Snippet list (Nested ListView for reordering animations) ──────────
            ListView {
                id: snListView
                anchors.top: folderHeader.bottom; anchors.topMargin: 4
                width: parent.width; visible: folderRow.fExpanded; spacing: 4
                height: visible ? contentHeight : 0
                interactive: false // Parent ListView handles scrolling
                model: libraryRoot.snippetModelFor(fName)

                move: Transition {
                    NumberAnimation { properties: "y"; duration: 200; easing.type: Easing.OutCubic }
                }
                displaced: Transition {
                    NumberAnimation { properties: "y"; duration: 200; easing.type: Easing.OutCubic }
                }

                delegate: Item {
                    id: snRow
                    width: snListView.width
                    property string snName: model.name || ""
                    property string snContent: model.content || ""
                    property string previewText: snContent.split("\n")[0]
                    // Snippet visible-filter: always show the drag-preview placeholder
                    visible: snName === "__drag_preview__" ||
                             libraryRoot.searchText === "" ||
                             snName.toLowerCase().indexOf(libraryRoot.searchText) >= 0 ||
                             snContent.toLowerCase().indexOf(libraryRoot.searchText) >= 0
                    height: visible ? 34 : 0

                    DropArea {
                        anchors.fill: parent; keys: ["snippetDrag"]
                        onEntered: {
                            var sn = libraryRoot.draggingSnippetName
                            if (sn === "" || sn === snName) return

                            var isSameFolder = (libraryRoot.draggingSnippetFolder === fName)

                            if (isSameFolder) {
                                // ── In-folder reorder: move the real item live ──────────────
                                var sm = libraryRoot.snippetModelFor(fName)
                                var fromIdx = -1
                                for (var i = 0; i < sm.count; i++) {
                                    if (sm.get(i).name === sn) { fromIdx = i; break }
                                }
                                if (fromIdx !== -1 && fromIdx !== index) sm.move(fromIdx, index, 1)
                            } else {
                                // ── Cross-folder: move the __drag_preview__ placeholder live ─
                                // Clear preview from a different folder first
                                if (libraryRoot.dragPreviewFolder !== "" && libraryRoot.dragPreviewFolder !== fName)
                                    libraryRoot.clearDragPreview()

                                // Auto-expand if needed
                                for (var i = 0; i < folderListModel.count; i++) {
                                    if (folderListModel.get(i).folderName === fName) {
                                        folderListModel.setProperty(i, "isExpanded", true); break
                                    }
                                }

                                var tsm = libraryRoot.snippetModelFor(fName)
                                if (!tsm) return

                                if (libraryRoot.dragPreviewFolder === fName) {
                                    // Preview already here — reposition it to this index
                                    var previewIdx = -1
                                    for (var i = 0; i < tsm.count; i++) {
                                        if (tsm.get(i).name === "__drag_preview__") { previewIdx = i; break }
                                    }
                                    if (previewIdx !== -1 && previewIdx !== index)
                                        tsm.move(previewIdx, index, 1)
                                } else {
                                    // No preview yet — insert at this position
                                    tsm.insert(index, { "name": "__drag_preview__" })
                                    libraryRoot.dragPreviewFolder = fName
                                }
                            }
                        }
                        onDropped: function(drop) {
                            var sn = libraryRoot.draggingSnippetName
                            var from = libraryRoot.draggingSnippetFolder
                            if (from !== fName && sn !== "") {
                                // Cross-folder drop on a specific snippet position
                                libraryRoot.clearDragPreview()
                                Qt.callLater(function() {
                                    libraryRoot.moveSnippetBetweenFolders(sn, from, fName)
                                })
                                drop.accept()
                            }
                            // Same-folder drops: no action needed, order already updated live
                        }
                    }

                    Rectangle {
                        id: snRect
                        anchors.fill: parent; radius: 10

                        // Drag-preview placeholder renders as a dashed insertion ghost
                        property bool isPreview: snName === "__drag_preview__"

                        color: isPreview ? "transparent" :
                               (libraryRoot.selectedAbbrev === snName && libraryRoot.selectedFolder === fName) ? AppTheme.hoverBg :
                               (snHover.hovered ? AppTheme.hoverBg : AppTheme.surface)

                        border.color: "transparent"
                        border.width: 0
                        opacity: isPreview ? 0 :
                                 (libraryRoot.draggingSnippetName === snName && libraryRoot.draggingSnippetFolder === fName) ? 0 : 1

                        HoverHandler { id: snHover; enabled: !snRect.isPreview }

                        MouseArea {
                            anchors.fill: parent
                            enabled: !snRect.isPreview   // preview slot is not interactive
                            
                            property point startPos
                            property bool dragTriggered: false

                            onPressed: (mouse) => {
                                startPos = Qt.point(mouse.x, mouse.y)
                                dragTriggered = false
                            }

                            onPositionChanged: (mouse) => {
                                if (!dragTriggered) {
                                    var delta = Math.sqrt(Math.pow(mouse.x - startPos.x, 2) + Math.pow(mouse.y - startPos.y, 2))
                                    if (delta > libraryRoot.dragThreshold) {
                                        dragTriggered = true
                                        var p = snRect.mapToItem(libraryRoot, mouse.x, mouse.y)
                                        libraryRoot.snippetProxy.x = p.x; libraryRoot.snippetProxy.y = p.y
                                        snippetGhost.snippetName = snName; snippetGhost.visible = true
                                        libraryRoot.draggingSnippetName = snName; libraryRoot.draggingSnippetFolder = fName
                                        libraryRoot.snippetProxy.Drag.active = true
                                        libraryRoot.snippetProxy.Drag.source = snRect
                                    }
                                } else {
                                    var p = snRect.mapToItem(libraryRoot, mouse.x, mouse.y)
                                    libraryRoot.snippetProxy.x = p.x; libraryRoot.snippetProxy.y = p.y
                                }
                            }
                            onReleased: {
                                if (dragTriggered) {
                                    if (libraryRoot && libraryRoot.snippetProxy) {
                                        libraryRoot.snippetProxy.Drag.drop()
                                        libraryRoot.snippetProxy.Drag.active = false
                                    }
                                    snippetGhost.visible = false
                                    // Clear any left-over cross-folder preview
                                    libraryRoot.clearDragPreview()
                                    // Persist the new snippet order for this folder
                                    var sm2 = libraryRoot.snippetModelFor(fName)
                                    if (sm2) {
                                        var snames = []
                                        for (var i = 0; i < sm2.count; i++)
                                            snames.push(sm2.get(i).name)
                                        snippetViewModel.reorderSnippets(fName, snames)
                                    }
                                    libraryRoot.draggingSnippetName = ""; libraryRoot.draggingSnippetFolder = ""
                                } else {
                                    libraryRoot.selectedFolder = fName; libraryRoot.selectedAbbrev = snName
                                    snippetViewModel.selectSnippet(fName, snName)
                                }
                            }
                            onCanceled: {
                                if (dragTriggered) {
                                    if (libraryRoot && libraryRoot.snippetProxy) {
                                        libraryRoot.snippetProxy.Drag.active = false
                                    }
                                    snippetGhost.visible = false
                                    libraryRoot.clearDragPreview()
                                    libraryRoot.draggingSnippetName = ""; libraryRoot.draggingSnippetFolder = ""
                                }
                            }
                        }

                        // Right-click → snippet context menu
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.RightButton
                            enabled: !snRect.isPreview
                            onClicked: (mouse) => libraryRoot.showContextMenu(fName, snName, mouse.x, mouse.y, snRect)
                        }

                        RowLayout {
                            anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                            Text {
                                text: snName; font.family: "Courier New"; font.pixelSize: 12; font.bold: true
                                color: AppTheme.textPrimary; elide: Text.ElideRight
                            }
                            Text {
                                visible: !snRect.isPreview && snContent !== ""
                                text: "\u2192"  // → arrow separator
                                font.family: "Inter"; font.pixelSize: 10
                                color: AppTheme.primary; opacity: 0.6
                            }
                            Text {
                                Layout.fillWidth: true; visible: !snRect.isPreview && snContent !== ""
                                text: snRow.previewText
                                font.family: "Inter"; font.pixelSize: 10
                                color: AppTheme.textPrimary; elide: Text.ElideRight
                            }
                        }
                    }
                }
            }
        }
    }
}
