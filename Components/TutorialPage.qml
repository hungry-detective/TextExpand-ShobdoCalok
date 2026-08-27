import QtQuick
import QtQuick.Layouts

Item {
    id: root

    property int currentStep: 0
    readonly property int stepCount: 3
    readonly property var stepTitles: ["Create a Snippet", "Type & Expand", "Dynamic Snippets"]

    property string instruction: ""
    property string instructionDetail: ""
    readonly property color successGreen: AppTheme.isDark ? "#34d399" : "#10b981"
    readonly property color successText: AppTheme.isDark ? "#ffffff" : "#10b981"

    // ── Step 0 state ─────────────────────────────────────────────────────────
    property string abbrevText: ""
    property string contentText: ""
    property bool folderSelected: false
    property bool saved: false
    property string savedDemoText: ""
    property bool savedDemoExpanded: false

    // ── Step 1 state ─────────────────────────────────────────────────────────
    property int currentExample: 0
    readonly property var examples: [
        { name: "Email", icon: "mail" },
        { name: "WhatsApp", icon: "chat" },
        { name: "Messenger", icon: "forum" }
    ]
    property string exTyped: ""
    property bool exExpanded: false
    property bool exExpanded1: false
    property bool exExpanded2: false

    // ── Step 2 state ─────────────────────────────────────────────────────────
    property int dynSubStep: 0
    readonly property var dynStepTitles: ["Date & Time", "Fill-in Fields", "Cursor"]
    property bool dynDateResolved: false
    property bool dynTimeResolved: false
    property bool dynFieldResolved: false
    property string dynFieldValue: ""
    property bool dynFieldPopupOpen: false
    property string dynFieldTriggerText: ""
    property bool dynFieldInserted: false
    property string dynCursorText: ""
    property string dynEditorText: ""
    property bool dynCursorPlaced: false
    property bool dynCursorInserted: false
    property bool dynCursorReplaced: false
    property string dynResultTyped: ""
    property bool dynResultExpanded: false
    property string dynFieldEditorText: ""
    property string dynCursorEditorText: ""
    property string dynCursorTriggerText: ""
    property string dynCursorAppText: ""
    property string dynCursorAppExpandedText: ""
    property string dynCursorAppExpandedPrefix: ""
    property string dynCursorAppExpandedSuffix: ""
    property bool dynCursorAppExpanded: false
    property string dynFieldAppText: ""
    property string dynFieldAppExpandedText: ""
    property bool dynFieldAppExpanded: false
    property string dynResult: "Meeting on " + (dynDateResolved ? "1 August 2026" : "{date}") + " at " + (dynTimeResolved ? "02:30 PM" : "{time}")

    // ── Animation UI state ────────────────────────────────────────────────────
    property string typingTarget: ""
    property string spotlight: ""
    property bool animRunning: false

    // ── Mouse cursor mapping ──────────────────────────────────────────────────
    property var cursorMap: ({})
    property real cursorTargetX: 0
    property real cursorTargetY: 0
    property bool cursorVisible: false

    onSpotlightChanged: {
        if (clickAnim.running) clickAnim.stop()
        clickRipple.opacity = 0
        if (clickDelayTimer.running) clickDelayTimer.stop()
        if (spotlight === "") { cursorVisible = false; return }
        var el = cursorMap[spotlight]
        if (!el) { cursorVisible = false; return }
        posTimer.targetEl = el
        posTimer.restart()
    }

    function isClickSpotlight(s) {
        return s === "folder" || s === "save" || s === "savedExpand" || s === "exExpand"
            || s === "date" || s === "time" || s === "dynFieldPopup" || s === "dynFieldResult" || s === "cursor"
    }

    Timer {
        id: posTimer
        interval: 60
        repeat: false
        property var targetEl: null
        onTriggered: {
            if (!targetEl) return
            recomputeCursor()
            root.cursorVisible = true
            if (isClickSpotlight(root.spotlight)) clickDelayTimer.start()
        }
    }

    Timer {
        id: clickDelayTimer
        interval: 480
        repeat: false
        onTriggered: clickAnim.restart()
    }

    function recomputeCursor() {
        var el = posTimer.targetEl
        if (!el) return
        var p = el.mapToItem(root, Qt.point(el.width / 2, el.height / 2))
        root.cursorTargetX = p.x
        root.cursorTargetY = p.y
    }

    onWidthChanged: if (root.cursorVisible && posTimer.targetEl) recomputeCursor()
    onHeightChanged: if (root.cursorVisible && posTimer.targetEl) recomputeCursor()

    // ── Animation engine ─────────────────────────────────────────────────────
    Timer {
        id: animTimer
        interval: 34
        repeat: true
        property int pos: 0
        property int idx: 0
        property var script: []
        property int pauseLeft: -1
        onTriggered: {
            if (idx >= script.length) {
                root.animRunning = false
                root.typingTarget = ""
                root.spotlight = ""
                animTimer.stop()
                advanceTimer.start()
                return
            }
            root.animRunning = true
            var f = script[idx]
            if (f.type === "type") {
                root.typingTarget = f.target
                root.spotlight = f.target
                if (f.text) root.instruction = f.text
                if (f.detail) root.instructionDetail = f.detail
                if (pos < f.textLen) {
                    var ch = f.chars.charAt(pos)
                    if (f.target === "abbrev") root.abbrevText += ch
                    else if (f.target === "content") root.contentText += ch
                    else if (f.target === "savedDemo") root.savedDemoText += ch
                    else if (f.target === "ex") root.exTyped += ch
                    else if (f.target === "dynField") root.dynFieldValue += ch
                    else if (f.target === "dynCursor") root.dynCursorText += ch
                    else if (f.target === "dynEditor") root.dynEditorText += ch
                    else if (f.target === "dynTrigger") root.dynFieldTriggerText += ch
                    else if (f.target === "dynExpand") root.dynResultTyped += ch
                    else if (f.target === "dynFieldEditor") root.dynFieldEditorText += ch
                    else if (f.target === "dynCursorEditor") root.dynCursorEditorText += ch
                    else if (f.target === "dynCursorTrigger") root.dynCursorTriggerText += ch
                    else if (f.target === "dynCursorApp") root.dynCursorAppText += ch
                    else if (f.target === "dynCursorAppExpand") root.dynCursorAppExpandedText += ch
                    else if (f.target === "dynCursorAppPrefix") root.dynCursorAppExpandedPrefix += ch
                    else if (f.target === "dynCursorAppSuffix") root.dynCursorAppExpandedSuffix += ch
                    else if (f.target === "dynFieldApp") root.dynFieldAppText += ch
                    else if (f.target === "dynFieldAppExpand") root.dynFieldAppExpandedText += ch
                    pos++
                } else { pos = 0; idx++ }
            } else if (f.type === "pause") {
                if (f.text) root.instruction = f.text
                if (f.detail) root.instructionDetail = f.detail
                if (f.spotlight) root.spotlight = f.spotlight
                if (f.typing !== undefined) root.typingTarget = f.typing
                if (pauseLeft < 0) pauseLeft = f.ms
                pauseLeft -= interval
                if (pauseLeft <= 0) { pauseLeft = -1; idx++ }
            } else if (f.type === "action") {
                if (f.text) root.instruction = f.text
                if (f.detail) root.instructionDetail = f.detail
                if (f.spotlight) root.spotlight = f.spotlight
                if (f.target === "folder") root.folderSelected = true
                else if (f.target === "save") root.saved = true
                else if (f.target === "savedExpand") root.savedDemoExpanded = true
                else if (f.target === "exExpand") root.exExpanded = true
                else if (f.target === "exExpand1") { root.exExpanded1 = true; root.exTyped = "" }
                else if (f.target === "exExpand2") { root.exExpanded2 = true; root.exTyped = "" }
                else if (f.target === "date") root.dynDateResolved = true
                else if (f.target === "time") root.dynTimeResolved = true
                else if (f.target === "addDate") root.dynEditorText += "{date}"
                else if (f.target === "addTime") root.dynEditorText += "{time}"
                else if (f.target === "openPopup") root.dynFieldPopupOpen = true
                else if (f.target === "insertField") { root.dynFieldInserted = true; root.dynFieldEditorText += "{field:Label}" }
                else if (f.target === "submitField") {
                    root.dynFieldEditorText = root.dynFieldEditorText.replace("{field:Label}", root.dynFieldValue)
                    root.dynFieldPopupOpen = false; root.dynFieldResolved = true
                }
                else if (f.target === "field") root.dynFieldResolved = true
                else if (f.target === "insertCursor") { root.dynCursorInserted = true; root.dynCursorPlaced = true; root.dynCursorEditorText += "{cursor}"; root.typingTarget = "" }
                else if (f.target === "cursor") root.dynCursorPlaced = true
                else if (f.target === "cursorAppExpand") { root.dynCursorAppExpanded = true; root.dynCursorAppText = ""; root.dynCursorAppExpandedText = ""; root.dynCursorAppExpandedPrefix = ""; root.dynCursorAppExpandedSuffix = "" }
                else if (f.target === "fieldAppExpand") { root.dynFieldAppExpanded = true; root.dynFieldAppText = ""; root.dynFieldAppExpandedText = "" }
                else if (f.target === "dynExpand") { root.dynResultExpanded = true; root.dynResultTyped = "" }
                idx++
            }
        }
    }

    Timer {
        id: advanceTimer
        interval: 1600
        repeat: false
        onTriggered: root.loopCurrentStep()
    }

    function loopCurrentStep() {
        if (currentStep === 0) {
            startDemo()
        } else if (currentStep === 1) {
            startExample()
        } else {
            startDemo()
        }
    }

    function resetDemo() {
        animTimer.stop()
        advanceTimer.stop()
        abbrevText = ""; contentText = ""
        folderSelected = false; saved = false
        savedDemoText = ""; savedDemoExpanded = false
        exTyped = ""; exExpanded = false; exExpanded1 = false; exExpanded2 = false
        dynDateResolved = false; dynTimeResolved = false
        dynFieldResolved = false; dynFieldValue = ""
        dynFieldPopupOpen = false; dynFieldTriggerText = ""; dynFieldInserted = false
        dynCursorText = ""; dynCursorPlaced = false; dynCursorInserted = false
        dynCursorAppText = ""; dynCursorAppExpandedText = ""; dynCursorAppExpandedPrefix = ""; dynCursorAppExpandedSuffix = ""; dynCursorAppExpanded = false
        dynFieldAppText = ""; dynFieldAppExpandedText = ""; dynFieldAppExpanded = false
        dynEditorText = ""
        dynResultTyped = ""; dynResultExpanded = false
        dynFieldEditorText = ""; dynCursorEditorText = ""; dynCursorTriggerText = ""
        typingTarget = ""; spotlight = ""
        instruction = ""; instructionDetail = ""
        animTimer.pos = 0; animTimer.idx = 0; animTimer.pauseLeft = -1
    }

    function startDemo() {
        resetDemo()
        if (currentStep === 0) {
            animTimer.script = [
                { type: "pause", spotlight: "folder",
                  text: "Step 1 — Choose a folder",
                  detail: "Click the folder dropdown to organize your snippets.",
                  ms: 2000 },
                { type: "action", target: "folder", spotlight: "folder",
                  text: "Folder selected!",
                  detail: "Your snippet is now in 'General'." },
                { type: "pause", spotlight: "folder", ms: 1000 },

                { type: "pause", spotlight: "abbrev",
                  text: "Step 2 — Type a trigger word",
                  detail: "This is the shortcut you'll type to expand text.",
                  ms: 2000 },
                { type: "type", target: "abbrev", chars: ".hello",
                  textLen: 6,
                  text: "Typing .hello…",
                  detail: "Try any word starting with a dot." },
                { type: "pause", spotlight: "abbrev", ms: 1200 },

                { type: "pause", spotlight: "content",
                  text: "Step 3 — Write the expansion",
                  detail: "This text appears when you type the trigger.",
                  ms: 2000 },
                { type: "type", target: "content", chars: "Hello, how are you?",
                  textLen: 19,
                  text: "Typing the expansion…",
                  detail: "This is what gets inserted." },
                { type: "pause", spotlight: "content", ms: 1200 },

                { type: "pause", spotlight: "save",
                  text: "Step 4 — Save the snippet",
                  detail: "Click the save button.",
                  ms: 2000 },
                { type: "action", target: "save", spotlight: "save",
                  text: "Saved!",
                  detail: "Your snippet is ready to use." },
                { type: "pause", spotlight: "save", ms: 1200 },

                { type: "pause", spotlight: "savedDemo",
                  text: "Step 5 — Try it out!",
                  detail: "Type .hello anywhere and it expands.",
                  ms: 2000 },
                { type: "type", target: "savedDemo", chars: ".hello",
                  textLen: 6,
                  text: "Typing .hello…",
                  detail: "Watch the magic happen." },
                { type: "pause", spotlight: "savedDemo", ms: 800 },
                { type: "action", target: "savedExpand", spotlight: "savedExpand",
                  text: "Expanded!",
                  detail: ".hello became 'Hello, how are you?'" },
                { type: "pause", spotlight: "savedExpand", ms: 2500 }
            ]
        } else if (currentStep === 1) {
            startExample()
            return
        } else {
            startDynStep()
            return
        }
        animTimer.start()
    }

    function startDynStep() {
        resetDemo()
        if (dynSubStep === 0) {
            cursorMap["dynTrigger"] = dynTriggerRect0
        } else {
            cursorMap["dynTrigger"] = dynTriggerRect
        }
        if (dynSubStep === 0) {
            animTimer.script = [
                { type: "pause", spotlight: "dynTrigger",
                  text: "Step 1 — Type the trigger word",
                  detail: "Type .mtg to start the snippet.",
                  ms: 2000 },
                { type: "type", target: "dynTrigger", chars: ".mtg",
                  textLen: 4,
                  text: "Typing '.mtg'…",
                  detail: "The trigger word opens the snippet." },
                { type: "pause", spotlight: "dynTrigger", ms: 1200 },

                { type: "action", target: "openPopup", spotlight: "dynEditor",
                  text: "Snippet opened!",
                  detail: "Now type your message with auto-fill words.",
                  ms: 1500 },

                { type: "pause", spotlight: "dynEditor", typing: "dynEditor",
                  text: "Step 2 — Type your message",
                  detail: "Type the start of your message.",
                  ms: 2000 },
                { type: "type", target: "dynEditor", chars: "Meeting on ",
                  textLen: 11,
                  text: "Typing 'Meeting on…'",
                  detail: "Now we'll add today's date." },
                { type: "pause", spotlight: "dynEditor", ms: 1000 },

                { type: "pause", spotlight: "date",
                  text: "Step 3 — Click DATE",
                  detail: "It adds {date}, which becomes today's date.",
                  ms: 2000 },
                { type: "action", target: "addDate", spotlight: "date",
                  text: "{date} added!",
                  detail: "{date} will turn into today's date." },
                { type: "pause", spotlight: "dynEditor", ms: 1200 },

                { type: "type", target: "dynEditor", chars: " at ",
                  textLen: 4,
                  text: "Typing ' at '…",
                  detail: "Now we'll add the current time." },
                { type: "pause", spotlight: "dynEditor", ms: 800 },

                { type: "pause", spotlight: "time",
                  text: "Step 4 — Click TIME",
                  detail: "It adds {time}, which becomes the current time.",
                  ms: 2000 },
                { type: "action", target: "addTime", spotlight: "time",
                  text: "{time} added!",
                  detail: "{time} will turn into the current time." },
                { type: "pause", spotlight: "dynEditor", ms: 1200 },

                { type: "action", target: "date", spotlight: "dynEditor",
                  text: "Now they fill themselves in!",
                  detail: "{date} became '1 August 2026', {time} became '02:30 PM'." },
                { type: "action", target: "time", spotlight: "dynEditor",
                  text: "Done!",
                  detail: "Every time you type .mtg, the date & time stay current." },
                { type: "pause", spotlight: "dynEditor", ms: 1500 },

                { type: "pause", spotlight: "dynExpand",
                  text: "Step 5 — Use the shortcut",
                  detail: "Type .mtg and it expands into the whole message.",
                  typing: "dynExpand", ms: 2000 },
                { type: "type", target: "dynExpand", chars: ".mtg",
                  textLen: 4,
                  text: "Typing '.mtg'…",
                  detail: "The trigger word expands instantly." },
                { type: "pause", spotlight: "dynExpand", ms: 700 },
                { type: "action", target: "dynExpand", spotlight: "dynExpand",
                  text: "Expanded!",
                  detail: ".mtg became the full message with live date & time." },
                { type: "pause", spotlight: "dynExpand", ms: 2500 }
            ]
        } else if (dynSubStep === 1) {
            animTimer.script = [
                { type: "pause", spotlight: "dynTrigger", typing: "dynTrigger",
                  text: "Step 1 — Type the trigger word",
                  detail: "Type .price to start a fill-in snippet.",
                  ms: 2000 },
                { type: "type", target: "dynTrigger", chars: ".price",
                  textLen: 6,
                  text: "Typing '.price'…",
                  detail: "The trigger word opens the fill-in form." },
                { type: "pause", spotlight: "dynTrigger", ms: 1000 },

                { type: "pause", spotlight: "dynFieldEditor", typing: "dynFieldEditor",
                  text: "Step 2 — Type the expansion word",
                  detail: "The message appears in the editor.",
                  ms: 2000 },
                { type: "type", target: "dynFieldEditor", chars: "The price of the product is ",
                  textLen: 29,
                  text: "Typing 'The price of the product…'",
                  detail: "Now we'll add a dynamic field." },
                { type: "pause", spotlight: "dynFieldEditor", ms: 1000 },

                { type: "pause", spotlight: "dynFieldBtn",
                  text: "Step 3 — Click FIELD",
                  detail: "Click FIELD to insert {field:Label} into the snippet.",
                  ms: 2000 },
                { type: "action", target: "insertField", spotlight: "dynFieldEditor",
                  text: "Field inserted!",
                  detail: "{field:Label} marks where your value will go.",
                  ms: 1500 },

                { type: "action", target: "openPopup", spotlight: "dynFieldPopup",
                  text: "Form opened!",
                  detail: "Type the value in the highlighted box.",
                  ms: 1500 },

                { type: "pause", spotlight: "dynFieldPopup",
                  text: "Step 4 — Type the value",
                  detail: "Type the price into the input box.",
                  ms: 2000 },
                { type: "type", target: "dynField", chars: "$49",
                  textLen: 3,
                  text: "Typing '$49'…",
                  detail: "This value replaces {field:Label}." },
                { type: "pause", spotlight: "dynField", ms: 800 },

                { type: "action", target: "submitField", spotlight: "dynFieldResult",
                  text: "Pasted right!",
                  detail: "$49 replaced {field:Label} in the snippet.",
                  ms: 2500 },

                { type: "pause", spotlight: "dynFieldApp", typing: "dynFieldApp",
                  text: "Step 5 — Use it in any other app",
                  detail: "Type the trigger word in any other app — like a chat or email.",
                  ms: 2000 },
                { type: "type", target: "dynFieldApp", chars: ".price",
                  textLen: 6,
                  text: "Typing '.price'…",
                  detail: "The trigger word works everywhere, not just here." },
                { type: "pause", spotlight: "dynFieldApp", ms: 800 },

                { type: "action", target: "fieldAppExpand", spotlight: "dynFieldApp",
                  text: "Expanding…",
                  detail: "The price is filled right into the line.",
                  ms: 800 },
                { type: "pause", spotlight: "dynFieldApp", typing: "dynFieldAppExpand",
                  text: "Typing the expansion…",
                  detail: "Your line appears with the price already filled in.",
                  ms: 1000 },
                { type: "type", target: "dynFieldAppExpand", chars: "The price of the product is $49.",
                  textLen: 33,
                  text: "Expanded!",
                  detail: "The field was filled automatically — ready to send.",
                  ms: 2500 }
            ]
        } else {
            animTimer.script = [
                { type: "pause", spotlight: "dynCursorTrigger", typing: "dynCursorTrigger",
                  text: "Step 1 — Type the trigger word",
                  detail: "Type .cursor to start a snippet with a cursor.",
                  ms: 2000 },
                { type: "type", target: "dynCursorTrigger", chars: ".cursor",
                  textLen: 7,
                  text: "Typing '.cursor'…",
                  detail: "The trigger word opens the cursor snippet." },
                { type: "pause", spotlight: "dynCursorTrigger", ms: 1000 },

                { type: "pause", spotlight: "dynCursorEditor", typing: "dynCursorEditor",
                  text: "Step 2 — Type the first part",
                  detail: "The words before the cursor appear in the editor.",
                  ms: 2000 },
                { type: "type", target: "dynCursorEditor", chars: "Hi",
                  textLen: 2,
                  text: "Typing 'Hi'…",
                  detail: "Now we'll place the cursor right in the middle." },
                { type: "pause", spotlight: "dynCursorEditor", ms: 800 },

                { type: "pause", spotlight: "dynCursorBtn",
                  text: "Step 3 — Click CURSOR",
                  detail: "Click CURSOR to put the cursor mid-sentence.",
                  ms: 2000 },
                { type: "action", target: "insertCursor", spotlight: "dynCursorEditor",
                  text: "Cursor is there!",
                  detail: "The {cursor} marker sits in the middle and stays there.",
                  ms: 1500 },

                { type: "pause", spotlight: "dynCursorEditor", typing: "dynCursorEditor",
                  text: "Step 4 — Type the rest",
                  detail: "Finish your sentence after the cursor.",
                  ms: 1500 },
                { type: "type", target: "dynCursorEditor", chars: " How are you today",
                  textLen: 18,
                  text: "Typing 'How are you today…'",
                  detail: "Now the line reads: Hi {cursor} How are you today" },
                { type: "pause", spotlight: "dynCursorEditor", ms: 1000 },

                { type: "pause", spotlight: "dynCursorApp", typing: "dynCursorApp",
                  text: "Step 5 — Use it in any other app",
                  detail: "Type the trigger word in any other app — like a chat or email.",
                  ms: 2000 },
                { type: "type", target: "dynCursorApp", chars: ".cursor",
                  textLen: 7,
                  text: "Typing '.cursor'…",
                  detail: "The trigger word works everywhere, not just here." },
                { type: "pause", spotlight: "dynCursorApp", ms: 800 },

                { type: "action", target: "cursorAppExpand", spotlight: "dynCursorApp",
                  text: "Expanding…",
                  detail: "The cursor lands right in the middle of your sentence.",
                  ms: 800 },
                { type: "pause", spotlight: "dynCursorApp", typing: "dynCursorAppPrefix",
                  text: "Expanding…",
                  detail: "The first part of your sentence appears.",
                  ms: 1000 },
                { type: "type", target: "dynCursorAppPrefix", chars: "Hi",
                  textLen: 2,
                  text: "Expanding…",
                  detail: "The cursor blinks mid-sentence." },
                { type: "pause", spotlight: "dynCursorApp", typing: "",
                  text: "Cursor is ready",
                  detail: "The blinking cursor sits in the middle — type your name there.",
                  ms: 1500 },
                { type: "pause", spotlight: "dynCursorApp", typing: "dynCursorAppSuffix",
                  text: "Finishing the sentence…",
                  detail: "The rest of your sentence follows the cursor.",
                  ms: 900 },
                { type: "type", target: "dynCursorAppSuffix", chars: "How are you today",
                  textLen: 17,
                  text: "Expanded!",
                  detail: "Hi [cursor] How are you today — just fill in the middle." },
                { type: "pause", spotlight: "dynCursorApp", typing: "", ms: 2500 }
            ]
        }
        animTimer.start()
    }

    function startExample() {
        resetDemo()
        var names = [".email", ".hello", ".ttyl"]
        var name = names[currentExample]
        var expandTexts = ["Email filled ✓", "Message sent ✓", "Talk to you later!"]
        if (currentExample === 1) {
            animTimer.script = [
                { type: "pause",
                  text: "Type .omw in the WhatsApp input",
                  detail: "Watch it expand into a message.",
                  spotlight: "ex", typing: "ex",
                  ms: 2000 },
                { type: "type", target: "ex", chars: ".omw",
                  textLen: 4,
                  text: "Typing .omw…",
                  detail: "The trigger word is being typed." },
                { type: "pause", spotlight: "ex", ms: 800 },
                { type: "action", target: "exExpand1", spotlight: "exMsg1",
                  text: "Expanded!",
                  detail: ".omw became 'On my way!' and sent automatically." },
                { type: "pause", spotlight: "exMsg1", ms: 2200 },
                { type: "pause", spotlight: "ex",
                  text: "Now type .hello",
                  detail: "It becomes another message.",
                  ms: 1500 },
                { type: "type", target: "ex", chars: ".hello",
                  textLen: 6,
                  text: "Typing .hello…",
                  detail: "The trigger word is being typed." },
                { type: "pause", spotlight: "ex", ms: 800 },
                { type: "action", target: "exExpand2", spotlight: "exMsg2",
                  text: "Expanded!",
                  detail: ".hello became 'Hello! How are you?' and sent automatically." },
                { type: "pause", spotlight: "exMsg2", ms: 2500 }
            ]
            animTimer.start()
            return
        }
        animTimer.script = [
            { type: "pause",
              text: "Type " + name + " in the " + examples[currentExample].name + " input",
              detail: "Watch it expand instantly.",
              spotlight: "ex", typing: "ex",
              ms: 2000 },
            { type: "type", target: "ex", chars: name,
              textLen: name.length,
              text: "Typing " + name + "…",
              detail: "The trigger word is being typed." },
            { type: "pause", spotlight: "ex", ms: 800 },
            { type: "action", target: "exExpand", spotlight: (currentExample === 2) ? "exMsg3" : "exExpand",
              text: "Expanded!",
              detail: expandTexts[currentExample] + (currentExample === 2 ? " and sent automatically." : "") },
            { type: "pause", spotlight: (currentExample === 2) ? "exMsg3" : "exExpand", ms: 2500 }
        ]
        animTimer.start()
    }

    Component {
        id: caretComp
        Rectangle {
            width: 3; height: 18; radius: 1.5; color: AppTheme.primary
            SequentialAnimation on opacity {
                loops: Animation.Infinite
                running: parent.visible
                NumberAnimation { to: 0; duration: 300 }
                NumberAnimation { to: 1; duration: 300 }
            }
        }
    }

    Component {
        id: spotGlow
        Rectangle {
            anchors.fill: parent
            radius: parent.radius + 4
            color: "transparent"
            border.color: AppTheme.primary
            border.width: 2
            opacity: 0.9
            SequentialAnimation on opacity {
                loops: Animation.Infinite
                running: parent.visible
                NumberAnimation { to: 0.35; duration: 380 }
                NumberAnimation { to: 0.9; duration: 380 }
            }
        }
    }

    SequentialAnimation {
        id: popAnim
        running: false
        property var animTarget: null
        NumberAnimation {
            target: popAnim.animTarget
            property: "scale"
            from: 1; to: 1.12
            duration: 110
            easing.type: Easing.OutQuad
        }
        NumberAnimation {
            target: popAnim.animTarget
            property: "scale"
            to: 1
            duration: 190
            easing.type: Easing.OutBack
        }
    }

    function popSuccess(target) {
        if (!target) return
        popAnim.animTarget = target
        target.scale = 1
        popAnim.restart()
    }

    // ── Mouse cursor animation ────────────────────────────────────────────────
    Item {
        id: mouseCursor
        x: root.cursorTargetX - 1
        y: root.cursorTargetY - 1
        z: 1000
        visible: root.cursorVisible
        width: 22; height: 26

        Behavior on x { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
        Behavior on y { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }

        Image {
            source: "cursor-pointer.svg"
            anchors.fill: parent
            fillMode: Image.PreserveAspectFit
            smooth: true
        }
    }

    // ── Click ripple (small dot at cursor tip) ────────────────────────────────
    Rectangle {
        id: clickRipple
        x: root.cursorTargetX - 5
        y: root.cursorTargetY - 5
        z: 1001
        width: 10; height: 10; radius: 5
        color: AppTheme.primary
        opacity: 0
    }

    SequentialAnimation {
        id: clickAnim
        running: false
        PropertyAnimation { target: clickRipple; property: "opacity"; from: 0.9; to: 0; duration: 350 }
        ScriptAction { script: { clickRipple.opacity = 0 } }
    }

    Component.onCompleted: {
        cursorMap = {
            "folder": folderRect,
            "abbrev": abbrevRect,
            "content": contentEditor,
            "save": saveBtn,
            "savedDemo": savedDemoBox,
            "savedExpand": savedDemoBox,
            "ex": exInputRect,
            "exExpand": exSendBtn,
            "exMsg1": exMsg1,
            "exMsg2": exMsg2,
            "exMsg3": exMsg3,
            "date": dynDateBtn,
            "time": dynTimeBtn,
            "dynEditor": dynEditorRect,
            "dynTrigger": dynTriggerRect,
            "dynFieldEditor": dynFieldEditorRect,
            "dynCursorEditor": dynCursorEditorRect,
            "dynCursorTrigger": dynCursorTriggerRect,
            "dynCursorBtn": dynCursorBtn,
            "dynFieldPopup": dynFieldPopup,
            "dynFieldBtn": dynFieldBtn,
            "dynFieldResult": dynFieldBubble,
            "dynExpand": dynResultBox,
            "field": dynFieldInput,
            "cursor": dynCursorEditorRect,
            "dynCursor": dynCursorEditorRect,
            "dynCursorApp": dynCursorAppRect,
            "dynFieldApp": dynFieldAppRect
        }
        startDemo()
    }
    onCurrentStepChanged: startDemo()
    onCurrentExampleChanged: startExample()
    onDynSubStepChanged: startDynStep()

    onSavedChanged: if (root.saved) popSuccess(saveBtn)
    onExExpandedChanged: if (root.exExpanded && root.currentExample !== 2) popSuccess(exSendBtn)
    onDynFieldResolvedChanged: if (root.dynFieldResolved) popSuccess(dynFieldInput)
    onDynCursorPlacedChanged: if (root.dynCursorPlaced) popSuccess(dynCursorEditorRect)
    onDynDateResolvedChanged: maybeResolvedPop()
    onDynTimeResolvedChanged: maybeResolvedPop()
    function maybeResolvedPop() {
        if (root.dynDateResolved && root.dynTimeResolved) popSuccess(watchBtn)
    }

    // ── Layout ───────────────────────────────────────────────────────────────
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 16

        Text {
            text: "Learn Shobdo Calok"
            font.family: "Inter"; font.pixelSize: 22; font.bold: true
            color: AppTheme.textPrimary
        }

        // Step selector cards
        RowLayout {
            Layout.fillWidth: true; spacing: 10
            Repeater {
                model: 3
                delegate: Rectangle {
                    Layout.fillWidth: true; Layout.preferredHeight: 64; radius: 14
                    color: root.currentStep === index ? AppTheme.primaryLight : AppTheme.surface
                    border.color: root.currentStep === index ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                    Behavior on color { ColorAnimation { duration: 200 } }
                    Behavior on border.color { ColorAnimation { duration: 200 } }
                    property bool hovered: false
                    scale: (root.currentStep === index ? 1.02 : 1) * (hovered ? 1.02 : 1)
                    Behavior on scale { NumberAnimation { duration: 150 } }
                    HoverHandler { onHoveredChanged: parent.hovered = hovered }
                    TapHandler { onTapped: root.currentStep = index }
                    RowLayout {
                        anchors.fill: parent; anchors.margins: 12; spacing: 10
                        Rectangle {
                            Layout.preferredWidth: 34; Layout.preferredHeight: 34; radius: 10
                            color: root.currentStep === index ? AppTheme.primary : AppTheme.hoverBg
                            Behavior on color { ColorAnimation { duration: 200 } }
                            Text {
                                anchors.centerIn: parent
                                text: ["edit_note", "bolt", "auto_fix_high"][index]
                                font.family: "Material Symbols Outlined"; font.pixelSize: 18
                                color: root.currentStep === index ? "white" : AppTheme.primary
                            }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true; spacing: 3
                            RowLayout { spacing: 6
                                Rectangle { width: 16; height: 16; radius: 8
                                    color: root.currentStep === index ? AppTheme.primary : AppTheme.hoverBg
                                    Text { anchors.centerIn: parent; text: index + 1; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: root.currentStep === index ? "white" : AppTheme.textSecondary }
                                }
                                Text { text: root.stepTitles[index]; font.family: "Inter"; font.pixelSize: 13; font.bold: true; color: AppTheme.textPrimary }
                            }
                        }
                    }
                }
            }
        }

        // Instruction banner
        Rectangle {
            Layout.fillWidth: true; Layout.preferredHeight: 54; radius: 12
            color: AppTheme.surface
            border.color: root.instruction !== "" ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
            Behavior on border.color { ColorAnimation { duration: 250 } }
            visible: root.instruction !== ""
            RowLayout {
                anchors.fill: parent; anchors.margins: 12; spacing: 10
                Rectangle {
                    width: 28; height: 28; radius: 14; color: AppTheme.primary
                    Text {
                        anchors.centerIn: parent
                        text: root.animRunning ? "touch_app" : "check_circle"
                        font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: "white"
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true; spacing: 2
                    Text {
                        Layout.fillWidth: true
                        text: root.instruction
                        font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: AppTheme.textPrimary
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        text: root.instructionDetail
                        font.family: "Inter"; font.pixelSize: 10; color: AppTheme.textSecondary
                        elide: Text.ElideRight; visible: text !== ""
                    }
                }
            }
        }

        // ── Demo area ─────────────────────────────────────────────────────────
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: 2
            radius: 16
            color: AppTheme.surface
            clip: true

            ColumnLayout {
                anchors.fill: parent; anchors.leftMargin: 16; anchors.rightMargin: 16; anchors.bottomMargin: 16; anchors.topMargin: 8; spacing: 12

                // ══ STEP 0 ═══════════════════════════════════════════════════════
                ColumnLayout {
                    visible: root.currentStep === 0
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 14
                    opacity: root.currentStep === 0 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: 250 } }
                    scale: root.currentStep === 0 ? 1 : 0.97
                    Behavior on scale { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }

                    RowLayout {
                        Layout.fillWidth: true; spacing: 10
                        Text { text: "Snippet Editor"; font.family: "Inter"; font.pixelSize: 16; font.bold: true; color: AppTheme.textPrimary; Layout.fillWidth: true }
                        Rectangle {
                            id: folderRect; objectName: "folderRect"
                            Layout.preferredWidth: 160; Layout.preferredHeight: 34; radius: 10
                            color: root.folderSelected ? AppTheme.primaryLight : AppTheme.surface
                            border.color: root.folderSelected ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                            Loader { active: root.spotlight === "folder"; sourceComponent: spotGlow }
                            RowLayout {
                                anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                                Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.folderSelected ? AppTheme.primary : AppTheme.textSecondary }
                                Text { Layout.fillWidth: true; text: root.folderSelected ? "General" : "— no folder —"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: root.folderSelected ? AppTheme.textPrimary : AppTheme.textSecondary; elide: Text.ElideRight }
                                Text { text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.textSecondary; visible: root.folderSelected }
                            }
                        }
                        Rectangle {
                            id: abbrevRect; objectName: "abbrevRect"
                            Layout.preferredWidth: 130; Layout.preferredHeight: 34; radius: 10; color: AppTheme.surface
                            border.color: root.abbrevText.length > 0 ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                            Loader { active: root.spotlight === "abbrev"; sourceComponent: spotGlow }
                            RowLayout {
                                anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
                                Image { source: "../app-icon.svg"; Layout.preferredWidth: 16; Layout.preferredHeight: 16; sourceSize.width: 16; sourceSize.height: 16; fillMode: Image.PreserveAspectFit; opacity: 0.7 }
                                Text { text: root.abbrevText; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.primary; verticalAlignment: Text.AlignVCenter; visible: root.abbrevText !== ""
                                    Loader { active: root.typingTarget === "abbrev"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                }
                                Text { Layout.fillWidth: true; text: "trigger word"; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.textSecondary; verticalAlignment: Text.AlignVCenter; visible: root.abbrevText === "" }
                            }
                        }
                            Rectangle { Layout.preferredWidth: 34; Layout.preferredHeight: 32; radius: 10; color: AppTheme.dangerLight
                                Text { anchors.centerIn: parent; text: "delete"; font.family: "Material Symbols Outlined"; font.pixelSize: 18; color: AppTheme.danger }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true; spacing: 8
                            Text { text: "INSERT"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
                        RowLayout {
                            spacing: 4
                            Text { text: "content_copy"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.textSecondary }
                            Text { text: "CLIP"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
                        }
                        RowLayout {
                            spacing: 4
                            Text { text: "calendar_today"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.textSecondary }
                            Text { text: "DATE"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
                        }
                        RowLayout {
                            spacing: 4
                            Text { text: "schedule"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.textSecondary }
                            Text { text: "TIME"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
                        }
                        RowLayout {
                            spacing: 4
                            Text { text: "input"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.primary }
                            Text { text: "FIELD"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.primary }
                        }
                        RowLayout {
                            spacing: 4
                            Text { text: "text_fields"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.primary }
                            Text { text: "CURSOR"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.primary }
                        }
                        Item { Layout.fillWidth: true }
                        Rectangle {
                            id: saveBtn; objectName: "saveBtn"
                            width: 130; height: 32; radius: 8
                            color: root.saved ? successGreen : AppTheme.primaryHover
                            Behavior on color { ColorAnimation { duration: 200 } }
                            Loader { active: root.spotlight === "save"; sourceComponent: spotGlow }
                            Text { anchors.centerIn: parent; text: root.saved ? "SAVED!" : "SAVE CHANGES"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: "white" }
                        }
                    }

                    // Editor
                    Rectangle {
                        id: contentEditor; objectName: "contentEditor"
                        Layout.fillWidth: true; Layout.fillHeight: true; radius: 16; color: AppTheme.surface; clip: true
                        Loader { active: root.spotlight === "content"; sourceComponent: spotGlow }
                        Text { id: contentTextItem; anchors.fill: parent; anchors.margins: 16; verticalAlignment: Text.AlignTop; text: root.contentText; font.family: "Inter"; font.pixelSize: 13; color: AppTheme.textPrimary; wrapMode: Text.WordWrap }
                        Loader { active: root.typingTarget === "content"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: 16 + Math.min(contentTextItem.contentWidth, parent.width - 32); anchors.top: parent.top; anchors.topMargin: 18 }
                        Text { anchors.fill: parent; anchors.margins: 16; verticalAlignment: Text.AlignTop; text: "Select a snippet from the library to edit it…"; font.family: "Inter"; font.pixelSize: 13; color: AppTheme.textSecondary; visible: root.contentText === "" }
                    }

                    // Saved confirmation
                    Rectangle {
                        Layout.fillWidth: true; Layout.preferredHeight: 96; radius: 12; color: AppTheme.surface; clip: true
                        border.color: root.saved ? successGreen : "transparent"; border.width: 1
                        Behavior on border.color { ColorAnimation { duration: 300 } }
                        ColumnLayout {
                            anchors.fill: parent; anchors.margins: 10; spacing: 6
                            RowLayout { spacing: 6
                                Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: successGreen; opacity: root.saved ? 1 : 0 }
                                Text { text: root.saved ? "Snippet saved! Type .hello anywhere and it expands instantly." : "Fill the fields above, then press Save — watch it work!"; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: root.saved ? successText : AppTheme.textSecondary; Behavior on color { ColorAnimation { duration: 300 } } }
                            }
                            Rectangle {
                                id: savedDemoBox; objectName: "savedDemoBox"
                                Layout.fillWidth: true; Layout.preferredHeight: 38; radius: 9; color: AppTheme.hoverBg
                                border.color: root.savedDemoExpanded ? successGreen : "transparent"; border.width: 1
                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "savedDemo" || root.spotlight === "savedExpand"; sourceComponent: spotGlow }
                                RowLayout {
                                    anchors.fill: parent; anchors.margins: 10; spacing: 6
                                    Text { text: "keyboard"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: AppTheme.textSecondary }
                                    Text { Layout.fillWidth: true; text: root.savedDemoExpanded ? "Hello, how are you?" : root.savedDemoText; font.family: "Inter"; font.pixelSize: 12; color: root.savedDemoExpanded ? successText : AppTheme.textPrimary; font.bold: root.savedDemoExpanded; Behavior on color { ColorAnimation { duration: 200 } }
                                        Loader { active: root.typingTarget === "savedDemo"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 2; anchors.verticalCenter: parent.verticalCenter }
                                    }
                                    Text { text: "send"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: root.savedDemoExpanded ? successGreen : AppTheme.primary }
                                }
                            }
                        }
                    }
                }

                // ══ STEP 1 ═══════════════════════════════════════════════════════
                ColumnLayout {
                    visible: root.currentStep === 1
                    Layout.fillWidth: true; Layout.fillHeight: true; spacing: 10
                    opacity: root.currentStep === 1 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: 250 } }
                    scale: root.currentStep === 1 ? 1 : 0.97
                    Behavior on scale { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }

                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        Text { text: "Pick an app:"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary }
                        Item { Layout.fillWidth: true }
                        Repeater {
                            model: root.examples
                            delegate: Rectangle {
                                Layout.preferredWidth: 108; Layout.preferredHeight: 30; radius: 15
                                color: root.currentExample === index ? AppTheme.primary : AppTheme.hoverBg
                                TapHandler { onTapped: root.currentExample = index }
                                RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 5
                                    Text { text: modelData.icon; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.currentExample === index ? "white" : AppTheme.textSecondary }
                                    Text { Layout.fillWidth: true; text: modelData.name; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: root.currentExample === index ? "white" : AppTheme.textPrimary }
                                }
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true; Layout.fillHeight: true; radius: 14; color: AppTheme.background; border.color: AppTheme.hoverBg; border.width: 1
                        ColumnLayout {
                            anchors.fill: parent; spacing: 0
                            Rectangle {
                                Layout.fillWidth: true; Layout.preferredHeight: 40; color: AppTheme.surface
                                RowLayout {
                                    anchors.fill: parent; anchors.leftMargin: 14; anchors.rightMargin: 14
                                    Rectangle { width: 26; height: 26; radius: 13; color: root.currentExample === 0 ? "#ea4335" : (root.currentExample === 1 ? "#25d366" : "#0084ff")
                                        Text { anchors.centerIn: parent; text: root.examples[root.currentExample].icon; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: "white" }
                                    }
                                    Text { Layout.fillWidth: true; text: root.examples[root.currentExample].name; font.family: "Inter"; font.pixelSize: 13; font.bold: true; color: AppTheme.textPrimary }
                                    Text { text: "more_vert"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.textSecondary }
                                }
                            }
                            Item {
                                Layout.fillWidth: true; Layout.fillHeight: true
                                ColumnLayout { visible: root.currentExample === 0; anchors.fill: parent; anchors.margins: 16; spacing: 10
                                    RowLayout { Layout.fillWidth: true; spacing: 8
                                        Text { text: "To:"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary }
                                        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 30; radius: 8; color: AppTheme.hoverBg; border.color: (root.exTyped !== "" || root.exExpanded) ? AppTheme.primary : "transparent"; border.width: 1
                                            Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; text: root.exExpanded ? "test@gmail.com" : root.exTyped; font.family: "Inter"; font.pixelSize: 12; color: root.exExpanded ? successText : AppTheme.textPrimary; font.bold: root.exExpanded; Behavior on color { ColorAnimation { duration: 200 } } }
                                        }
                                        Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: successGreen; visible: root.exExpanded }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Text { text: "Subject:"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary }
                                        Text { text: "Meeting notes"; font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary }
                                    }
                                    Rectangle { Layout.fillWidth: true; Layout.fillHeight: true; radius: 8; color: AppTheme.hoverBg
                                        Text { anchors.fill: parent; anchors.margins: 10; text: "Hi Alex,\n\nHere are the notes from today's meeting.\n\nBest,\nShobdo"; font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary }
                                    }
                                }
                                ColumnLayout { visible: root.currentExample === 1; anchors.fill: parent; anchors.margins: 14; spacing: 8
                                    Item { Layout.fillHeight: true }
                                    Rectangle {
                                        Layout.alignment: Qt.AlignLeft
                                        Layout.preferredWidth: 170; Layout.preferredHeight: 34; radius: 12
                                        color: AppTheme.surface; clip: true
                                        Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap; text: "Hey! Are you coming?"; font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary }
                                    }
                                    Rectangle {
                                        id: exMsg1; objectName: "exMsg1"
                                        Layout.alignment: Qt.AlignRight
                                        Layout.preferredWidth: 118
                                        Layout.preferredHeight: root.exExpanded1 ? 34 : 0
                                        radius: 12
                                        color: AppTheme.primary; clip: true
                                        opacity: root.exExpanded1 ? 1 : 0
                                        scale: root.exExpanded1 ? 1 : 0.92
                                        Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                                        Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack } }
                                        Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap; text: "On my way!"; font.family: "Inter"; font.pixelSize: 12; color: "white" }
                                    }
                                    Rectangle {
                                        id: exMsg2; objectName: "exMsg2"
                                        Layout.alignment: Qt.AlignRight
                                        Layout.preferredWidth: 210
                                        Layout.preferredHeight: root.exExpanded2 ? 36 : 0
                                        radius: 12
                                        color: AppTheme.primary; clip: true
                                        opacity: root.exExpanded2 ? 1 : 0
                                        scale: root.exExpanded2 ? 1 : 0.92
                                        Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                                        Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack } }
                                        Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap; text: "Hello! How are you? \ud83d\ude0a"; font.family: "Inter"; font.pixelSize: 12; color: "white" }
                                    }
                                }
                                ColumnLayout { visible: root.currentExample === 2; anchors.fill: parent; anchors.margins: 14; spacing: 8
                                    Item { Layout.fillHeight: true }
                                    Rectangle { Layout.alignment: Qt.AlignLeft; Layout.preferredWidth: 100; Layout.preferredHeight: 34; radius: 12; color: AppTheme.surface; clip: true
                                        Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap; text: "you there?"; font.family: "Inter"; font.pixelSize: 12; color: AppTheme.textPrimary }
                                    }
                                    Rectangle {
                                        id: exMsg3; objectName: "exMsg3"
                                        Layout.alignment: Qt.AlignRight
                                        Layout.preferredWidth: 165
                                        Layout.preferredHeight: root.exExpanded ? 36 : 0
                                        radius: 12
                                        color: AppTheme.primary; clip: true
                                        opacity: root.exExpanded ? 1 : 0
                                        scale: root.exExpanded ? 1 : 0.92
                                        Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                                        Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack } }
                                        Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap; text: "Talk to you later!"; font.family: "Inter"; font.pixelSize: 12; color: "white" }
                                    }
                                }
                            }
                            Rectangle {
                                Layout.fillWidth: true; Layout.preferredHeight: 46; color: AppTheme.surface
                                RowLayout {
                                    anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                                    Rectangle {
                                        id: exInputRect; objectName: "exInputRect"
                                        Layout.fillWidth: true; Layout.preferredHeight: 32; radius: 16; color: AppTheme.hoverBg
                                        Loader { active: root.spotlight === "ex"; sourceComponent: spotGlow }
                                        Text { anchors.fill: parent; anchors.margins: 12; verticalAlignment: Text.AlignVCenter; text: root.exTyped; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.textPrimary; visible: root.exTyped !== "" && !root.exExpanded
                                            Loader { active: root.typingTarget === "ex"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                        Text { anchors.fill: parent; anchors.margins: 12; verticalAlignment: Text.AlignVCenter; text: root.currentExample === 0 ? (root.exExpanded ? "Email filled ✓" : "Type .email…") : (root.currentExample === 1 ? (root.exExpanded2 ? "Messages sent ✓" : (root.exExpanded1 ? "Type .hello…" : "Type .omw…")) : (root.exExpanded ? "Message sent ✓" : "Type .ttyl…")); font.family: "Inter"; font.pixelSize: 11; color: (root.exExpanded || root.exExpanded2) ? successText : AppTheme.textSecondary; font.bold: true; visible: root.exTyped === "" || root.exExpanded }
                                    }
                                    Rectangle {
                                        id: exSendBtn; objectName: "exSendBtn"
                                        width: 32; height: 32; radius: 16; color: (root.exExpanded || root.exExpanded2) ? successGreen : AppTheme.primary
                                        Behavior on color { ColorAnimation { duration: 200 } }
                                        Loader { active: root.spotlight === "exExpand"; sourceComponent: spotGlow }
                                        Text { anchors.centerIn: parent; text: "send"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: "white" }
                                    }
                                }
                            }
                        }
                    }
                }

                // ══ STEP 2 ═══════════════════════════════════════════════════════
                ColumnLayout {
                    visible: root.currentStep === 2
                    Layout.fillWidth: true; Layout.fillHeight: true; spacing: 12
                    opacity: root.currentStep === 2 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: 250 } }
                    scale: root.currentStep === 2 ? 1 : 0.97
                    Behavior on scale { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }

                    // Sub-step chips
                    RowLayout {
                        Layout.fillWidth: true; spacing: 8
                        Repeater {
                            model: root.dynStepTitles.length
                            delegate: Rectangle {
                                Layout.fillWidth: true; Layout.preferredHeight: 34; radius: 10
                                color: root.dynSubStep === index ? AppTheme.primary : AppTheme.surface
                                border.color: root.dynSubStep === index ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                                Behavior on color { ColorAnimation { duration: 200 } }
                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                property bool hovered: false
                                scale: hovered && root.dynSubStep !== index ? 1.03 : 1
                                Behavior on scale { NumberAnimation { duration: 150 } }
                                HoverHandler { onHoveredChanged: parent.hovered = hovered }
                                TapHandler { onTapped: root.dynSubStep = index }
                                RowLayout {
                                    anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                                    Text { text: ["calendar_month", "input", "text_fields"][index]; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.dynSubStep === index ? "white" : AppTheme.primary }
                                    Text { Layout.fillWidth: true; text: root.dynStepTitles[index]; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: root.dynSubStep === index ? "white" : AppTheme.textPrimary; elide: Text.ElideRight }
                                }
                            }
                        }
                    }

                    // Sub-step 0: Date & Time
                    ColumnLayout {
                        visible: root.dynSubStep === 0
                        Layout.fillWidth: true; Layout.fillHeight: true; spacing: 12
                        opacity: root.dynSubStep === 0 ? 1 : 0; Behavior on opacity { NumberAnimation { duration: 220 } }
                        scale: root.dynSubStep === 0 ? 1 : 0.96; Behavior on scale { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }

                        RowLayout {
                            Layout.fillWidth: true; spacing: 10
                            Text { text: "Dynamic Snippet"; font.family: "Inter"; font.pixelSize: 16; font.bold: true; color: AppTheme.textPrimary; Layout.fillWidth: true }
                            Rectangle { Layout.preferredWidth: 160; Layout.preferredHeight: 34; radius: 10; color: AppTheme.primaryLight; border.color: AppTheme.primary; border.width: 1
                                RowLayout { anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                                    Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.primary }
                                    Text { Layout.fillWidth: true; text: "General"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary; elide: Text.ElideRight }
                                    Text { text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.textSecondary }
                                }
                            }
                            Rectangle {
                                id: dynTriggerRect0; objectName: "dynTriggerRect0"
                                Layout.preferredWidth: 130; Layout.preferredHeight: 34; radius: 10; color: AppTheme.surface
                                border.color: root.dynFieldTriggerText !== "" ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "dynTrigger"; sourceComponent: spotGlow }
                                RowLayout {
                                    anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
                                    Image { source: "../app-icon.svg"; Layout.preferredWidth: 16; Layout.preferredHeight: 16; sourceSize.width: 16; sourceSize.height: 16; fillMode: Image.PreserveAspectFit; opacity: 0.7 }
                                    Text { text: root.dynFieldTriggerText; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.primary; verticalAlignment: Text.AlignVCenter; visible: root.dynFieldTriggerText !== ""
                                        Loader { active: root.typingTarget === "dynTrigger"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                    }
                                    Text { Layout.fillWidth: true; text: "trigger word"; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.textSecondary; verticalAlignment: Text.AlignVCenter; visible: root.dynFieldTriggerText === "" }
                                }
                            }
                            Rectangle { Layout.preferredWidth: 34; Layout.preferredHeight: 34; radius: 10; color: AppTheme.dangerLight
                                Text { anchors.centerIn: parent; text: "delete"; font.family: "Material Symbols Outlined"; font.pixelSize: 18; color: AppTheme.danger }
                            }
                        }

                        RowLayout {
                            Layout.fillWidth: true; spacing: 8
                            Text { text: "INSERT"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.textSecondary }
                            Rectangle {
                                id: dynDateBtn; objectName: "dynDateBtn"
                                Layout.preferredWidth: 58; Layout.preferredHeight: 26; radius: 8
                                color: root.dynDateResolved ? AppTheme.primaryLight : AppTheme.hoverBg
                                border.color: root.dynDateResolved ? successGreen : "transparent"; border.width: 1
                                Behavior on color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "date"; sourceComponent: spotGlow }
                                RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                    Text { text: "calendar_today"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.dynDateResolved ? successGreen : AppTheme.textSecondary }
                                    Text { text: "DATE"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: root.dynDateResolved ? successGreen : AppTheme.textSecondary }
                                }
                            }
                            Rectangle {
                                id: dynTimeBtn; objectName: "dynTimeBtn"
                                Layout.preferredWidth: 58; Layout.preferredHeight: 26; radius: 8
                                color: root.dynTimeResolved ? AppTheme.primaryLight : AppTheme.hoverBg
                                border.color: root.dynTimeResolved ? successGreen : "transparent"; border.width: 1
                                Behavior on color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "time"; sourceComponent: spotGlow }
                                RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                    Text { text: "schedule"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: root.dynTimeResolved ? successGreen : AppTheme.textSecondary }
                                    Text { text: "TIME"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: root.dynTimeResolved ? successGreen : AppTheme.textSecondary }
                                }
                            }
                            RowLayout {
                                spacing: 4
                                Text { text: "input"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.primary }
                                Text { text: "FIELD"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.primary }
                            }
                            RowLayout {
                                spacing: 4
                                Text { text: "text_fields"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.primary }
                                Text { text: "CURSOR"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: AppTheme.primary }
                            }
                            Item { Layout.fillWidth: true }
                            Rectangle {
                                id: watchBtn; objectName: "watchBtn"
                                width: 130; height: 32; radius: 8
                                color: (root.dynDateResolved && root.dynTimeResolved) ? successGreen : AppTheme.primaryHover
                                Behavior on color { ColorAnimation { duration: 200 } }
                                Text { anchors.centerIn: parent; text: (root.dynDateResolved && root.dynTimeResolved) ? "DONE!" : "TRY DATE & TIME"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: "white" }
                            }
                        }

                        // Editor
                        Rectangle {
                            id: dynEditorRect; objectName: "dynEditorRect"
                            Layout.fillWidth: true; Layout.fillHeight: true; radius: 16; color: AppTheme.surface; clip: true
                            Loader { active: root.spotlight === "date" || root.spotlight === "time" || root.spotlight === "dynEditor"; sourceComponent: spotGlow }
                            ColumnLayout { anchors.fill: parent; anchors.margins: 16; spacing: 12
                                RowLayout {
                                    Layout.fillWidth: true; spacing: 6
                                    Text { Layout.fillWidth: true; text: root.dynEditorText === "" ? "Type your message here…" : root.dynEditorText; font.family: "Inter"; font.pixelSize: 14; color: root.dynEditorText === "" ? AppTheme.textSecondary : AppTheme.textPrimary; wrapMode: Text.WordWrap
                                        Loader { active: root.typingTarget === "dynEditor"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 2; anchors.verticalCenter: parent.verticalCenter }
                                    }
                                }
                                Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.hoverBg }
                                Text { Layout.fillWidth: true; text: root.dynDateResolved && root.dynTimeResolved ? "Both words turned green — they now fill with live values automatically." : "Type normally, then click DATE and TIME to add auto-fill words."; font.family: "Inter"; font.pixelSize: 10; color: AppTheme.textPrimary; wrapMode: Text.WordWrap }
                            }
                        }

                        Rectangle {
                            id: dynResultBox; objectName: "dynResultBox"
                            Layout.fillWidth: true; Layout.preferredHeight: 80; radius: 12; color: AppTheme.surface; clip: true
                            border.color: (root.dynDateResolved && root.dynTimeResolved) ? successGreen : "transparent"; border.width: 1
                            Behavior on border.color { ColorAnimation { duration: 300 } }
                            Loader { active: root.spotlight === "dynExpand"; sourceComponent: spotGlow }
                            ColumnLayout {
                                anchors.fill: parent; anchors.margins: 10; spacing: 6
                                RowLayout { spacing: 6
                                    Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: successGreen; opacity: (root.dynDateResolved && root.dynTimeResolved) ? 1 : 0 }
                                    Text { Layout.fillWidth: true; text: (root.dynDateResolved && root.dynTimeResolved) ? "Done! Every time you type .mtg, the date & time are always up to date." : "Type .mtg to see the auto-fill words expand."; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: (root.dynDateResolved && root.dynTimeResolved) ? successText : AppTheme.textPrimary; Behavior on color { ColorAnimation { duration: 300 } } }
                                }
                                Rectangle {
                                    Layout.fillWidth: true; Layout.preferredHeight: 38; radius: 9; color: AppTheme.hoverBg
                                    border.color: (root.dynDateResolved && root.dynTimeResolved) ? successGreen : "transparent"; border.width: 1
                                    Behavior on border.color { ColorAnimation { duration: 300 } }
                                    RowLayout {
                                        anchors.fill: parent; anchors.margins: 10; spacing: 6
                                        Text { text: "keyboard"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: AppTheme.textSecondary }
                                        Text { Layout.fillWidth: true; text: root.dynResultExpanded ? root.dynResult : root.dynResultTyped; font.family: "Inter"; font.pixelSize: 12; color: (root.dynDateResolved && root.dynTimeResolved) ? successText : AppTheme.textPrimary; font.bold: root.dynDateResolved && root.dynTimeResolved || root.dynResultExpanded; Behavior on color { ColorAnimation { duration: 300 } }
                                            Loader { active: root.typingTarget === "dynExpand"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                        Text { text: "send"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: (root.dynDateResolved && root.dynTimeResolved) ? successGreen : AppTheme.primary }
                                    }
                                }
                            }
                        }
                    }

                    // Sub-step 1: Fill-in Fields
                    Item {
                        visible: root.dynSubStep === 1
                        Layout.fillWidth: true; Layout.fillHeight: true
                        opacity: root.dynSubStep === 1 ? 1 : 0; Behavior on opacity { NumberAnimation { duration: 220 } }
                        scale: root.dynSubStep === 1 ? 1 : 0.96; Behavior on scale { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                        clip: true

                        Flickable {
                            anchors.fill: parent
                            clip: true
                            contentWidth: width
                            contentHeight: fillCol.implicitHeight
                            boundsBehavior: Flickable.StopAtBounds

                        ColumnLayout {
                            id: fillCol
                            width: parent.width
                            spacing: 6

                            // Row 1: Title + folder + trigger + delete
                            RowLayout {
                                Layout.fillWidth: true; spacing: 10
                                Text { text: "Fill-in Snippet"; font.family: "Inter"; font.pixelSize: 15; font.bold: true; color: AppTheme.textPrimary; Layout.fillWidth: true }
                                Rectangle { Layout.preferredWidth: 128; Layout.preferredHeight: 28; radius: 10; color: AppTheme.primaryLight; border.color: AppTheme.primary; border.width: 1
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                                        Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.primary }
                                        Text { Layout.fillWidth: true; text: "General"; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: AppTheme.textPrimary; elide: Text.ElideRight }
                                        Text { text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle {
                                    id: dynTriggerRect; objectName: "dynTriggerRect"
                                    Layout.preferredWidth: 118; Layout.preferredHeight: 28; radius: 10; color: AppTheme.surface
                                    border.color: root.dynFieldTriggerText !== "" ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                                    Behavior on border.color { ColorAnimation { duration: 200 } }
                                    Loader { active: root.spotlight === "dynTrigger"; sourceComponent: spotGlow }
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
                                        Image { source: "../app-icon.svg"; Layout.preferredWidth: 14; Layout.preferredHeight: 14; sourceSize.width: 14; sourceSize.height: 14; fillMode: Image.PreserveAspectFit; opacity: 0.7 }
                                        Text { text: root.dynFieldTriggerText; font.family: "Courier New"; font.pixelSize: 11; font.bold: true; color: AppTheme.primary; verticalAlignment: Text.AlignVCenter; visible: root.dynFieldTriggerText !== ""
                                            Loader { active: root.typingTarget === "dynTrigger"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                        Text { Layout.fillWidth: true; text: "trigger word"; font.family: "Courier New"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary; verticalAlignment: Text.AlignVCenter; visible: root.dynFieldTriggerText === "" }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 28; Layout.preferredHeight: 28; radius: 10; color: AppTheme.dangerLight
                                    Text { anchors.centerIn: parent; text: "delete"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.danger }
                                }
                            }

                            // Row 2: INSERT toolbar
                            RowLayout {
                                Layout.fillWidth: true; spacing: 8
                                Text { text: "INSERT"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                Rectangle { Layout.preferredWidth: 50; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                        Text { text: "calendar_today"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "DATE"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 50; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                        Text { text: "schedule"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "TIME"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle {
                                    id: dynFieldBtn; objectName: "dynFieldBtn"
                                    Layout.preferredWidth: 68; Layout.preferredHeight: 22; radius: 8
                                    color: root.dynFieldInserted ? AppTheme.primaryLight : AppTheme.hoverBg
                                    border.color: root.dynFieldInserted ? AppTheme.primary : "transparent"; border.width: 1
                                    Behavior on color { ColorAnimation { duration: 200 } }
                                    Behavior on border.color { ColorAnimation { duration: 200 } }
                                    Loader { active: root.spotlight === "dynFieldBtn"; sourceComponent: spotGlow }
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8; spacing: 4
                                        Text { text: "input"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: root.dynFieldInserted ? AppTheme.primary : AppTheme.textSecondary }
                                        Text { text: "FIELD"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: root.dynFieldInserted ? AppTheme.primary : AppTheme.textSecondary }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 74; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8; spacing: 4
                                        Text { text: "text_fields"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "CURSOR"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Item { Layout.fillWidth: true }
                                Rectangle {
                                    Layout.preferredWidth: 132; Layout.preferredHeight: 26; radius: 8
                                    color: root.dynFieldResolved ? successGreen : AppTheme.primaryHover
                                    Behavior on color { NumberAnimation { duration: 200 } }
                                    Text { anchors.centerIn: parent; text: root.dynFieldResolved ? "DONE!" : "TRY FILL-IN"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: "white" }
                                }
                            }

                            // Row 3: Editor strip (compact)
                            Rectangle {
                                id: dynFieldEditorRect; objectName: "dynFieldEditorRect"
                                Layout.fillWidth: true; Layout.preferredHeight: 52; radius: 12; color: AppTheme.surface; clip: true
                                border.color: root.dynFieldResolved ? successGreen : (root.dynFieldEditorText !== "" ? AppTheme.primary : AppTheme.hoverBg); border.width: 1
                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "dynFieldEditor"; sourceComponent: spotGlow }
                                RowLayout {
                                    anchors.fill: parent; anchors.margins: 10; spacing: 8
                                    Text { text: "description"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.primary }
                                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                                        Text { text: "Your snippet"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; font.letterSpacing: 1; color: AppTheme.textSecondary }
                                        Text { Layout.fillWidth: true; text: root.dynFieldEditorText === "" ? "Type the expansion message here…" : root.dynFieldEditorText; font.family: "Inter"; font.pixelSize: 12; color: root.dynFieldResolved ? successText : (root.dynFieldEditorText === "" ? AppTheme.textSecondary : AppTheme.textPrimary); font.bold: root.dynFieldResolved; wrapMode: Text.WordWrap; elide: Text.ElideRight
                                            Loader { active: root.typingTarget === "dynFieldEditor"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 2; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                    }
                                    Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: successGreen; visible: root.dynFieldResolved }
                                }
                            }

                            // Row 4: Any-other-app window (fills remaining height)
                            Rectangle {
                                Layout.fillWidth: true; Layout.preferredHeight: 140; radius: 14; color: AppTheme.background; border.color: AppTheme.hoverBg; border.width: 1; clip: true
                                ColumnLayout { anchors.fill: parent; spacing: 0
                                    Rectangle {
                                        Layout.fillWidth: true; Layout.preferredHeight: 28; color: AppTheme.surface
                                        RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12
                                            Rectangle { width: 24; height: 24; radius: 12; color: AppTheme.primary
                                                Text { anchors.centerIn: parent; text: "smartphone"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: "white" }
                                            }
                                            Text { Layout.fillWidth: true; text: "Any other app"; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: AppTheme.textPrimary }
                                            Text { text: "more_vert"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: AppTheme.textSecondary }
                                        }
                                    }
                                    Item {
                                        Layout.fillWidth: true; Layout.fillHeight: true
                                        Text { anchors.centerIn: parent; text: "Type .price here and your snippet fills in"; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary; visible: !root.dynFieldAppExpanded }
                                        Rectangle {
                                            id: dynFieldBubble
                                            anchors.right: parent.right; anchors.bottom: parent.bottom
                                            anchors.rightMargin: 12; anchors.bottomMargin: 10
                                            width: Math.min(parent.width - 24, 320)
                                            height: root.dynFieldAppExpanded ? 50 : 0
                                            radius: 12; color: AppTheme.primary; clip: true
                                            opacity: root.dynFieldAppExpanded ? 1 : 0
                                            scale: root.dynFieldAppExpanded ? 1 : 0.92
                                            Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                                            Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack } }
                                            Behavior on height { NumberAnimation { duration: 240 } }
                                            ColumnLayout {
                                                anchors.fill: parent; anchors.margins: 10; spacing: 3
                                                RowLayout { spacing: 5
                                                    Text { text: "auto_awesome"; font.family: "Material Symbols Outlined"; font.pixelSize: 12; color: "white" }
                                                    Text { text: "Expanded — price filled in"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: "white" }
                                                }
                                                Text {
                                                    Layout.fillWidth: true; text: root.dynFieldAppExpandedText; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white"; wrapMode: Text.WordWrap; verticalAlignment: Text.AlignVCenter
                                                    Loader { active: root.typingTarget === "dynFieldAppExpand"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                                }
                                            }
                                        }
                                    }
                                    Rectangle {
                                        id: dynFieldAppRect; objectName: "dynFieldAppRect"
                                        Layout.fillWidth: true; Layout.preferredHeight: 34; color: AppTheme.surface
                                        RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                                            Rectangle {
                                                Layout.fillWidth: true; Layout.preferredHeight: 28; radius: 14; color: AppTheme.hoverBg
                                                border.color: root.dynFieldAppExpanded ? successGreen : (root.dynFieldAppText !== "" ? AppTheme.primary : "transparent"); border.width: 1
                                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                                Loader { active: root.spotlight === "dynFieldApp"; sourceComponent: spotGlow }
                                                Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; text: root.dynFieldAppText; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.primary; visible: root.dynFieldAppText !== "" && !root.dynFieldAppExpanded
                                                    Loader { active: root.typingTarget === "dynFieldApp"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                                }
                                                Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; text: root.dynFieldAppExpanded ? "Saved with $49 filled in" : "Type .price…"; font.family: "Inter"; font.pixelSize: 11; color: root.dynFieldAppExpanded ? successText : AppTheme.textSecondary; font.bold: true; visible: root.dynFieldAppText === "" || root.dynFieldAppExpanded }
                                            }
                                            Rectangle {
                                                width: 28; height: 28; radius: 14; color: root.dynFieldAppExpanded ? successGreen : AppTheme.primary
                                                Behavior on color { ColorAnimation { duration: 200 } }
                                                Text { anchors.centerIn: parent; text: "send"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: "white" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        }

                        // Fill-in form popup (floating overlay)
                        Rectangle {
                            id: dynFieldPopup; objectName: "dynFieldPopup"
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.top: parent.top; anchors.topMargin: 108
                            width: parent.width * 0.6
                            height: root.dynFieldPopupOpen ? 128 : 0
                            z: 5
                            radius: 14; color: AppTheme.surface; clip: true
                            border.color: AppTheme.primary; border.width: 1
                            opacity: root.dynFieldPopupOpen ? 1 : 0
                            Behavior on height { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
                            Behavior on opacity { NumberAnimation { duration: 250 } }
                            Loader { active: root.spotlight === "dynFieldPopup"; sourceComponent: spotGlow }
                            ColumnLayout {
                                anchors.fill: parent; anchors.margins: 10; spacing: 6
                                RowLayout { spacing: 6
                                    Rectangle { width: 8; height: 8; radius: 4; color: AppTheme.primary }
                                    Text { text: "Fill in the field"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: AppTheme.textPrimary }
                                    Item { Layout.fillWidth: true }
                                    Text { text: "close"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.textSecondary }
                                }
                                Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.hoverBg }
                                RowLayout { spacing: 6
                                    Text { text: "Label:"; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: AppTheme.textSecondary }
                                    Rectangle {
                                        id: dynFieldInput; objectName: "dynFieldInput"
                                        Layout.fillWidth: true; Layout.preferredHeight: 30; radius: 8
                                        color: root.dynFieldResolved ? AppTheme.primaryLight : AppTheme.hoverBg
                                        border.color: root.dynFieldResolved ? successGreen : AppTheme.primary; border.width: 1
                                        Behavior on border.color { ColorAnimation { duration: 200 } }
                                        Loader { active: root.spotlight === "field"; sourceComponent: spotGlow }
                                        RowLayout {
                                            anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8; spacing: 4
                                            Text { text: root.dynFieldValue; font.family: "Inter"; font.pixelSize: 13; color: AppTheme.textPrimary; font.bold: true; verticalAlignment: Text.AlignVCenter
                                                Loader { active: root.typingTarget === "dynField"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                            }
                                            Item { Layout.fillWidth: true }
                                            Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: successGreen; visible: root.dynFieldResolved }
                                        }
                                    }
                                }
                                Rectangle { Layout.fillWidth: true; height: 1; color: AppTheme.hoverBg }
                                Rectangle {
                                    Layout.fillWidth: true; Layout.preferredHeight: 28; radius: 8; color: AppTheme.primary
                                    Text { anchors.centerIn: parent; text: "Submit"; font.family: "Inter"; font.pixelSize: 11; font.bold: true; color: "white" }
                                }
                            }
                        }
                    }

                    // Sub-step 2: Cursor
                    Item {
                        visible: root.dynSubStep === 2
                        Layout.fillWidth: true; Layout.fillHeight: true
                        opacity: root.dynSubStep === 2 ? 1 : 0; Behavior on opacity { NumberAnimation { duration: 220 } }
                        scale: root.dynSubStep === 2 ? 1 : 0.96; Behavior on scale { NumberAnimation { duration: 220; easing.type: Easing.OutCubic } }
                        clip: true

                        Flickable {
                            anchors.fill: parent
                            clip: true
                            contentWidth: width
                            contentHeight: curCol.implicitHeight
                            boundsBehavior: Flickable.StopAtBounds

                        ColumnLayout {
                            id: curCol
                            width: parent.width
                            spacing: 6

                            // Row 1: Title + folder + trigger + delete
                            RowLayout {
                                Layout.fillWidth: true; spacing: 10
                                Text { text: "Cursor Snippet"; font.family: "Inter"; font.pixelSize: 15; font.bold: true; color: AppTheme.textPrimary; Layout.fillWidth: true }
                                Rectangle { Layout.preferredWidth: 128; Layout.preferredHeight: 28; radius: 10; color: AppTheme.primaryLight; border.color: AppTheme.primary; border.width: 1
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10
                                        Text { text: "folder"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.primary }
                                        Text { Layout.fillWidth: true; text: "General"; font.family: "Inter"; font.pixelSize: 10; font.bold: true; color: AppTheme.textPrimary; elide: Text.ElideRight }
                                        Text { text: "expand_more"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle {
                                    id: dynCursorTriggerRect; objectName: "dynCursorTriggerRect"
                                    Layout.preferredWidth: 118; Layout.preferredHeight: 28; radius: 10; color: AppTheme.surface
                                    border.color: root.dynCursorTriggerText !== "" ? AppTheme.primary : AppTheme.hoverBg; border.width: 1
                                    Behavior on border.color { ColorAnimation { duration: 200 } }
                                    Loader { active: root.spotlight === "dynCursorTrigger"; sourceComponent: spotGlow }
                                    RowLayout {
                                        anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; spacing: 6
                                        Image { source: "../app-icon.svg"; Layout.preferredWidth: 14; Layout.preferredHeight: 14; sourceSize.width: 14; sourceSize.height: 14; fillMode: Image.PreserveAspectFit; opacity: 0.7 }
                                        Text { text: root.dynCursorTriggerText; font.family: "Courier New"; font.pixelSize: 11; font.bold: true; color: AppTheme.primary; verticalAlignment: Text.AlignVCenter; visible: root.dynCursorTriggerText !== ""
                                            Loader { active: root.typingTarget === "dynCursorTrigger"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                        }
                                        Text { Layout.fillWidth: true; text: "trigger word"; font.family: "Courier New"; font.pixelSize: 11; font.bold: true; color: AppTheme.textSecondary; verticalAlignment: Text.AlignVCenter; visible: root.dynCursorTriggerText === "" }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 28; Layout.preferredHeight: 28; radius: 10; color: AppTheme.dangerLight
                                    Text { anchors.centerIn: parent; text: "delete"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: AppTheme.danger }
                                }
                            }

                            // Row 2: INSERT toolbar
                            RowLayout {
                                Layout.fillWidth: true; spacing: 8
                                Text { text: "INSERT"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                Rectangle { Layout.preferredWidth: 50; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                        Text { text: "calendar_today"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "DATE"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 50; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 6; anchors.rightMargin: 6; spacing: 4
                                        Text { text: "schedule"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "TIME"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle { Layout.preferredWidth: 68; Layout.preferredHeight: 22; radius: 8; color: AppTheme.hoverBg
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8; spacing: 4
                                        Text { text: "input"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: AppTheme.textSecondary }
                                        Text { text: "FIELD"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: AppTheme.textSecondary }
                                    }
                                }
                                Rectangle {
                                    id: dynCursorBtn; objectName: "dynCursorBtn"
                                    Layout.preferredWidth: 78; Layout.preferredHeight: 22; radius: 8
                                    color: root.dynCursorInserted ? AppTheme.primaryLight : AppTheme.hoverBg
                                    border.color: root.dynCursorInserted ? AppTheme.primary : "transparent"; border.width: 1
                                    Behavior on color { ColorAnimation { duration: 200 } }
                                    Behavior on border.color { ColorAnimation { duration: 200 } }
                                    Loader { active: root.spotlight === "dynCursorBtn"; sourceComponent: spotGlow }
                                    RowLayout { anchors.fill: parent; anchors.leftMargin: 8; anchors.rightMargin: 8; spacing: 4
                                        Text { text: "text_fields"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: root.dynCursorInserted ? AppTheme.primary : AppTheme.textSecondary }
                                        Text { text: "CURSOR"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: root.dynCursorInserted ? AppTheme.primary : AppTheme.textSecondary }
                                    }
                                }
                                Item { Layout.fillWidth: true }
                                Rectangle {
                                    Layout.preferredWidth: 132; Layout.preferredHeight: 26; radius: 8
                                    color: root.dynCursorPlaced ? successGreen : AppTheme.primaryHover
                                    Behavior on color { ColorAnimation { duration: 200 } }
                                    Text { anchors.centerIn: parent; text: root.dynCursorPlaced ? "DONE!" : "TRY CURSOR"; font.family: "Inter"; font.pixelSize: 9; font.bold: true; color: "white" }
                                }
                            }

                            // Row 3: Editor strip (compact) with cursor marker
                            Rectangle {
                                id: dynCursorEditorRect; objectName: "dynCursorEditorRect"
                                Layout.fillWidth: true; Layout.preferredHeight: 52; radius: 12; color: AppTheme.surface; clip: true
                                border.color: root.dynCursorPlaced ? successGreen : (root.dynCursorEditorText !== "" ? AppTheme.primary : AppTheme.hoverBg); border.width: 1
                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                Loader { active: root.spotlight === "dynCursorEditor" || root.spotlight === "dynCursor"; sourceComponent: spotGlow }
                                RowLayout {
                                    anchors.fill: parent; anchors.margins: 10; spacing: 8
                                    Text { text: "text_fields"; font.family: "Material Symbols Outlined"; font.pixelSize: 14; color: AppTheme.primary }
                                    ColumnLayout { Layout.fillWidth: true; spacing: 2
                                        Text { text: "Your snippet"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; font.letterSpacing: 1; color: AppTheme.textSecondary }
                                        RowLayout { Layout.fillWidth: true; spacing: 2
                                            Text {
                                                Layout.fillWidth: true
                                                textFormat: Text.RichText
                                                text: {
                                                    var t = root.dynCursorEditorText
                                                    if (t === "") return "Type your line here…"
                                                    if (t.indexOf("{cursor}") >= 0) {
                                                        var hl = "<span style='background-color:#ffe08a;color:#1a1a1a;font-weight:bold'>"
                                                        t = t.replace("{cursor}", hl + "{cursor}" + "</span>")
                                                    }
                                                    return t
                                                }
                                                font.family: "Inter"; font.pixelSize: 12; color: root.dynCursorEditorText === "" ? AppTheme.textSecondary : AppTheme.textPrimary; wrapMode: Text.WordWrap; elide: Text.ElideRight; verticalAlignment: Text.AlignVCenter
                                                Loader { active: root.typingTarget === "dynCursorEditor"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 2; anchors.verticalCenter: parent.verticalCenter }
                                            }
                                            Rectangle {
                                                width: 2; height: 15; radius: 1; color: AppTheme.primary
                                                visible: root.dynCursorPlaced && root.typingTarget !== "dynCursorEditor"
                                                SequentialAnimation on opacity {
                                                    loops: Animation.Infinite
                                                    NumberAnimation { to: 0; duration: 400 }
                                                    NumberAnimation { to: 1; duration: 400 }
                                                }
                                            }
                                        }
                                    }
                                    Text { text: "check_circle"; font.family: "Material Symbols Outlined"; font.pixelSize: 16; color: successGreen; visible: root.dynCursorPlaced }
                                }
                            }

                            // Row 4: Any-other-app window
                            Rectangle {
                                Layout.fillWidth: true; Layout.preferredHeight: 140; radius: 14; color: AppTheme.background; border.color: AppTheme.hoverBg; border.width: 1; clip: true
                                ColumnLayout { anchors.fill: parent; spacing: 0
                                    Rectangle {
                                        Layout.fillWidth: true; Layout.preferredHeight: 28; color: AppTheme.surface
                                        RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12
                                            Rectangle { width: 24; height: 24; radius: 12; color: AppTheme.primary
                                                Text { anchors.centerIn: parent; text: "smartphone"; font.family: "Material Symbols Outlined"; font.pixelSize: 13; color: "white" }
                                            }
                                            Text { Layout.fillWidth: true; text: "Any other app"; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: AppTheme.textPrimary }
                                            Text { text: "more_vert"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: AppTheme.textSecondary }
                                        }
                                    }
                                    Item {
                                        Layout.fillWidth: true; Layout.fillHeight: true
                                        Text { anchors.centerIn: parent; text: "Type .cursor here and the cursor lands on your line"; font.family: "Inter"; font.pixelSize: 11; color: AppTheme.textSecondary; visible: !root.dynCursorAppExpanded }
                                        Rectangle {
                                            id: dynCursorBubble
                                            anchors.right: parent.right; anchors.bottom: parent.bottom
                                            anchors.rightMargin: 12; anchors.bottomMargin: 10
                                            width: Math.min(parent.width - 24, 360)
                                            height: root.dynCursorAppExpanded ? 66 : 0
                                            radius: 12; color: AppTheme.primary; clip: true
                                            opacity: root.dynCursorAppExpanded ? 1 : 0
                                            scale: root.dynCursorAppExpanded ? 1 : 0.92
                                            Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.OutCubic } }
                                            Behavior on scale { NumberAnimation { duration: 240; easing.type: Easing.OutBack } }
                                            Behavior on height { NumberAnimation { duration: 240 } }
                                            ColumnLayout {
                                                anchors.fill: parent; anchors.margins: 10; spacing: 4
                                                RowLayout { spacing: 5
                                                    Text { text: "auto_awesome"; font.family: "Material Symbols Outlined"; font.pixelSize: 12; color: "white" }
                                                    Text { text: "Expanded — cursor ready"; font.family: "Inter"; font.pixelSize: 8; font.bold: true; color: "white" }
                                                }
                                                RowLayout { spacing: 3; Layout.fillWidth: true
                                                    Text {
                                                        text: root.dynCursorAppExpandedPrefix; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white"; verticalAlignment: Text.AlignVCenter
                                                        Loader { active: root.typingTarget === "dynCursorAppPrefix"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                                    }
                                                    Rectangle {
                                                        width: 2; height: 15; radius: 1; color: "white"
                                                        visible: root.dynCursorAppExpanded && root.typingTarget !== "dynCursorAppPrefix"
                                                        SequentialAnimation on opacity {
                                                            loops: Animation.Infinite
                                                            NumberAnimation { to: 0; duration: 400 }
                                                            NumberAnimation { to: 1; duration: 400 }
                                                        }
                                                    }
                                                    Text {
                                                        Layout.fillWidth: true; text: root.dynCursorAppExpandedSuffix; font.family: "Inter"; font.pixelSize: 12; font.bold: true; color: "white"; wrapMode: Text.WordWrap; verticalAlignment: Text.AlignVCenter
                                                        Loader { active: root.typingTarget === "dynCursorAppSuffix"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Rectangle {
                                        id: dynCursorAppRect; objectName: "dynCursorAppRect"
                                        Layout.fillWidth: true; Layout.preferredHeight: 34; color: AppTheme.surface
                                        RowLayout { anchors.fill: parent; anchors.leftMargin: 12; anchors.rightMargin: 12; spacing: 8
                                            Rectangle {
                                                Layout.fillWidth: true; Layout.preferredHeight: 28; radius: 14; color: AppTheme.hoverBg
                                                border.color: root.dynCursorAppExpanded ? successGreen : (root.dynCursorAppText !== "" ? AppTheme.primary : "transparent"); border.width: 1
                                                Behavior on border.color { ColorAnimation { duration: 200 } }
                                                Loader { active: root.spotlight === "dynCursorApp"; sourceComponent: spotGlow }
                                                Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; text: root.dynCursorAppText; font.family: "Courier New"; font.pixelSize: 12; font.bold: true; color: AppTheme.primary; visible: root.dynCursorAppText !== "" && !root.dynCursorAppExpanded
                                                    Loader { active: root.typingTarget === "dynCursorApp"; sourceComponent: caretComp; anchors.left: parent.left; anchors.leftMargin: parent.contentWidth + 1; anchors.verticalCenter: parent.verticalCenter }
                                                }
                                                Text { anchors.fill: parent; anchors.margins: 10; verticalAlignment: Text.AlignVCenter; text: root.dynCursorAppExpanded ? "Saved with cursor ready" : "Type .cursor…"; font.family: "Inter"; font.pixelSize: 11; color: root.dynCursorAppExpanded ? successText : AppTheme.textSecondary; font.bold: true; visible: root.dynCursorAppText === "" || root.dynCursorAppExpanded }
                                            }
                                            Rectangle {
                                                width: 28; height: 28; radius: 14; color: root.dynCursorAppExpanded ? successGreen : AppTheme.primary
                                                Behavior on color { ColorAnimation { duration: 200 } }
                                                Text { anchors.centerIn: parent; text: "send"; font.family: "Material Symbols Outlined"; font.pixelSize: 15; color: "white" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        }
                    }

                }
            }
        }
    }
}
