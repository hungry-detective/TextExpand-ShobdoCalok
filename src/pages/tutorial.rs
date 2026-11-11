//! Port of Components/TutorialPage.qml — the animated, self-playing tutorial:
//! a 34 ms script engine types into mock widgets, an animated mouse pointer
//! glides to the spotlighted element and "clicks" it, success targets pop.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashMap;

use crate::anim::{self, Ease};
use crate::app::App;
use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{f, fb, icon_font, mono_b};
use crate::svg;
use crate::theme::{self, Theme};

const TICK: f64 = 0.034;

#[derive(Clone, Debug)]
enum Kind {
    Type { chars: Vec<char>, len: usize },
    Pause { ms: i32 },
    Action,
}

#[derive(Clone, Debug)]
struct Fr {
    kind: Kind,
    target: &'static str,
    spotlight: Option<&'static str>,
    typing: Option<&'static str>,
    text: Option<String>,
    detail: Option<String>,
}

fn pause(ms: i32, spot: &'static str) -> Fr {
    Fr { kind: Kind::Pause { ms }, target: "", spotlight: Some(spot), typing: None, text: None, detail: None }
}
fn pause_t(ms: i32, spot: &'static str, text: &str, detail: &str) -> Fr {
    Fr { text: Some(text.into()), detail: Some(detail.into()), ..pause(ms, spot) }
}
fn pause_ty(ms: i32, spot: &'static str, typing: &'static str, text: &str, detail: &str) -> Fr {
    Fr { typing: Some(typing), ..pause_t(ms, spot, text, detail) }
}
fn typ(target: &'static str, chars: &str, len: usize, text: &str, detail: &str) -> Fr {
    Fr {
        kind: Kind::Type { chars: chars.chars().collect(), len },
        target,
        spotlight: None,
        typing: None,
        text: Some(text.into()),
        detail: Some(detail.into()),
    }
}
fn act(target: &'static str, spot: &'static str, text: &str, detail: &str) -> Fr {
    Fr { kind: Kind::Action, target, spotlight: Some(spot), typing: None, text: Some(text.into()), detail: Some(detail.into()) }
}

#[derive(Default)]
pub struct TutorialState {
    init: bool,
    pub current_step: usize,
    current_example: usize,
    dyn_sub_step: usize,
    instruction: String,
    instruction_detail: String,
    // step 0
    abbrev_text: String,
    content_text: String,
    folder_selected: bool,
    saved: bool,
    saved_demo_text: String,
    saved_demo_expanded: bool,
    // step 1
    ex_typed: String,
    ex_expanded: bool,
    ex_expanded1: bool,
    ex_expanded2: bool,
    // step 2
    dyn_date_resolved: bool,
    dyn_time_resolved: bool,
    dyn_field_resolved: bool,
    dyn_field_value: String,
    dyn_field_popup_open: bool,
    dyn_field_trigger_text: String,
    dyn_field_inserted: bool,
    dyn_cursor_text: String,
    dyn_editor_text: String,
    dyn_cursor_placed: bool,
    dyn_cursor_inserted: bool,
    dyn_result_typed: String,
    dyn_result_expanded: bool,
    dyn_field_editor_text: String,
    dyn_cursor_editor_text: String,
    dyn_cursor_trigger_text: String,
    dyn_cursor_app_text: String,
    dyn_cursor_app_expanded_text: String,
    dyn_cursor_app_prefix: String,
    dyn_cursor_app_suffix: String,
    dyn_cursor_app_expanded: bool,
    dyn_field_app_text: String,
    dyn_field_app_expanded_text: String,
    dyn_field_app_expanded: bool,
    // animation UI state
    typing_target: String,
    spotlight: String,
    anim_running: bool,
    // engine
    script: Vec<Fr>,
    pos: usize,
    idx: usize,
    pause_left: i32,
    running: bool,
    acc: f64,
    last_t: Option<f64>,
    advance_at: Option<f64>,
    // mouse cursor
    cursor_visible: bool,
    cursor_target: Pos2,
    pos_timer_at: Option<f64>,
    click_at: Option<f64>,
    ripple_at: Option<f64>,
    rects: HashMap<&'static str, Rect>,
}

impl TutorialState {
    fn reset_demo(&mut self) {
        self.running = false;
        self.advance_at = None;
        self.abbrev_text.clear();
        self.content_text.clear();
        self.folder_selected = false;
        self.saved = false;
        self.saved_demo_text.clear();
        self.saved_demo_expanded = false;
        self.ex_typed.clear();
        self.ex_expanded = false;
        self.ex_expanded1 = false;
        self.ex_expanded2 = false;
        self.dyn_date_resolved = false;
        self.dyn_time_resolved = false;
        self.dyn_field_resolved = false;
        self.dyn_field_value.clear();
        self.dyn_field_popup_open = false;
        self.dyn_field_trigger_text.clear();
        self.dyn_field_inserted = false;
        self.dyn_cursor_text.clear();
        self.dyn_cursor_placed = false;
        self.dyn_cursor_inserted = false;
        self.dyn_cursor_app_text.clear();
        self.dyn_cursor_app_expanded_text.clear();
        self.dyn_cursor_app_prefix.clear();
        self.dyn_cursor_app_suffix.clear();
        self.dyn_cursor_app_expanded = false;
        self.dyn_field_app_text.clear();
        self.dyn_field_app_expanded_text.clear();
        self.dyn_field_app_expanded = false;
        self.dyn_editor_text.clear();
        self.dyn_result_typed.clear();
        self.dyn_result_expanded = false;
        self.dyn_field_editor_text.clear();
        self.dyn_cursor_editor_text.clear();
        self.dyn_cursor_trigger_text.clear();
        self.typing_target.clear();
        self.instruction.clear();
        self.instruction_detail.clear();
        self.pos = 0;
        self.idx = 0;
        self.pause_left = -1;
    }

    fn start(&mut self, script: Vec<Fr>) {
        self.script = script;
        self.running = true;
        self.acc = 0.0;
    }

    fn start_demo(&mut self, now: f64) {
        self.reset_demo();
        self.set_spotlight("", now);
        match self.current_step {
            0 => {
                let s = vec![
                    pause_t(2000, "folder", "Step 1 — Choose a folder", "Click the folder dropdown to organize your snippets."),
                    act("folder", "folder", "Folder selected!", "Your snippet is now in 'General'."),
                    pause(1000, "folder"),
                    pause_t(2000, "abbrev", "Step 2 — Type a trigger word", "This is the shortcut you'll type to expand text."),
                    typ("abbrev", ".hello", 6, "Typing .hello…", "Try any word starting with a dot."),
                    pause(1200, "abbrev"),
                    pause_t(2000, "content", "Step 3 — Write the expansion", "This text appears when you type the trigger."),
                    typ("content", "Hello, how are you?", 19, "Typing the expansion…", "This is what gets inserted."),
                    pause(1200, "content"),
                    pause_t(2000, "save", "Step 4 — Save the snippet", "Click the save button."),
                    act("save", "save", "Saved!", "Your snippet is ready to use."),
                    pause(1200, "save"),
                    pause_t(2000, "savedDemo", "Step 5 — Try it out!", "Type .hello anywhere and it expands."),
                    typ("savedDemo", ".hello", 6, "Typing .hello…", "Watch the magic happen."),
                    pause(800, "savedDemo"),
                    act("savedExpand", "savedExpand", "Expanded!", ".hello became 'Hello, how are you?'"),
                    pause(2500, "savedExpand"),
                ];
                self.start(s);
            }
            1 => self.start_example(now),
            _ => self.start_dyn_step(now),
        }
    }

    fn start_dyn_step(&mut self, now: f64) {
        self.reset_demo();
        self.set_spotlight("", now);
        let s = match self.dyn_sub_step {
            0 => vec![
                pause_t(2000, "dynTrigger", "Step 1 — Type the trigger word", "Type .mtg to start the snippet."),
                typ("dynTrigger", ".mtg", 4, "Typing '.mtg'…", "The trigger word opens the snippet."),
                pause(1200, "dynTrigger"),
                act("openPopup", "dynEditor", "Snippet opened!", "Now type your message with auto-fill words."),
                pause_ty(2000, "dynEditor", "dynEditor", "Step 2 — Type your message", "Type the start of your message."),
                typ("dynEditor", "Meeting on ", 11, "Typing 'Meeting on…'", "Now we'll add today's date."),
                pause(1000, "dynEditor"),
                pause_t(2000, "date", "Step 3 — Click DATE", "It adds {date}, which becomes today's date."),
                act("addDate", "date", "{date} added!", "{date} will turn into today's date."),
                pause(1200, "dynEditor"),
                typ("dynEditor", " at ", 4, "Typing ' at '…", "Now we'll add the current time."),
                pause(800, "dynEditor"),
                pause_t(2000, "time", "Step 4 — Click TIME", "It adds {time}, which becomes the current time."),
                act("addTime", "time", "{time} added!", "{time} will turn into the current time."),
                pause(1200, "dynEditor"),
                act("date", "dynEditor", "Now they fill themselves in!", "{date} became '1 August 2026', {time} became '02:30 PM'."),
                act("time", "dynEditor", "Done!", "Every time you type .mtg, the date & time stay current."),
                pause(1500, "dynEditor"),
                pause_ty(2000, "dynExpand", "dynExpand", "Step 5 — Use the shortcut", "Type .mtg and it expands into the whole message."),
                typ("dynExpand", ".mtg", 4, "Typing '.mtg'…", "The trigger word expands instantly."),
                pause(700, "dynExpand"),
                act("dynExpand", "dynExpand", "Expanded!", ".mtg became the full message with live date & time."),
                pause(2500, "dynExpand"),
            ],
            1 => vec![
                pause_ty(2000, "dynTrigger", "dynTrigger", "Step 1 — Type the trigger word", "Type .price to start a fill-in snippet."),
                typ("dynTrigger", ".price", 6, "Typing '.price'…", "The trigger word opens the fill-in form."),
                pause(1000, "dynTrigger"),
                pause_ty(2000, "dynFieldEditor", "dynFieldEditor", "Step 2 — Type the expansion word", "The message appears in the editor."),
                typ("dynFieldEditor", "The price of the product is ", 29, "Typing 'The price of the product…'", "Now we'll add a dynamic field."),
                pause(1000, "dynFieldEditor"),
                pause_t(2000, "dynFieldBtn", "Step 3 — Click FIELD", "Click FIELD to insert {field:Label} into the snippet."),
                act("insertField", "dynFieldEditor", "Field inserted!", "{field:Label} marks where your value will go."),
                pause_ty(2000, "dynFieldApp", "dynFieldApp", "Step 4 — Use it in any other app", "Type the trigger word in any other app — like a chat or email."),
                typ("dynFieldApp", ".price", 6, "Typing '.price'…", "The trigger word works everywhere, not just here."),
                pause(800, "dynFieldApp"),
                act("openPopup", "dynFieldPopup", "Form opened!", "Type the value in the highlighted box."),
                pause_t(2000, "dynFieldPopup", "Step 5 — Type the value", "Type the price into the input box."),
                typ("dynField", "$49", 3, "Typing '$49'…", "This value replaces {field:Label}."),
                pause(800, "dynField"),
                act("submitField", "dynFieldApp", "Pasted right!", "$49 replaced {field:Label} and your snippet expanded."),
                act("fieldAppExpand", "dynFieldApp", "Expanding…", "The price is filled right into the line."),
                pause_ty(1000, "dynFieldApp", "dynFieldAppExpand", "Typing the expansion…", "Your line appears with the price already filled in."),
                typ("dynFieldAppExpand", "The price of the product is $49.", 33, "Expanded!", "The field was filled automatically — ready to send."),
            ],
            _ => vec![
                pause_ty(2000, "dynCursorTrigger", "dynCursorTrigger", "Step 1 — Type the trigger word", "Type .cursor to start a snippet with a cursor."),
                typ("dynCursorTrigger", ".cursor", 7, "Typing '.cursor'…", "The trigger word opens the cursor snippet."),
                pause(1000, "dynCursorTrigger"),
                pause_ty(2000, "dynCursorEditor", "dynCursorEditor", "Step 2 — Type the first part", "The words before the cursor appear in the editor."),
                typ("dynCursorEditor", "Hi", 2, "Typing 'Hi'…", "Now we'll place the cursor right in the middle."),
                pause(800, "dynCursorEditor"),
                pause_t(2000, "dynCursorBtn", "Step 3 — Click CURSOR", "Click CURSOR to put the cursor mid-sentence."),
                act("insertCursor", "dynCursorEditor", "Cursor is there!", "The {cursor} marker sits in the middle and stays there."),
                pause_ty(1500, "dynCursorEditor", "dynCursorEditor", "Step 4 — Type the rest", "Finish your sentence after the cursor."),
                typ("dynCursorEditor", " How are you today", 18, "Typing 'How are you today…'", "Now the line reads: Hi {cursor} How are you today"),
                pause(1000, "dynCursorEditor"),
                pause_ty(2000, "dynCursorApp", "dynCursorApp", "Step 5 — Use it in any other app", "Type the trigger word in any other app — like a chat or email."),
                typ("dynCursorApp", ".cursor", 7, "Typing '.cursor'…", "The trigger word works everywhere, not just here."),
                pause(800, "dynCursorApp"),
                act("cursorAppExpand", "dynCursorApp", "Expanding…", "The cursor lands right in the middle of your sentence."),
                pause_ty(1000, "dynCursorApp", "dynCursorAppPrefix", "Expanding…", "The first part of your sentence appears."),
                typ("dynCursorAppPrefix", "Hi", 2, "Expanding…", "The cursor blinks mid-sentence."),
                pause_ty(1500, "dynCursorApp", "", "Cursor is ready", "The blinking cursor sits in the middle — type your name there."),
                pause_ty(900, "dynCursorApp", "dynCursorAppSuffix", "Finishing the sentence…", "The rest of your sentence follows the cursor."),
                typ("dynCursorAppSuffix", "How are you today", 17, "Expanded!", "Hi [cursor] How are you today — just fill in the middle."),
                Fr { typing: Some(""), ..pause(2500, "dynCursorApp") },
            ],
        };
        self.start(s);
    }

    fn start_example(&mut self, now: f64) {
        self.reset_demo();
        self.set_spotlight("", now);
        let names = [".email", ".hello", ".ttyl"];
        let apps = ["Email", "WhatsApp", "Messenger"];
        let expand_texts = ["Email filled ✓", "Message sent ✓", "Talk to you later!"];
        let ce = self.current_example;
        if ce == 1 {
            let s = vec![
                pause_ty(2000, "ex", "ex", "Type .omw in the WhatsApp input", "Watch it expand into a message."),
                typ("ex", ".omw", 4, "Typing .omw…", "The trigger word is being typed."),
                pause(800, "ex"),
                act("exExpand1", "ex", "Expanded!", ".omw became 'On my way!' and sent automatically."),
                pause(2200, "ex"),
                pause_t(1500, "ex", "Now type .hello", "It becomes another message."),
                typ("ex", ".hello", 6, "Typing .hello…", "The trigger word is being typed."),
                pause(800, "ex"),
                act("exExpand2", "ex", "Expanded!", ".hello became 'Hello! How are you?' and sent automatically."),
                pause(2500, "ex"),
            ];
            self.start(s);
            return;
        }
        let name = names[ce];
        let spot: &'static str = if ce == 0 { "exToField" } else { "ex" };
        let mut t = typ("ex", name, name.chars().count(), &format!("Typing {name}…"), "The trigger word is being typed.");
        t.spotlight = Some(spot);
        let s = vec![
            pause_ty(2000, spot, "ex", &format!("Type {name} in the {} input", apps[ce]), "Watch it expand instantly."),
            t,
            pause(800, spot),
            act("exExpand", spot, "Expanded!", &format!("{}{}", expand_texts[ce], if ce == 2 { " and sent automatically." } else { "" })),
            pause(2500, spot),
        ];
        self.start(s);
    }

    fn set_spotlight(&mut self, s: &str, now: f64) {
        if self.spotlight == s {
            return;
        }
        self.spotlight = s.to_string();
        self.click_at = None;
        self.ripple_at = None;
        if s.is_empty() {
            self.cursor_visible = false;
            self.pos_timer_at = None;
            return;
        }
        if self.element_for(s).is_none() {
            self.cursor_visible = false;
            return;
        }
        self.pos_timer_at = Some(now + 0.060);
    }

    fn element_for(&self, spot: &str) -> Option<Rect> {
        let key: &str = match spot {
            "folder" => "folderRect",
            "abbrev" => "abbrevRect",
            "content" => "contentEditor",
            "save" => "saveBtn",
            "savedDemo" | "savedExpand" => "savedDemoBox",
            "ex" => "exInputRect",
            "exExpand" => "exSendBtn",
            "exToField" => "exToField",
            "date" => "dynDateBtn",
            "time" => "dynTimeBtn",
            "dynEditor" => "dynEditorRect",
            "dynTrigger" => {
                if self.dyn_sub_step == 0 {
                    "dynTriggerRect0"
                } else {
                    "dynTriggerRect"
                }
            }
            "dynFieldEditor" => "dynFieldEditorRect",
            "dynCursorEditor" | "cursor" | "dynCursor" => "dynCursorEditorRect",
            "dynCursorTrigger" => "dynCursorTriggerRect",
            "dynCursorBtn" => "dynCursorBtn",
            "dynFieldPopup" => "dynFieldPopup",
            "dynFieldBtn" => "dynFieldBtn",
            "dynFieldResult" | "dynFieldApp" => "dynFieldAppRect",
            "dynExpand" => "dynResultBox",
            "field" => "dynFieldInput",
            "dynCursorApp" => "dynCursorAppRect",
            _ => return None,
        };
        self.rects.get(key).copied()
    }

    fn append(&mut self, target: &str, ch: char) {
        let s = match target {
            "abbrev" => &mut self.abbrev_text,
            "content" => &mut self.content_text,
            "savedDemo" => &mut self.saved_demo_text,
            "ex" => &mut self.ex_typed,
            "dynField" => &mut self.dyn_field_value,
            "dynCursor" => &mut self.dyn_cursor_text,
            "dynEditor" => &mut self.dyn_editor_text,
            "dynTrigger" => &mut self.dyn_field_trigger_text,
            "dynExpand" => &mut self.dyn_result_typed,
            "dynFieldEditor" => &mut self.dyn_field_editor_text,
            "dynCursorEditor" => &mut self.dyn_cursor_editor_text,
            "dynCursorTrigger" => &mut self.dyn_cursor_trigger_text,
            "dynCursorApp" => &mut self.dyn_cursor_app_text,
            "dynCursorAppExpand" => &mut self.dyn_cursor_app_expanded_text,
            "dynCursorAppPrefix" => &mut self.dyn_cursor_app_prefix,
            "dynCursorAppSuffix" => &mut self.dyn_cursor_app_suffix,
            "dynFieldApp" => &mut self.dyn_field_app_text,
            "dynFieldAppExpand" => &mut self.dyn_field_app_expanded_text,
            _ => return,
        };
        s.push(ch);
    }

    /// One 34 ms tick of the animation engine (animTimer.onTriggered).
    fn tick(&mut self, ctx: &egui::Context, now: f64) {
        if self.idx >= self.script.len() {
            self.anim_running = false;
            self.typing_target.clear();
            self.set_spotlight("", now);
            self.running = false;
            self.advance_at = Some(now + 1.6);
            return;
        }
        self.anim_running = true;
        let fr = self.script[self.idx].clone();
        match fr.kind {
            Kind::Type { chars, len } => {
                self.typing_target = fr.target.to_string();
                self.set_spotlight(fr.spotlight.unwrap_or(fr.target), now);
                if let Some(t) = fr.text {
                    self.instruction = t;
                }
                if let Some(d) = fr.detail {
                    self.instruction_detail = d;
                }
                if self.pos < len {
                    if let Some(ch) = chars.get(self.pos) {
                        self.append(fr.target, *ch);
                    }
                    self.pos += 1;
                } else {
                    self.pos = 0;
                    self.idx += 1;
                }
            }
            Kind::Pause { ms } => {
                if let Some(t) = fr.text {
                    self.instruction = t;
                }
                if let Some(d) = fr.detail {
                    self.instruction_detail = d;
                }
                if let Some(s) = fr.spotlight {
                    self.set_spotlight(s, now);
                }
                if let Some(t) = fr.typing {
                    self.typing_target = t.to_string();
                }
                if self.pause_left < 0 {
                    self.pause_left = ms;
                }
                self.pause_left -= 34;
                if self.pause_left <= 0 {
                    self.pause_left = -1;
                    self.idx += 1;
                }
            }
            Kind::Action => {
                if let Some(t) = fr.text {
                    self.instruction = t;
                }
                if let Some(d) = fr.detail {
                    self.instruction_detail = d;
                }
                if let Some(s) = fr.spotlight {
                    self.set_spotlight(s, now);
                }
                let before = (self.saved, self.ex_expanded, self.dyn_field_resolved, self.dyn_cursor_placed, self.dyn_date_resolved && self.dyn_time_resolved);
                match fr.target {
                    "folder" => self.folder_selected = true,
                    "save" => self.saved = true,
                    "savedExpand" => self.saved_demo_expanded = true,
                    "exExpand" => self.ex_expanded = true,
                    "exExpand1" => {
                        self.ex_expanded1 = true;
                        self.ex_typed.clear();
                    }
                    "exExpand2" => {
                        self.ex_expanded2 = true;
                        self.ex_typed.clear();
                    }
                    "date" => self.dyn_date_resolved = true,
                    "time" => self.dyn_time_resolved = true,
                    "addDate" => self.dyn_editor_text.push_str("{date}"),
                    "addTime" => self.dyn_editor_text.push_str("{time}"),
                    "openPopup" => {
                        if self.dyn_sub_step == 1 {
                            self.dyn_field_popup_open = true;
                        }
                    }
                    "insertField" => {
                        self.dyn_field_inserted = true;
                        self.dyn_field_editor_text.push_str("{field:Label}");
                    }
                    "submitField" => {
                        self.dyn_field_popup_open = false;
                        self.dyn_field_resolved = true;
                        self.dyn_field_app_text = format!("The price of the product is {}", self.dyn_field_value);
                    }
                    "field" => self.dyn_field_resolved = true,
                    "insertCursor" => {
                        self.dyn_cursor_inserted = true;
                        self.dyn_cursor_placed = true;
                        self.dyn_cursor_editor_text.push_str("{cursor}");
                        self.typing_target.clear();
                    }
                    "cursor" => self.dyn_cursor_placed = true,
                    "cursorAppExpand" => {
                        self.dyn_cursor_app_expanded = true;
                        self.dyn_cursor_app_text.clear();
                        self.dyn_cursor_app_expanded_text.clear();
                        self.dyn_cursor_app_prefix.clear();
                        self.dyn_cursor_app_suffix.clear();
                    }
                    "fieldAppExpand" => {
                        self.dyn_field_app_expanded = true;
                        self.dyn_field_app_text.clear();
                        self.dyn_field_app_expanded_text.clear();
                    }
                    "dynExpand" => {
                        self.dyn_result_expanded = true;
                        self.dyn_result_typed.clear();
                    }
                    _ => {}
                }
                // onXxxChanged → popSuccess(target)
                if !before.0 && self.saved {
                    anim::restart(ctx, Id::new(("tut-pop", "saveBtn")));
                }
                if !before.1 && self.ex_expanded && self.current_example != 2 {
                    anim::restart(ctx, Id::new(("tut-pop", if self.current_example == 0 { "exToField" } else { "exSendBtn" })));
                }
                if !before.2 && self.dyn_field_resolved {
                    anim::restart(ctx, Id::new(("tut-pop", "dynFieldInput")));
                }
                if !before.3 && self.dyn_cursor_placed {
                    anim::restart(ctx, Id::new(("tut-pop", "dynCursorEditorRect")));
                }
                if !before.4 && self.dyn_date_resolved && self.dyn_time_resolved {
                    anim::restart(ctx, Id::new(("tut-pop", "watchBtn")));
                }
                self.idx += 1;
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, now: f64) {
        let dt = self.last_t.map(|t| (now - t).min(0.5)).unwrap_or(0.0);
        self.last_t = Some(now);
        if self.running {
            self.acc += dt;
            while self.acc >= TICK && self.running {
                self.acc -= TICK;
                self.tick(ctx, now);
            }
        }
        if let Some(t) = self.advance_at {
            if now >= t {
                self.advance_at = None;
                if self.current_step == 1 {
                    self.start_example(now);
                } else {
                    self.start_demo(now);
                }
            }
        }
        // posTimer (60 ms) → place pointer, then clickDelayTimer (480 ms)
        if let Some(t) = self.pos_timer_at {
            if now >= t {
                self.pos_timer_at = None;
                if let Some(r) = self.element_for(&self.spotlight.clone()) {
                    self.cursor_target = r.center();
                    self.cursor_visible = true;
                    let s = self.spotlight.as_str();
                    if matches!(s, "folder" | "save" | "savedExpand" | "exExpand" | "date" | "time" | "dynFieldPopup" | "dynFieldResult" | "cursor") {
                        self.click_at = Some(now + 0.480);
                    }
                }
            }
        }
        if let Some(t) = self.click_at {
            if now >= t {
                self.click_at = None;
                self.ripple_at = Some(now);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

// ── Drawing helpers ─────────────────────────────────────────────────────────

struct D<'a> {
    c: Canvas,
    th: Theme,
    ctx: &'a egui::Context,
    rects: HashMap<&'static str, Rect>,
    spotlight: String,
    typing: String,
    success_green: Color32,
    success_text: Color32,
}

impl D<'_> {
    fn rec(&mut self, name: &'static str, r: Rect) {
        self.rects.insert(name, r);
    }
    fn pop(&self, name: &'static str, r: Rect) -> Canvas {
        let s = anim::pulse(self.ctx, Id::new(("tut-pop", name)), 1.12, 110.0, Ease::OutQuad, 190.0, Ease::OutBack);
        self.c.scaled(r.center(), s)
    }
    /// spotGlow component
    fn glow(&self, c: &Canvas, r: Rect, radius: f32, on: bool) {
        if !on {
            return;
        }
        let op = anim::ping_pong(self.ctx, 0.9, 0.35, 380.0, Ease::Linear);
        c.faded(op).rect_stroke(r, radius + 4.0, 2.0, self.th.primary);
    }
    /// caretComp next to text ending at `x`
    fn caret(&self, c: &Canvas, x: f32, cy: f32, on: bool) {
        if !on {
            return;
        }
        let op = anim::ping_pong(self.ctx, 1.0, 0.0, 300.0, Ease::Linear);
        c.faded(op).rect(rect_xywh(x, cy - 9.0, 3.0, 18.0), 1.5, self.th.primary);
    }
    fn spot(&self, s: &str) -> bool {
        self.spotlight == s
    }
    fn typing(&self, s: &str) -> bool {
        self.typing == s
    }
    fn ac(&self, key: &str, target: Color32, ms: f32) -> Color32 {
        anim::anim_color(self.ctx, Id::new(("tut-col", key)), target, ms)
    }
    /// static token label (icon + text)
    fn token(&self, c: &Canvas, x: f32, cy: f32, icon: &str, isz: f32, label: &str, fsz: f32, col: Color32) -> f32 {
        let ir = c.icon_left(x, cy, icon, isz, col);
        let tr = c.text(Pos2::new(ir.right() + 4.0, cy), Align2::LEFT_CENTER, label, fb(fsz), col);
        tr.right() - x
    }
    /// small boxed token button (sub-steps 1/2)
    fn token_box(&self, c: &Canvas, r: Rect, pad: f32, icon: &str, isz: f32, label: &str, fsz: f32, col: Color32, bg: Color32, border: Color32) {
        c.rect_fs(r, 8.0, bg, 1.0, border);
        let ir = c.icon_left(r.left() + pad, r.center().y, icon, isz, col);
        c.text(Pos2::new(ir.right() + 4.0, r.center().y), Align2::LEFT_CENTER, label, fb(fsz), col);
    }
    fn folder_chip(&self, c: &Canvas, r: Rect, fsz: f32, isz: f32) {
        let th = self.th;
        c.rect_fs(r, 10.0, th.primary_light, 1.0, th.primary);
        let ir = c.icon_left(r.left() + 10.0, r.center().y, "folder", isz, th.primary);
        let er = c.text(Pos2::new(r.right() - 10.0, r.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("expand_more"), icon_font(isz), th.text_secondary);
        c.text_elide(Pos2::new(ir.right() + 5.0, r.center().y), er.left() - ir.right() - 10.0, "General", fb(fsz), th.text_primary);
    }
}

pub fn show(app: &mut App, ui: &mut Ui, page: Rect) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let app_icon = app.svg.get_raw(&ctx, "app-icon", svg::APP_ICON, 16.0, 16.0);
    let app_icon14 = app.svg.get_raw(&ctx, "app-icon", svg::APP_ICON, 14.0, 14.0);
    let cursor_tex = app.svg.get_raw(&ctx, "cursor-pointer", svg::CURSOR_POINTER, 22.0, 26.0);
    let st = &mut app.tutorial;
    if !st.init {
        st.init = true;
        // Developer aid for documentation screenshots: SHOBDO_TUT="step,sub,example"
        if let Ok(v) = std::env::var("SHOBDO_TUT") {
            let n: Vec<usize> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            st.current_step = n.first().copied().unwrap_or(0).min(2);
            st.dyn_sub_step = n.get(1).copied().unwrap_or(0).min(2);
            st.current_example = n.get(2).copied().unwrap_or(0).min(2);
        }
        st.start_demo(now);
    }
    st.update(&ctx, now);

    let mut d = D {
        c: Canvas::new(ui.painter()),
        th,
        ctx: &ctx,
        rects: HashMap::new(),
        spotlight: st.spotlight.clone(),
        typing: st.typing_target.clone(),
        success_green: if th.is_dark { theme::hex(0x34d399) } else { theme::hex(0x10b981) },
        success_text: if th.is_dark { Color32::WHITE } else { theme::hex(0x10b981) },
    };
    let c = d.c.clone();
    let col = page.shrink(20.0);
    let mut y = col.top();

    // Title
    let tr = c.text(Pos2::new(col.left(), y), Align2::LEFT_TOP, "Learn Shobdo Calok", fb(22.0), th.text_primary);
    y = tr.bottom() + 16.0;

    // ── Step selector cards ────────────────────────────────────────────
    let titles = ["Create a Snippet", "Type & Expand", "Dynamic Snippets"];
    let icons = ["edit_note", "bolt", "auto_fix_high"];
    let cw = (col.width() - 20.0) / 3.0;
    let mut new_step = None;
    for k in 0..3 {
        let r = rect_xywh(col.left() + k as f32 * (cw + 10.0), y, cw, 64.0);
        let resp = ui.interact(r, Id::new(("tut-card", k)), Sense::CLICK);
        let sel = st.current_step == k;
        let s = anim::anim(&ctx, Id::new(("tut-card-s", k)), (if sel { 1.02 } else { 1.0 }) * (if resp.hovered() { 1.02 } else { 1.0 }), 150.0, Ease::Linear);
        let cc = c.scaled(r.center(), s);
        let bg = d.ac(&format!("card-bg{k}"), if sel { th.primary_light } else { th.surface }, 200.0);
        let bc = d.ac(&format!("card-bc{k}"), if sel { th.primary } else { th.hover_bg }, 200.0);
        cc.rect_fs(r, 14.0, bg, 1.0, bc);
        let ib = rect_xywh(r.left() + 12.0, r.center().y - 17.0, 34.0, 34.0);
        cc.rect(ib, 10.0, d.ac(&format!("card-ib{k}"), if sel { th.primary } else { th.hover_bg }, 200.0));
        cc.icon(ib.center(), icons[k], 18.0, if sel { Color32::WHITE } else { th.primary });
        let nc = Pos2::new(ib.right() + 10.0 + 8.0, r.center().y);
        cc.circle(nc, 8.0, if sel { th.primary } else { th.hover_bg });
        cc.text(nc, Align2::CENTER_CENTER, &(k + 1).to_string(), fb(9.0), if sel { Color32::WHITE } else { th.text_secondary });
        cc.text_elide(Pos2::new(nc.x + 8.0 + 6.0, r.center().y), r.right() - 12.0 - (nc.x + 14.0), titles[k], fb(13.0), th.text_primary);
        if resp.clicked() {
            new_step = Some(k);
        }
    }
    y += 64.0 + 16.0;

    // ── Instruction banner ─────────────────────────────────────────────
    if !st.instruction.is_empty() {
        let r = rect_xywh(col.left(), y, col.width(), 54.0);
        c.rect_fs(r, 12.0, th.surface, 1.0, d.ac("banner-bc", th.primary, 250.0));
        let ic = Pos2::new(r.left() + 12.0 + 14.0, r.center().y);
        c.circle(ic, 14.0, th.primary);
        c.icon(ic, if st.anim_running { "touch_app" } else { "check_circle" }, 16.0, Color32::WHITE);
        let tx = ic.x + 14.0 + 10.0;
        let tw = r.right() - 12.0 - tx;
        let g1 = crate::draw::layout_elide(&c.p, &st.instruction, fb(12.0), th.text_primary, tw);
        let has_d = !st.instruction_detail.is_empty();
        let g2 = crate::draw::layout_elide(&c.p, &st.instruction_detail, f(10.0), th.text_secondary, tw);
        let total = g1.size().y + if has_d { 2.0 + g2.size().y } else { 0.0 };
        let top = r.center().y - total / 2.0;
        let h1 = g1.size().y;
        c.galley(Pos2::new(tx, top), g1, th.text_primary);
        if has_d {
            c.galley(Pos2::new(tx, top + h1 + 2.0), g2, th.text_secondary);
        }
        y += 54.0 + 16.0;
    }
    if st.dyn_field_popup_open {
        y += 16.0; // the popup is (invisibly) part of the ColumnLayout
    }

    // ── Demo area ──────────────────────────────────────────────────────
    let demo = Rect::from_min_max(Pos2::new(col.left(), y + 2.0), Pos2::new(col.right(), col.bottom()));
    c.rect(demo, 16.0, th.surface);
    let inner = Rect::from_min_max(Pos2::new(demo.left() + 16.0, demo.top() + 8.0), Pos2::new(demo.right() - 16.0, demo.bottom() - 16.0));
    let dc = c.clipped(demo);

    let mut new_example = None;
    let mut new_sub = None;
    for step in 0..3 {
        let op = anim::anim(&ctx, Id::new(("tut-step-op", step)), if st.current_step == step { 1.0 } else { 0.0 }, 250.0, Ease::Linear);
        let sc = anim::anim(&ctx, Id::new(("tut-step-sc", step)), if st.current_step == step { 1.0 } else { 0.97 }, 250.0, Ease::OutCubic);
        if st.current_step != step {
            continue;
        }
        let sc_c = dc.scaled(inner.center(), sc).faded(op);
        match step {
            0 => step0(&mut d, &sc_c, st, inner, &app_icon),
            1 => new_example = step1(&mut d, ui, &sc_c, st, inner),
            _ => new_sub = step2(&mut d, ui, &sc_c, st, inner, &app_icon, &app_icon14),
        }
    }

    // ── Fill-in popup overlay (root level) ─────────────────────────────
    {
        let ph = anim::anim(&ctx, Id::new("tut-popup-h"), if st.dyn_field_popup_open { 128.0 } else { 0.0 }, 300.0, Ease::OutCubic);
        let pop_op = anim::anim(&ctx, Id::new("tut-popup-op"), if st.dyn_field_popup_open { 1.0 } else { 0.0 }, 250.0, Ease::Linear);
        if st.dyn_field_popup_open {
            let pw = col.width() * 0.6;
            let pr = rect_xywh(col.center().x - pw / 2.0, col.top() + 108.0, pw, ph);
            d.rec("dynFieldPopup", pr);
            let pc = c.faded(pop_op);
            pc.rect_fs(pr, 14.0, th.surface, 1.0, th.primary);
            let cc = pc.clipped(pr);
            let pin = pr.shrink(10.0);
            let mut py = pin.top();
            cc.circle(Pos2::new(pin.left() + 4.0, py + 8.0), 4.0, th.primary);
            cc.text(Pos2::new(pin.left() + 14.0, py + 8.0), Align2::LEFT_CENTER, "Fill in the field", fb(11.0), th.text_primary);
            cc.text(Pos2::new(pin.right(), py + 8.0), Align2::RIGHT_CENTER, &crate::fonts::icon("close"), icon_font(14.0), th.text_secondary);
            py += 16.0 + 6.0;
            cc.hline(rect_xywh(pin.left(), py, pin.width(), 1.0), th.hover_bg);
            py += 1.0 + 6.0;
            let ir = rect_xywh(pin.right() - 120.0, py, 120.0, 30.0);
            d.rec("dynFieldInput", ir);
            cc.text_wrap(Pos2::new(pin.left(), py + 6.0), ir.left() - 6.0 - pin.left(), "The price of the product is", f(12.0), th.text_secondary);
            let pcc = d.pop("dynFieldInput", ir).faded(pop_op).clipped(pr);
            pcc.rect_fs(ir, 8.0, if st.dyn_field_resolved { th.primary_light } else { th.hover_bg }, 1.0, d.ac("fi-bc", if st.dyn_field_resolved { d.success_green } else { th.primary }, 200.0));
            d.glow(&pcc, ir, 8.0, d.spot("field"));
            let vr = pcc.text(Pos2::new(ir.left() + 8.0, ir.center().y), Align2::LEFT_CENTER, &st.dyn_field_value, fb(13.0), th.text_primary);
            d.caret(&pcc, vr.right() + 1.0, ir.center().y, d.typing("dynField"));
            if st.dyn_field_resolved {
                pcc.icon(Pos2::new(ir.right() - 8.0 - 7.0, ir.center().y), "check_circle", 14.0, d.success_green);
            }
            py += 30.0 + 6.0;
            cc.hline(rect_xywh(pin.left(), py, pin.width(), 1.0), th.hover_bg);
            py += 1.0 + 6.0;
            let sb = rect_xywh(pin.left(), py, pin.width(), 28.0);
            cc.rect(sb, 8.0, th.primary);
            cc.text(sb.center(), Align2::CENTER_CENTER, "Submit", fb(11.0), Color32::WHITE);
            d.glow(&pc, pr, 14.0, d.spot("dynFieldPopup"));
        }
    }

    // ── Mouse pointer + click ripple ───────────────────────────────────
    let cx = anim::anim(&ctx, Id::new("tut-cur-x"), st.cursor_target.x, 400.0, Ease::OutCubic);
    let cy = anim::anim(&ctx, Id::new("tut-cur-y"), st.cursor_target.y, 400.0, Ease::OutCubic);
    if let Some(t) = st.ripple_at {
        let el = ((now - t) * 1000.0) as f32;
        if el < 350.0 {
            c.faded(0.9 * (1.0 - el / 350.0)).circle(Pos2::new(st.cursor_target.x, st.cursor_target.y), 5.0, th.primary);
        } else {
            st.ripple_at = None;
        }
    }
    if st.cursor_visible {
        c.image(&cursor_tex, rect_xywh(cx - 1.0, cy - 1.0, 22.0, 26.0), Color32::WHITE);
    }

    st.rects = d.rects;

    if let Some(k) = new_step {
        if k != st.current_step {
            st.current_step = k;
            st.start_demo(now);
        }
    }
    if let Some(k) = new_example {
        if k != st.current_example {
            st.current_example = k;
            st.start_example(now);
        }
    }
    if let Some(k) = new_sub {
        if k != st.dyn_sub_step {
            st.dyn_sub_step = k;
            st.start_dyn_step(now);
        }
    }
}

/// Header row shared by Step 0 / Sub-step 0 (title, folder, trigger, delete).
fn trigger_box(d: &mut D, c: &Canvas, r: Rect, name: &'static str, text: &str, typing_key: &str, icon: &egui::TextureHandle, isz: f32, fsz: f32, spot_key: &str) {
    let th = d.th;
    c.rect_fs(r, 10.0, th.surface, 1.0, d.ac(&format!("{name}-bc"), if !text.is_empty() { th.primary } else { th.hover_bg }, 200.0));
    d.glow(c, r, 10.0, d.spot(spot_key));
    c.faded(0.7).image(icon, rect_xywh(r.left() + 10.0, r.center().y - isz / 2.0, isz, isz), Color32::WHITE);
    let tx = r.left() + 10.0 + isz + 6.0;
    if text.is_empty() {
        c.text(Pos2::new(tx, r.center().y), Align2::LEFT_CENTER, "trigger word", mono_b(fsz), th.text_secondary);
    } else {
        let tr = c.text(Pos2::new(tx, r.center().y), Align2::LEFT_CENTER, text, mono_b(fsz), th.primary);
        d.caret(c, tr.right() + 1.0, r.center().y, d.typing(typing_key));
    }
    d.rec(name, r);
}

fn step0(d: &mut D, c: &Canvas, st: &TutorialState, inner: Rect, icon: &egui::TextureHandle) {
    let th = d.th;
    let mut y = inner.top();
    // Row 1
    let del = rect_xywh(inner.right() - 34.0, y + 1.0, 34.0, 32.0);
    let ab = rect_xywh(del.left() - 10.0 - 130.0, y, 130.0, 34.0);
    let fr = rect_xywh(ab.left() - 10.0 - 160.0, y, 160.0, 34.0);
    c.text(Pos2::new(inner.left(), y + 17.0), Align2::LEFT_CENTER, "Snippet Editor", fb(16.0), th.text_primary);
    c.rect_fs(fr, 10.0, if st.folder_selected { th.primary_light } else { th.surface }, 1.0, if st.folder_selected { th.primary } else { th.hover_bg });
    d.glow(c, fr, 10.0, d.spot("folder"));
    let fi = c.icon_left(fr.left() + 10.0, fr.center().y, "folder", 14.0, if st.folder_selected { th.primary } else { th.text_secondary });
    let right = if st.folder_selected {
        c.text(Pos2::new(fr.right() - 10.0, fr.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("expand_more"), icon_font(14.0), th.text_secondary).left() - 5.0
    } else {
        fr.right() - 10.0
    };
    c.text_elide(Pos2::new(fi.right() + 5.0, fr.center().y), right - fi.right() - 5.0, if st.folder_selected { "General" } else { "— no folder —" }, fb(11.0), if st.folder_selected { th.text_primary } else { th.text_secondary });
    d.rec("folderRect", fr);
    trigger_box(d, c, ab, "abbrevRect", &st.abbrev_text, "abbrev", icon, 16.0, 12.0, "abbrev");
    c.rect(del, 10.0, th.danger_light);
    c.icon(del.center(), "delete", 18.0, th.danger);
    y += 34.0 + 14.0;

    // Row 2: INSERT + tokens + save
    let cy = y + 16.0;
    let ins = c.text(Pos2::new(inner.left(), cy), Align2::LEFT_CENTER, "INSERT", fb(9.0), th.text_secondary);
    let mut x = ins.right() + 8.0;
    for (icon_n, label, hl) in [("content_copy", "CLIP", false), ("calendar_today", "DATE", false), ("schedule", "TIME", false), ("input", "FIELD", true), ("text_fields", "CURSOR", true)] {
        x += d.token(c, x, cy, icon_n, 16.0, label, 9.0, if hl { th.primary } else { th.text_secondary }) + 8.0;
    }
    let sb = rect_xywh(inner.right() - 130.0, y, 130.0, 32.0);
    let pc = d.pop("saveBtn", sb);
    pc.rect(sb, 8.0, d.ac("saveBtn", if st.saved { d.success_green } else { th.primary_hover }, 200.0));
    d.glow(&pc, sb, 8.0, d.spot("save"));
    pc.text(sb.center(), Align2::CENTER_CENTER, if st.saved { "SAVED!" } else { "SAVE CHANGES" }, fb(9.0), Color32::WHITE);
    d.rec("saveBtn", sb);
    y += 32.0 + 14.0;

    // Editor
    let ed = Rect::from_min_max(Pos2::new(inner.left(), y), Pos2::new(inner.right(), (inner.bottom() - 96.0 - 14.0).max(y)));
    c.rect(ed, 16.0, th.surface);
    d.glow(c, ed, 16.0, d.spot("content"));
    let ec = c.clipped(ed);
    if st.content_text.is_empty() {
        ec.text_wrap(ed.min + Vec2::splat(16.0), ed.width() - 32.0, "Select a snippet from the library to edit it…", f(13.0), th.text_secondary);
    } else {
        let tr = ec.text_wrap(ed.min + Vec2::splat(16.0), ed.width() - 32.0, &st.content_text, f(13.0), th.text_primary);
        d.caret(&ec, 16.0 + ed.left() + tr.width().min(ed.width() - 32.0), ed.top() + 18.0 + 9.0, d.typing("content"));
    }
    d.rec("contentEditor", ed);

    // Saved confirmation
    let sr = rect_xywh(inner.left(), inner.bottom() - 96.0, inner.width(), 96.0);
    c.rect_fs(sr, 12.0, th.surface, 1.0, d.ac("saved-bc", if st.saved { d.success_green } else { Color32::TRANSPARENT }, 300.0));
    let si = sr.shrink(10.0);
    if st.saved {
        c.icon_left(si.left(), si.top() + 8.0, "check_circle", 14.0, d.success_green);
    }
    let msg = if st.saved { "Snippet saved! Type .hello anywhere and it expands instantly." } else { "Fill the fields above, then press Save — watch it work!" };
    let iw = crate::widgets::icon_text_size(c, "check_circle", 14.0).x;
    c.text(Pos2::new(si.left() + iw + 6.0, si.top() + 8.0), Align2::LEFT_CENTER, msg, fb(10.0), d.ac("saved-msg", if st.saved { d.success_text } else { th.text_secondary }, 300.0));
    let bx = rect_xywh(si.left(), si.top() + 16.0 + 6.0, si.width(), 38.0);
    c.rect_fs(bx, 9.0, th.hover_bg, 1.0, d.ac("sdb-bc", if st.saved_demo_expanded { d.success_green } else { Color32::TRANSPARENT }, 200.0));
    d.glow(c, bx, 9.0, d.spot("savedDemo") || d.spot("savedExpand"));
    let bi = bx.shrink(10.0);
    let ki = c.icon_left(bi.left(), bi.center().y, "keyboard", 15.0, th.text_secondary);
    c.text(Pos2::new(bi.right(), bi.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("send"), icon_font(15.0), if st.saved_demo_expanded { d.success_green } else { th.primary });
    let (txt, font, colr) = if st.saved_demo_expanded {
        ("Hello, how are you?".to_string(), fb(12.0), d.success_text)
    } else {
        (st.saved_demo_text.clone(), f(12.0), th.text_primary)
    };
    let tr = c.text(Pos2::new(ki.right() + 6.0, bi.center().y), Align2::LEFT_CENTER, &txt, font, d.ac("sdb-tc", colr, 200.0));
    d.caret(c, tr.right() + 2.0, bi.center().y, d.typing("savedDemo"));
    d.rec("savedDemoBox", bx);
}

fn step1(d: &mut D, ui: &mut Ui, c: &Canvas, st: &TutorialState, inner: Rect) -> Option<usize> {
    let th = d.th;
    let mut out = None;
    let mut y = inner.top();
    c.text(Pos2::new(inner.left(), y + 15.0), Align2::LEFT_CENTER, "Pick an app:", fb(11.0), th.text_secondary);
    let ex = [("Email", "mail"), ("WhatsApp", "chat"), ("Messenger", "forum")];
    for k in 0..3 {
        let r = rect_xywh(inner.right() - (3 - k) as f32 * 108.0 - (2 - k) as f32 * 8.0, y, 108.0, 30.0);
        let sel = st.current_example == k;
        c.rect(r, 15.0, if sel { th.primary } else { th.hover_bg });
        let ir = c.icon_left(r.left() + 12.0, r.center().y, ex[k].1, 14.0, if sel { Color32::WHITE } else { th.text_secondary });
        c.text_elide(Pos2::new(ir.right() + 5.0, r.center().y), r.right() - 12.0 - ir.right() - 5.0, ex[k].0, fb(10.0), if sel { Color32::WHITE } else { th.text_primary });
        if ui.interact(r, Id::new(("tut-ex", k)), Sense::CLICK).clicked() {
            out = Some(k);
        }
    }
    y += 30.0 + 10.0;

    let win = Rect::from_min_max(Pos2::new(inner.left(), y), inner.max);
    c.rect_fs(win, 14.0, th.background, 1.0, th.hover_bg);
    let ce = st.current_example;
    // header
    let hr = rect_xywh(win.left(), win.top(), win.width(), 40.0);
    c.clipped(win).rect(hr, 0.0, th.surface);
    let cc = Pos2::new(hr.left() + 14.0 + 13.0, hr.center().y);
    c.circle(cc, 13.0, [theme::hex(0xea4335), theme::hex(0x25d366), theme::hex(0x0084ff)][ce]);
    c.icon(cc, ex[ce].1, 14.0, Color32::WHITE);
    c.text(Pos2::new(cc.x + 13.0 + 5.0, hr.center().y), Align2::LEFT_CENTER, ex[ce].0, fb(13.0), th.text_primary);
    c.text(Pos2::new(hr.right() - 14.0, hr.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("more_vert"), icon_font(16.0), th.text_secondary);

    let footer_h = if ce != 0 { 46.0 } else { 0.0 };
    let body = Rect::from_min_max(Pos2::new(win.left(), hr.bottom()), Pos2::new(win.right(), win.bottom() - footer_h));
    let bc = c.clipped(body);
    let ctx = d.ctx;
    if ce == 0 {
        let b = body.shrink(16.0);
        let mut by = b.top();
        let to = bc.text(Pos2::new(b.left(), by + 15.0), Align2::LEFT_CENTER, "To:", fb(11.0), th.text_secondary);
        let check_w = if st.ex_expanded { crate::widgets::icon_text_size(c, "check_circle", 16.0).x + 8.0 } else { 0.0 };
        let tf = Rect::from_min_max(Pos2::new(to.right() + 8.0, by), Pos2::new(b.right() - check_w, by + 30.0));
        let pc = d.pop("exToField", tf).clipped(body);
        pc.rect_fs(tf, 8.0, th.hover_bg, 1.0, if !st.ex_typed.is_empty() || st.ex_expanded { th.primary } else { Color32::TRANSPARENT });
        d.glow(&pc, tf, 8.0, d.spot("exToField"));
        let (t, fnt, colr) = if st.ex_expanded { ("test@gmail.com".to_string(), fb(12.0), d.success_text) } else { (st.ex_typed.clone(), f(12.0), th.text_primary) };
        let tr = pc.text(Pos2::new(tf.left() + 10.0, tf.center().y), Align2::LEFT_CENTER, &t, fnt, d.ac("exto-c", colr, 200.0));
        d.caret(&pc, tr.right() + 1.0, tf.center().y, d.typing("ex"));
        if st.ex_expanded {
            bc.icon_left(tf.right() + 8.0, tf.center().y, "check_circle", 16.0, d.success_green);
        }
        d.rec("exToField", tf);
        by += 30.0 + 10.0;
        let sj = bc.text(Pos2::new(b.left(), by + 8.0), Align2::LEFT_CENTER, "Subject:", fb(11.0), th.text_secondary);
        bc.text(Pos2::new(sj.right() + 5.0, by + 8.0), Align2::LEFT_CENTER, "Meeting notes", f(12.0), th.text_primary);
        by += 16.0 + 10.0;
        let mb = Rect::from_min_max(Pos2::new(b.left(), by), b.max);
        bc.rect(mb, 8.0, th.hover_bg);
        bc.clipped(mb).text_wrap(mb.min + Vec2::splat(10.0), mb.width() - 20.0, "Hi Alex,\n\nHere are the notes from today's meeting.\n\nBest,\nShobdo", f(12.0), th.text_primary);
    } else {
        let b = body.shrink(14.0);
        // bottom-aligned bubble stack
        let mut items: Vec<(f32, f32, bool, &str, bool, &str)> = vec![]; // w,h,right,text,shown,key
        if ce == 1 {
            items.push((170.0, 34.0, false, "Hey! Are you coming?", true, ""));
            items.push((118.0, if st.ex_expanded1 { 34.0 } else { 0.0 }, true, "On my way!", st.ex_expanded1, "m1"));
            items.push((210.0, if st.ex_expanded2 { 36.0 } else { 0.0 }, true, "Hello! How are you? \u{1f60a}", st.ex_expanded2, "m2"));
        } else {
            items.push((100.0, 34.0, false, "you there?", true, ""));
            items.push((165.0, if st.ex_expanded { 36.0 } else { 0.0 }, true, "Talk to you later!", st.ex_expanded, "m3"));
        }
        let total: f32 = items.iter().map(|i| i.1).sum::<f32>() + 8.0 * (items.len() as f32 - 1.0);
        let mut by = b.bottom() - total;
        for (w, h, right, text, shown, key) in items {
            let r = if right { rect_xywh(b.right() - w, by, w, h) } else { rect_xywh(b.left(), by, w, h) };
            if key.is_empty() {
                bc.rect(r, 12.0, th.surface);
                bc.clipped(r).text(Pos2::new(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, text, f(12.0), th.text_primary);
            } else {
                let op = anim::anim(ctx, Id::new(("tut-msg-op", key)), if shown { 1.0 } else { 0.0 }, 240.0, Ease::OutCubic);
                let sc = anim::anim(ctx, Id::new(("tut-msg-sc", key)), if shown { 1.0 } else { 0.92 }, 240.0, Ease::OutBack);
                if h > 0.0 && op > 0.0 {
                    let mc = bc.scaled(r.center(), sc).faded(op);
                    mc.rect(r, 12.0, th.primary);
                    mc.clipped(r).text(Pos2::new(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, text, f(12.0), Color32::WHITE);
                }
            }
            by += h + 8.0;
        }
    }

    if ce != 0 {
        let fr = rect_xywh(win.left(), win.bottom() - 46.0, win.width(), 46.0);
        c.clipped(win).rect(fr, 0.0, th.surface);
        let send = rect_xywh(fr.right() - 12.0 - 32.0, fr.center().y - 16.0, 32.0, 32.0);
        let inp = Rect::from_min_max(Pos2::new(fr.left() + 12.0, fr.center().y - 16.0), Pos2::new(send.left() - 8.0, fr.center().y + 16.0));
        c.rect(inp, 16.0, th.hover_bg);
        d.glow(c, inp, 16.0, d.spot("ex"));
        if !st.ex_typed.is_empty() && !st.ex_expanded {
            let tr = c.text(Pos2::new(inp.left() + 12.0, inp.center().y), Align2::LEFT_CENTER, &st.ex_typed, mono_b(12.0), th.text_primary);
            d.caret(c, tr.right() + 1.0, inp.center().y, d.typing("ex"));
        } else {
            let t = if ce == 1 {
                if st.ex_expanded2 { "Messages sent ✓" } else if st.ex_expanded1 { "Type .hello…" } else { "Type .omw…" }
            } else if st.ex_expanded {
                "Message sent ✓"
            } else {
                "Type .ttyl…"
            };
            c.text(Pos2::new(inp.left() + 12.0, inp.center().y), Align2::LEFT_CENTER, t, fb(11.0), if st.ex_expanded || st.ex_expanded2 { d.success_text } else { th.text_secondary });
        }
        d.rec("exInputRect", inp);
        let pc = d.pop("exSendBtn", send);
        pc.circle(send.center(), 16.0, d.ac("exsend", if st.ex_expanded || st.ex_expanded2 { d.success_green } else { th.primary }, 200.0));
        d.glow(&pc, send, 16.0, d.spot("exExpand"));
        pc.icon(send.center(), "send", 16.0, Color32::WHITE);
        d.rec("exSendBtn", send);
    }
    out
}

fn step2(d: &mut D, ui: &mut Ui, c: &Canvas, st: &TutorialState, inner: Rect, icon16: &egui::TextureHandle, icon14: &egui::TextureHandle) -> Option<usize> {
    let th = d.th;
    let ctx = d.ctx;
    let mut out = None;
    // Sub-step chips
    let titles = ["Date & Time", "Fill-in Fields", "Cursor"];
    let icons = ["calendar_month", "input", "text_fields"];
    let cw = (inner.width() - 16.0) / 3.0;
    for k in 0..3 {
        let r = rect_xywh(inner.left() + k as f32 * (cw + 8.0), inner.top(), cw, 34.0);
        let resp = ui.interact(r, Id::new(("tut-sub", k)), Sense::CLICK);
        let sel = st.dyn_sub_step == k;
        let s = anim::anim(ctx, Id::new(("tut-sub-s", k)), if resp.hovered() && !sel { 1.03 } else { 1.0 }, 150.0, Ease::Linear);
        let cc = c.scaled(r.center(), s);
        cc.rect_fs(r, 10.0, d.ac(&format!("sub-bg{k}"), if sel { th.primary } else { th.surface }, 200.0), 1.0, d.ac(&format!("sub-bc{k}"), if sel { th.primary } else { th.hover_bg }, 200.0));
        let ir = cc.icon_left(r.left() + 12.0, r.center().y, icons[k], 14.0, if sel { Color32::WHITE } else { th.primary });
        cc.text_elide(Pos2::new(ir.right() + 8.0, r.center().y), r.right() - 12.0 - ir.right() - 8.0, titles[k], fb(10.0), if sel { Color32::WHITE } else { th.text_primary });
        if resp.clicked() {
            out = Some(k);
        }
    }
    let area = Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 34.0 + 12.0), inner.max);
    for sub in 0..3 {
        let op = anim::anim(ctx, Id::new(("tut-sub-op", sub)), if st.dyn_sub_step == sub { 1.0 } else { 0.0 }, 220.0, Ease::Linear);
        let sc = anim::anim(ctx, Id::new(("tut-sub-sc", sub)), if st.dyn_sub_step == sub { 1.0 } else { 0.96 }, 220.0, Ease::OutCubic);
        if st.dyn_sub_step != sub {
            continue;
        }
        let sc_c = c.scaled(area.center(), sc).faded(op);
        match sub {
            0 => sub0(d, &sc_c, st, area, icon16),
            1 => sub1(d, ui, &sc_c, st, area, icon14),
            _ => sub2(d, ui, &sc_c, st, area, icon14),
        }
    }
    out
}

fn sub0(d: &mut D, c: &Canvas, st: &TutorialState, a: Rect, icon: &egui::TextureHandle) {
    let th = d.th;
    let mut y = a.top();
    let del = rect_xywh(a.right() - 34.0, y, 34.0, 34.0);
    let tr = rect_xywh(del.left() - 10.0 - 130.0, y, 130.0, 34.0);
    let fr = rect_xywh(tr.left() - 10.0 - 160.0, y, 160.0, 34.0);
    c.text(Pos2::new(a.left(), y + 17.0), Align2::LEFT_CENTER, "Dynamic Snippet", fb(16.0), th.text_primary);
    d.folder_chip(c, fr, 11.0, 14.0);
    trigger_box(d, c, tr, "dynTriggerRect0", &st.dyn_field_trigger_text, "dynTrigger", icon, 16.0, 12.0, "dynTrigger");
    c.rect(del, 10.0, th.danger_light);
    c.icon(del.center(), "delete", 18.0, th.danger);
    y += 34.0 + 12.0;

    let cy = y + 16.0;
    let ins = c.text(Pos2::new(a.left(), cy), Align2::LEFT_CENTER, "INSERT", fb(9.0), th.text_secondary);
    let db = rect_xywh(ins.right() + 8.0, cy - 13.0, 58.0, 26.0);
    let tb = rect_xywh(db.right() + 8.0, cy - 13.0, 58.0, 26.0);
    for (r, res, icon_n, label, key, spot) in [(db, st.dyn_date_resolved, "calendar_today", "DATE", "dynDateBtn", "date"), (tb, st.dyn_time_resolved, "schedule", "TIME", "dynTimeBtn", "time")] {
        let fg = if res { d.success_green } else { th.text_secondary };
        d.token_box(c, r, 6.0, icon_n, 14.0, label, 8.0, fg, d.ac(key, if res { th.primary_light } else { th.hover_bg }, 200.0), if res { d.success_green } else { Color32::TRANSPARENT });
        d.glow(c, r, 8.0, d.spot(spot));
        d.rec(key, r);
    }
    let mut x = tb.right() + 8.0;
    x += d.token(c, x, cy, "input", 16.0, "FIELD", 9.0, th.primary) + 8.0;
    d.token(c, x, cy, "text_fields", 16.0, "CURSOR", 9.0, th.primary);
    let both = st.dyn_date_resolved && st.dyn_time_resolved;
    let wb = rect_xywh(a.right() - 130.0, y, 130.0, 32.0);
    let pc = d.pop("watchBtn", wb);
    pc.rect(wb, 8.0, d.ac("watchBtn", if both { d.success_green } else { th.primary_hover }, 200.0));
    pc.text(wb.center(), Align2::CENTER_CENTER, if both { "DONE!" } else { "TRY DATE & TIME" }, fb(9.0), Color32::WHITE);
    y += 32.0 + 12.0;

    let er = Rect::from_min_max(Pos2::new(a.left(), y), Pos2::new(a.right(), (a.bottom() - 80.0 - 12.0).max(y)));
    c.rect(er, 16.0, th.surface);
    d.glow(c, er, 16.0, d.spot("date") || d.spot("time") || d.spot("dynEditor"));
    let ec = c.clipped(er);
    let (t, colr) = if st.dyn_editor_text.is_empty() { ("Type your message here…", th.text_secondary) } else { (st.dyn_editor_text.as_str(), th.text_primary) };
    let r = ec.text_wrap(er.min + Vec2::splat(16.0), er.width() - 32.0, t, f(14.0), colr);
    d.caret(&ec, er.left() + 16.0 + r.width().min(er.width() - 32.0) + 2.0, r.center().y, d.typing("dynEditor"));
    d.rec("dynEditorRect", er);

    let rb = rect_xywh(a.left(), a.bottom() - 80.0, a.width(), 80.0);
    c.rect_fs(rb, 12.0, th.surface, 1.0, d.ac("dynres-bc", if both { d.success_green } else { Color32::TRANSPARENT }, 300.0));
    d.glow(c, rb, 12.0, d.spot("dynExpand"));
    let ri = rb.shrink(10.0);
    if both {
        c.icon_left(ri.left(), ri.top() + 8.0, "check_circle", 14.0, d.success_green);
    }
    let iw = crate::widgets::icon_text_size(c, "check_circle", 14.0).x;
    let msg = if both { "Done! Every time you type .mtg, the date & time are always up to date." } else { "Type .mtg to see the auto-fill words expand." };
    c.text_elide(Pos2::new(ri.left() + iw + 6.0, ri.top() + 8.0), ri.width() - iw - 6.0, msg, fb(10.0), d.ac("dynres-msg", if both { d.success_text } else { th.text_primary }, 300.0));
    let kb = rect_xywh(ri.left(), ri.top() + 16.0 + 6.0, ri.width(), 38.0);
    c.rect_fs(kb, 9.0, th.hover_bg, 1.0, d.ac("dynres-kb", if both { d.success_green } else { Color32::TRANSPARENT }, 300.0));
    let ki = kb.shrink(10.0);
    let k_ic = c.icon_left(ki.left(), ki.center().y, "keyboard", 15.0, th.text_secondary);
    let sr = c.text(Pos2::new(ki.right(), ki.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("send"), icon_font(15.0), if both { d.success_green } else { th.primary });
    let result = format!("Meeting on {} at {}", if st.dyn_date_resolved { "1 August 2026" } else { "{date}" }, if st.dyn_time_resolved { "02:30 PM" } else { "{time}" });
    let shown = if st.dyn_result_expanded { result } else { st.dyn_result_typed.clone() };
    let bold = both || st.dyn_result_expanded;
    let tr = c.text_elide(Pos2::new(k_ic.right() + 6.0, ki.center().y), sr.left() - 6.0 - k_ic.right() - 6.0, &shown, if bold { fb(12.0) } else { f(12.0) }, d.ac("dynres-t", if both { d.success_text } else { th.text_primary }, 300.0));
    d.caret(c, tr.right() + 1.0, ki.center().y, d.typing("dynExpand"));
    d.rec("dynResultBox", rb);
}

/// Row 1 + Row 2 of sub-steps 1 & 2.
#[allow(clippy::too_many_arguments)]
fn compact_header(d: &mut D, c: &Canvas, y: f32, a: Rect, title: &str, trig_name: &'static str, trig_text: &str, trig_key: &str, icon: &egui::TextureHandle) {
    let th = d.th;
    let del = rect_xywh(a.right() - 28.0, y, 28.0, 28.0);
    let tr = rect_xywh(del.left() - 10.0 - 118.0, y, 118.0, 28.0);
    let fr = rect_xywh(tr.left() - 10.0 - 128.0, y, 128.0, 28.0);
    c.text(Pos2::new(a.left(), y + 14.0), Align2::LEFT_CENTER, title, fb(15.0), th.text_primary);
    d.folder_chip(c, fr, 10.0, 13.0);
    trigger_box(d, c, tr, trig_name, trig_text, trig_key, icon, 14.0, 11.0, trig_key);
    c.rect(del, 10.0, th.danger_light);
    c.icon(del.center(), "delete", 16.0, th.danger);
}

fn scroll_area(ui: &mut Ui, id: &str, a: Rect, content_h: f32) -> f32 {
    let key = Id::new(("tut-scroll", id));
    let mut s = ui.ctx().data(|d| d.get_temp::<f32>(key)).unwrap_or(0.0);
    crate::pages::settings_page::flick(ui, a, &mut s, content_h);
    ui.ctx().data_mut(|d| d.insert_temp(key, s));
    s
}

fn sub1(d: &mut D, ui: &mut Ui, c0: &Canvas, st: &TutorialState, a: Rect, icon: &egui::TextureHandle) {
    let th = d.th;
    let scroll = scroll_area(ui, "fill", a, 28.0 + 26.0 + 52.0 + 40.0 + 18.0);
    let c = &c0.clipped(a);
    let mut y = a.top() - scroll;
    compact_header(d, c, y, a, "Fill-in Snippet", "dynTriggerRect", &st.dyn_field_trigger_text, "dynTrigger", icon);
    y += 28.0 + 6.0;
    let cy = y + 13.0;
    let ins = c.text(Pos2::new(a.left(), cy), Align2::LEFT_CENTER, "INSERT", fb(8.0), th.text_secondary);
    let r1 = rect_xywh(ins.right() + 8.0, cy - 11.0, 50.0, 22.0);
    d.token_box(c, r1, 6.0, "calendar_today", 13.0, "DATE", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let r2 = rect_xywh(r1.right() + 8.0, cy - 11.0, 50.0, 22.0);
    d.token_box(c, r2, 6.0, "schedule", 13.0, "TIME", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let r3 = rect_xywh(r2.right() + 8.0, cy - 11.0, 68.0, 22.0);
    let fi = st.dyn_field_inserted;
    d.token_box(c, r3, 8.0, "input", 13.0, "FIELD", 8.0, if fi { th.primary } else { th.text_secondary }, d.ac("fbtn-bg", if fi { th.primary_light } else { th.hover_bg }, 200.0), d.ac("fbtn-bc", if fi { th.primary } else { Color32::TRANSPARENT }, 200.0));
    d.glow(c, r3, 8.0, d.spot("dynFieldBtn"));
    d.rec("dynFieldBtn", r3);
    let r4 = rect_xywh(r3.right() + 8.0, cy - 11.0, 74.0, 22.0);
    d.token_box(c, r4, 8.0, "text_fields", 13.0, "CURSOR", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let tb = rect_xywh(a.right() - 132.0, y, 132.0, 26.0);
    c.rect(tb, 8.0, if st.dyn_field_resolved { d.success_green } else { th.primary_hover });
    c.text(tb.center(), Align2::CENTER_CENTER, if st.dyn_field_resolved { "DONE!" } else { "TRY FILL-IN" }, fb(9.0), Color32::WHITE);
    y += 26.0 + 6.0;

    // Editor strip
    let er = rect_xywh(a.left(), y, a.width(), 52.0);
    c.rect_fs(er, 12.0, th.surface, 1.0, d.ac("fed-bc", if st.dyn_field_resolved { d.success_green } else if !st.dyn_field_editor_text.is_empty() { th.primary } else { th.hover_bg }, 200.0));
    d.glow(c, er, 12.0, d.spot("dynFieldEditor"));
    let ei = er.shrink(10.0);
    let di = c.icon_left(ei.left(), ei.center().y, "description", 14.0, th.primary);
    let check_w = if st.dyn_field_resolved { 24.0 } else { 0.0 };
    let tx = di.right() + 8.0;
    let lbl = c.text_spaced(Pos2::new(tx, ei.top() + 2.0), Align2::LEFT_TOP, "Your snippet", fb(8.0), th.text_secondary, 1.0);
    let (t, colr, bold) = if st.dyn_field_editor_text.is_empty() {
        ("Type the expansion message here…", th.text_secondary, false)
    } else {
        (st.dyn_field_editor_text.as_str(), if st.dyn_field_resolved { d.success_text } else { th.text_primary }, st.dyn_field_resolved)
    };
    let trr = c.text_elide(Pos2::new(tx, lbl.bottom() + 2.0 + 8.0), ei.right() - check_w - tx, t, if bold { fb(12.0) } else { f(12.0) }, colr);
    d.caret(c, trr.right() + 2.0, trr.center().y, d.typing("dynFieldEditor"));
    if st.dyn_field_resolved {
        c.icon(Pos2::new(ei.right() - 8.0, ei.center().y), "check_circle", 16.0, d.success_green);
    }
    d.rec("dynFieldEditorRect", er);
    y += 52.0 + 6.0;

    // Any-other-app row
    let ar = rect_xywh(a.left(), y, a.width(), 40.0);
    c.rect_fs(ar, 14.0, th.background, 1.0, th.hover_bg);
    let ic = Pos2::new(ar.left() + 10.0 + 11.0, ar.center().y);
    c.circle(ic, 11.0, th.primary);
    c.icon(ic, "smartphone", 12.0, Color32::WHITE);
    let send = Pos2::new(ar.right() - 10.0 - 14.0, ar.center().y);
    let inp = Rect::from_min_max(Pos2::new(ic.x + 11.0 + 8.0, ar.center().y - 14.0), Pos2::new(send.x - 14.0 - 8.0, ar.center().y + 14.0));
    c.rect_fs(inp, 14.0, th.hover_bg, 1.0, d.ac("fapp-bc", if st.dyn_field_app_expanded { d.success_green } else if !st.dyn_field_app_text.is_empty() { th.primary } else { Color32::TRANSPARENT }, 200.0));
    d.glow(c, inp, 14.0, d.spot("dynFieldApp"));
    if !st.dyn_field_app_text.is_empty() && !st.dyn_field_app_expanded {
        let tr = c.text_elide(Pos2::new(inp.left() + 10.0, inp.center().y), inp.width() - 20.0, &st.dyn_field_app_text, mono_b(12.0), th.primary);
        d.caret(c, tr.right() + 1.0, inp.center().y, d.typing("dynFieldApp"));
    } else {
        let t = if st.dyn_field_app_expanded { "The price of the product is $49" } else { "Type .price…" };
        c.text(Pos2::new(inp.left() + 10.0, inp.center().y), Align2::LEFT_CENTER, t, fb(11.0), if st.dyn_field_app_expanded { d.success_text } else { th.text_secondary });
    }
    d.rec("dynFieldAppRect", inp);
    c.circle(send, 14.0, d.ac("fapp-send", if st.dyn_field_app_expanded { d.success_green } else { th.primary }, 200.0));
    c.icon(send, "send", 15.0, Color32::WHITE);
}

fn sub2(d: &mut D, ui: &mut Ui, c0: &Canvas, st: &TutorialState, a: Rect, icon: &egui::TextureHandle) {
    let th = d.th;
    let ctx = d.ctx;
    let scroll = scroll_area(ui, "cursor", a, 28.0 + 26.0 + 52.0 + 140.0 + 18.0);
    let c = &c0.clipped(a);
    let mut y = a.top() - scroll;
    compact_header(d, c, y, a, "Cursor Snippet", "dynCursorTriggerRect", &st.dyn_cursor_trigger_text, "dynCursorTrigger", icon);
    y += 28.0 + 6.0;
    let cy = y + 13.0;
    let ins = c.text(Pos2::new(a.left(), cy), Align2::LEFT_CENTER, "INSERT", fb(8.0), th.text_secondary);
    let r1 = rect_xywh(ins.right() + 8.0, cy - 11.0, 50.0, 22.0);
    d.token_box(c, r1, 6.0, "calendar_today", 13.0, "DATE", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let r2 = rect_xywh(r1.right() + 8.0, cy - 11.0, 50.0, 22.0);
    d.token_box(c, r2, 6.0, "schedule", 13.0, "TIME", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let r3 = rect_xywh(r2.right() + 8.0, cy - 11.0, 68.0, 22.0);
    d.token_box(c, r3, 8.0, "input", 13.0, "FIELD", 8.0, th.text_secondary, th.hover_bg, Color32::TRANSPARENT);
    let r4 = rect_xywh(r3.right() + 8.0, cy - 11.0, 78.0, 22.0);
    let ci = st.dyn_cursor_inserted;
    d.token_box(c, r4, 8.0, "text_fields", 13.0, "CURSOR", 8.0, if ci { th.primary } else { th.text_secondary }, d.ac("cbtn-bg", if ci { th.primary_light } else { th.hover_bg }, 200.0), d.ac("cbtn-bc", if ci { th.primary } else { Color32::TRANSPARENT }, 200.0));
    d.glow(c, r4, 8.0, d.spot("dynCursorBtn"));
    d.rec("dynCursorBtn", r4);
    let tb = rect_xywh(a.right() - 132.0, y, 132.0, 26.0);
    c.rect(tb, 8.0, d.ac("cur-try", if st.dyn_cursor_placed { d.success_green } else { th.primary_hover }, 200.0));
    c.text(tb.center(), Align2::CENTER_CENTER, if st.dyn_cursor_placed { "DONE!" } else { "TRY CURSOR" }, fb(9.0), Color32::WHITE);
    y += 26.0 + 6.0;

    // Editor strip with highlighted {cursor}
    let er = rect_xywh(a.left(), y, a.width(), 52.0);
    let pc = d.pop("dynCursorEditorRect", er).clipped(a);
    pc.rect_fs(er, 12.0, th.surface, 1.0, d.ac("ced-bc", if st.dyn_cursor_placed { d.success_green } else if !st.dyn_cursor_editor_text.is_empty() { th.primary } else { th.hover_bg }, 200.0));
    d.glow(&pc, er, 12.0, d.spot("dynCursorEditor") || d.spot("dynCursor"));
    let ei = er.shrink(10.0);
    let ti = pc.icon_left(ei.left(), ei.center().y, "text_fields", 14.0, th.primary);
    let tx = ti.right() + 8.0;
    let lbl = pc.text_spaced(Pos2::new(tx, ei.top() + 2.0), Align2::LEFT_TOP, "Your snippet", fb(8.0), th.text_secondary, 1.0);
    let line_cy = lbl.bottom() + 2.0 + 8.0;
    let t = &st.dyn_cursor_editor_text;
    let mut job = LayoutJob::default();
    if t.is_empty() {
        job.append("Type your line here…", 0.0, TextFormat::simple(f(12.0), th.text_secondary));
    } else if let Some(i) = t.find("{cursor}") {
        job.append(&t[..i], 0.0, TextFormat::simple(f(12.0), th.text_primary));
        job.append("{cursor}", 0.0, TextFormat { font_id: fb(12.0), color: theme::hex(0x1a1a1a), background: theme::hex(0xffe08a), ..Default::default() });
        job.append(&t[i + 8..], 0.0, TextFormat::simple(f(12.0), th.text_primary));
    } else {
        job.append(t, 0.0, TextFormat::simple(f(12.0), th.text_primary));
    }
    let g = pc.p.layout_job(job);
    let gs = g.size();
    let gpos = Pos2::new(tx, line_cy - gs.y / 2.0);
    pc.galley(gpos, g, th.text_primary);
    d.caret(&pc, gpos.x + gs.x + 2.0, line_cy, d.typing("dynCursorEditor"));
    if st.dyn_cursor_placed && !d.typing("dynCursorEditor") {
        let op = anim::ping_pong(ctx, 1.0, 0.0, 400.0, Ease::Linear);
        pc.faded(op).rect(rect_xywh(gpos.x + gs.x + 2.0, line_cy - 7.5, 2.0, 15.0), 1.0, th.primary);
    }
    if st.dyn_cursor_placed {
        pc.icon(Pos2::new(ei.right() - 8.0, ei.center().y), "check_circle", 16.0, d.success_green);
    }
    d.rec("dynCursorEditorRect", er);
    y += 52.0 + 6.0;

    // Any-other-app window
    let wr = rect_xywh(a.left(), y, a.width(), 140.0);
    c.rect_fs(wr, 14.0, th.background, 1.0, th.hover_bg);
    let wc = c.clipped(wr.intersect(a));
    let hr = rect_xywh(wr.left(), wr.top(), wr.width(), 28.0);
    wc.rect(hr, 0.0, th.surface);
    let ic = Pos2::new(hr.left() + 12.0 + 12.0, hr.center().y);
    wc.circle(ic, 12.0, th.primary);
    wc.icon(ic, "smartphone", 13.0, Color32::WHITE);
    wc.text(Pos2::new(ic.x + 12.0 + 5.0, hr.center().y), Align2::LEFT_CENTER, "Any other app", fb(12.0), th.text_primary);
    wc.text(Pos2::new(hr.right() - 12.0, hr.center().y), Align2::RIGHT_CENTER, &crate::fonts::icon("more_vert"), icon_font(15.0), th.text_secondary);
    let body = Rect::from_min_max(Pos2::new(wr.left(), hr.bottom()), Pos2::new(wr.right(), wr.bottom() - 34.0));
    if !st.dyn_cursor_app_expanded {
        wc.text(body.center(), Align2::CENTER_CENTER, "Type .cursor here and the cursor lands on your line", f(11.0), th.text_secondary);
    }
    let bh = anim::anim(ctx, Id::new("tut-cbub-h"), if st.dyn_cursor_app_expanded { 66.0 } else { 0.0 }, 240.0, Ease::Linear);
    let bop = anim::anim(ctx, Id::new("tut-cbub-op"), if st.dyn_cursor_app_expanded { 1.0 } else { 0.0 }, 240.0, Ease::OutCubic);
    let bsc = anim::anim(ctx, Id::new("tut-cbub-sc"), if st.dyn_cursor_app_expanded { 1.0 } else { 0.92 }, 240.0, Ease::OutBack);
    if bh > 0.5 && bop > 0.0 {
        let bw = (body.width() - 24.0).min(360.0);
        let br = rect_xywh(body.right() - 12.0 - bw, body.bottom() - 10.0 - bh, bw, bh);
        let bc = wc.scaled(br.center(), bsc).faded(bop);
        bc.rect(br, 12.0, th.primary);
        let bcc = bc.clipped(br);
        let bi = br.shrink(10.0);
        let aa = bcc.icon_left(bi.left(), bi.top() + 7.0, "auto_awesome", 12.0, Color32::WHITE);
        bcc.text(Pos2::new(aa.right() + 5.0, bi.top() + 7.0), Align2::LEFT_CENTER, "Expanded — cursor ready", fb(8.0), Color32::WHITE);
        let ly = bi.top() + 14.0 + 4.0 + 9.0;
        let pr = bcc.text(Pos2::new(bi.left(), ly), Align2::LEFT_CENTER, &st.dyn_cursor_app_prefix, fb(12.0), Color32::WHITE);
        d.caret(&bcc, pr.right() + 1.0, ly, d.typing("dynCursorAppPrefix"));
        let mut sx = pr.right() + 3.0;
        if st.dyn_cursor_app_expanded && !d.typing("dynCursorAppPrefix") {
            let op = anim::ping_pong(ctx, 1.0, 0.0, 400.0, Ease::Linear);
            bcc.faded(op).rect(rect_xywh(sx, ly - 7.5, 2.0, 15.0), 1.0, Color32::WHITE);
            sx += 2.0 + 3.0;
        }
        let srr = bcc.text_wrap(Pos2::new(sx, ly - 8.0), bi.right() - sx, &st.dyn_cursor_app_suffix, fb(12.0), Color32::WHITE);
        d.caret(&bcc, srr.right() + 1.0, ly, d.typing("dynCursorAppSuffix"));
    }
    // input row
    let ir = rect_xywh(wr.left(), wr.bottom() - 34.0, wr.width(), 34.0);
    wc.rect(ir, 0.0, th.surface);
    let send = Pos2::new(ir.right() - 12.0 - 14.0, ir.center().y);
    let inp = Rect::from_min_max(Pos2::new(ir.left() + 12.0, ir.center().y - 14.0), Pos2::new(send.x - 14.0 - 8.0, ir.center().y + 14.0));
    wc.rect_fs(inp, 14.0, th.hover_bg, 1.0, d.ac("capp-bc", if st.dyn_cursor_app_expanded { d.success_green } else if !st.dyn_cursor_app_text.is_empty() { th.primary } else { Color32::TRANSPARENT }, 200.0));
    d.glow(&wc, inp, 14.0, d.spot("dynCursorApp"));
    if !st.dyn_cursor_app_text.is_empty() && !st.dyn_cursor_app_expanded {
        let tr = wc.text(Pos2::new(inp.left() + 10.0, inp.center().y), Align2::LEFT_CENTER, &st.dyn_cursor_app_text, mono_b(12.0), th.primary);
        d.caret(&wc, tr.right() + 1.0, inp.center().y, d.typing("dynCursorApp"));
    } else {
        let t = if st.dyn_cursor_app_expanded { "Saved with cursor ready" } else { "Type .cursor…" };
        wc.text(Pos2::new(inp.left() + 10.0, inp.center().y), Align2::LEFT_CENTER, t, fb(11.0), if st.dyn_cursor_app_expanded { d.success_text } else { th.text_secondary });
    }
    d.rec("dynCursorAppRect", ir);
    wc.circle(send, 14.0, d.ac("capp-send", if st.dyn_cursor_app_expanded { d.success_green } else { th.primary }, 200.0));
    wc.icon(send, "send", 15.0, Color32::WHITE);
}
