//! Port of Components/LibraryPanel.qml

use egui::{Align2, Color32, Id, LayerId, Order, Pos2, Rect, Sense, Ui, UiBuilder, Vec2};
use std::collections::HashMap;

use crate::anim::{self, Ease};
use crate::app::{Action, App};
use crate::draw::{rect_xywh, Canvas};
use crate::fonts::{f, fb, mono_b};
use crate::store::SnippetStore;
use crate::theme::{self, Theme};
use crate::widgets::{self, TipPos};

const PREVIEW: &str = "__drag_preview__";

#[derive(Clone, Debug)]
pub struct SnRow {
    pub name: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct FolderRow {
    pub name: String,
    pub expanded: bool,
    pub snippets: Vec<SnRow>,
}

impl FolderRow {
    fn real_count(&self) -> usize {
        self.snippets.iter().filter(|s| s.name != PREVIEW).count()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum PressKind {
    Folder(String),
    Snippet(String, String),
}

#[derive(Clone, Debug, PartialEq)]
enum DropTarget {
    Header(String),
    Row(String, String),
}

struct CtxMenu {
    folder: String,
    snippet: String,
    view: u8,
    pos: Pos2,
    opened: bool,
}

struct Rename {
    original: String,
    input: String,
    focus_pending: bool,
}

/// y-displacement used for ListView `move`/`displaced` transitions.
#[derive(Clone, Copy)]
struct Disp {
    from: f32,
    start: f64,
    dur: f32,
}

#[derive(Default)]
pub struct LibraryState {
    pub folders: Vec<FolderRow>,
    pub selected_folder: String,
    pub selected_abbrev: String,
    pub search: String,
    search_raw_text: String,
    expanded_count: usize,
    scroll: f32,
    press: Option<(PressKind, Pos2)>,
    drag_folder: String,
    drag_snippet: String,
    drag_snippet_folder: String,
    drag_pointer: Pos2,
    ghost_shown_at: f64,
    drag_preview_folder: String,
    last_target: Option<DropTarget>,
    ctx_menu: Option<CtxMenu>,
    rename: Option<Rename>,
    root: Option<Rect>,
    heights: HashMap<String, f32>,
    disp: HashMap<String, Disp>,
}

impl LibraryState {
    pub fn rebuild_all(&mut self, store: &SnippetStore) {
        let state: HashMap<String, bool> = self.folders.iter().map(|r| (r.name.clone(), r.expanded)).collect();
        self.folders = store
            .folders()
            .iter()
            .map(|f| FolderRow {
                name: f.name.clone(),
                expanded: state.get(&f.name).copied().unwrap_or(false),
                snippets: f.snippets.iter().map(|s| SnRow { name: s.abbreviation.clone(), content: s.content.clone() }).collect(),
            })
            .collect();
        self.drag_preview_folder.clear();
    }

    pub fn refresh_folder(&mut self, store: &SnippetStore, fname: &str) {
        let sns: Vec<SnRow> = store
            .snippets_for_folder(fname)
            .into_iter()
            .map(|s| SnRow { name: s.abbreviation, content: s.content })
            .collect();
        if let Some(r) = self.folders.iter_mut().find(|r| r.name == fname) {
            r.snippets = sns;
        }
        if self.drag_preview_folder == fname {
            self.drag_preview_folder.clear();
        }
    }

    pub fn set_expanded(&mut self, fname: &str, v: bool) {
        if let Some(r) = self.folders.iter_mut().find(|r| r.name == fname) {
            r.expanded = v;
        }
    }

    fn all_expanded(&self) -> bool {
        !self.folders.is_empty() && self.expanded_count == self.folders.len()
    }

    fn update_expanded_count(&mut self) {
        self.expanded_count = self.folders.iter().filter(|r| r.expanded).count();
    }

    pub fn toggle_all(&mut self) {
        let target = !self.all_expanded();
        for r in self.folders.iter_mut() {
            r.expanded = target;
        }
        self.update_expanded_count();
    }

    fn clear_drag_preview(&mut self) {
        if self.drag_preview_folder.is_empty() {
            return;
        }
        let pf = self.drag_preview_folder.clone();
        if let Some(r) = self.folders.iter_mut().find(|r| r.name == pf) {
            if let Some(i) = r.snippets.iter().rposition(|s| s.name == PREVIEW) {
                r.snippets.remove(i);
            }
        }
        self.drag_preview_folder.clear();
    }

    fn folder_visible(&self, r: &FolderRow) -> bool {
        let q = &self.search;
        if q.is_empty() || r.name.to_lowercase().contains(q.as_str()) {
            return true;
        }
        r.snippets.iter().any(|s| s.name.to_lowercase().contains(q.as_str()) || s.content.to_lowercase().contains(q.as_str()))
    }

    fn snippet_visible(&self, s: &SnRow) -> bool {
        let q = &self.search;
        s.name == PREVIEW || q.is_empty() || s.name.to_lowercase().contains(q.as_str()) || s.content.to_lowercase().contains(q.as_str())
    }

    fn sn_content_height(&self, r: &FolderRow) -> f32 {
        let n = r.snippets.len();
        if n == 0 {
            return 0.0;
        }
        let vis: f32 = r.snippets.iter().map(|s| if self.snippet_visible(s) { 34.0 } else { 0.0 }).sum();
        vis + 4.0 * (n as f32 - 1.0)
    }

    fn target_height(&self, r: &FolderRow) -> f32 {
        if !self.folder_visible(r) {
            return 0.0;
        }
        40.0 + if r.expanded {
            self.sn_content_height(r) + if r.real_count() == 0 { 26.0 } else { 0.0 } + 4.0
        } else {
            0.0
        }
    }

    fn disp_offset(&mut self, now: f64, key: &str) -> f32 {
        if let Some(d) = self.disp.get(key).copied() {
            let t = ((now - d.start) as f32 / d.dur).clamp(0.0, 1.0);
            if t >= 1.0 {
                self.disp.remove(key);
                return 0.0;
            }
            return d.from * (1.0 - anim::ease(Ease::OutCubic, t));
        }
        0.0
    }

    fn add_disp(&mut self, now: f64, key: String, delta: f32, dur: f32) {
        if delta.abs() < 0.5 {
            return;
        }
        let cur = self.disp_offset(now, &key);
        self.disp.insert(key, Disp { from: cur + delta, start: now, dur });
    }

    fn folder_layout_ys(&self) -> Vec<f32> {
        let mut y = 0.0;
        self.folders
            .iter()
            .map(|r| {
                let cur = y;
                y += self.heights.get(&r.name).copied().unwrap_or(40.0) + 6.0;
                cur
            })
            .collect()
    }

    fn move_folder(&mut self, now: f64, from: usize, to: usize) {
        let before: HashMap<String, f32> =
            self.folders.iter().map(|r| r.name.clone()).zip(self.folder_layout_ys()).collect();
        let row = self.folders.remove(from);
        self.folders.insert(to, row);
        let after: Vec<(String, f32)> = self.folders.iter().map(|r| r.name.clone()).zip(self.folder_layout_ys()).collect();
        for (name, y) in after {
            let d = before.get(&name).copied().unwrap_or(y) - y;
            self.add_disp(now, format!("f:{name}"), d, 0.25);
        }
    }

    fn snippet_ys(&self, fi: usize) -> Vec<f32> {
        let r = &self.folders[fi];
        let mut y = 0.0;
        r.snippets
            .iter()
            .map(|s| {
                let cur = y;
                y += if self.snippet_visible(s) { 34.0 } else { 0.0 } + 4.0;
                cur
            })
            .collect()
    }

    fn move_snippet_row(&mut self, now: f64, fi: usize, from: usize, to: usize) {
        let fname = self.folders[fi].name.clone();
        let before: HashMap<String, f32> =
            self.folders[fi].snippets.iter().map(|s| s.name.clone()).zip(self.snippet_ys(fi)).collect();
        let row = self.folders[fi].snippets.remove(from);
        self.folders[fi].snippets.insert(to, row);
        let after: Vec<(String, f32)> =
            self.folders[fi].snippets.iter().map(|s| s.name.clone()).zip(self.snippet_ys(fi)).collect();
        for (name, y) in after {
            let d = before.get(&name).copied().unwrap_or(y) - y;
            self.add_disp(now, format!("s:{fname}:{name}"), d, 0.2);
        }
    }

    /// DropArea.onEntered for a folder header / snippet row while dragging a snippet.
    fn on_entered(&mut self, now: f64, t: &DropTarget) {
        let sn = self.drag_snippet.clone();
        let from = self.drag_snippet_folder.clone();
        if sn.is_empty() {
            return;
        }
        match t {
            DropTarget::Header(f) => {
                if &from == f {
                    return;
                }
                if !self.drag_preview_folder.is_empty() && &self.drag_preview_folder != f {
                    self.clear_drag_preview();
                }
                self.set_expanded(f, true);
                if &self.drag_preview_folder != f {
                    if let Some(r) = self.folders.iter_mut().find(|r| &r.name == f) {
                        r.snippets.push(SnRow { name: PREVIEW.into(), content: String::new() });
                        self.drag_preview_folder = f.clone();
                    }
                }
            }
            DropTarget::Row(f, name) => {
                if name == &sn {
                    return;
                }
                let Some(fi) = self.folders.iter().position(|r| &r.name == f) else { return };
                let Some(index) = self.folders[fi].snippets.iter().position(|s| &s.name == name) else { return };
                if &from == f {
                    if let Some(fr) = self.folders[fi].snippets.iter().position(|s| s.name == sn) {
                        if fr != index {
                            self.move_snippet_row(now, fi, fr, index);
                        }
                    }
                } else {
                    if !self.drag_preview_folder.is_empty() && &self.drag_preview_folder != f {
                        self.clear_drag_preview();
                    }
                    let Some(fi) = self.folders.iter().position(|r| &r.name == f) else { return };
                    self.folders[fi].expanded = true;
                    if &self.drag_preview_folder == f {
                        if let Some(pi) = self.folders[fi].snippets.iter().position(|s| s.name == PREVIEW) {
                            if pi != index {
                                self.move_snippet_row(now, fi, pi, index);
                            }
                        }
                    } else {
                        let idx = index.min(self.folders[fi].snippets.len());
                        self.folders[fi].snippets.insert(idx, SnRow { name: PREVIEW.into(), content: String::new() });
                        self.drag_preview_folder = f.clone();
                    }
                }
            }
        }
    }
}

fn round_btn(ui: &mut Ui, th: &Theme, c: &Canvas, id: Id, r: Rect, icon: &str, tip: &str) -> bool {
    let resp = ui.interact(r, id, Sense::CLICK);
    c.rect(r, 14.0, if resp.hovered() { th.hover_bg } else { th.surface });
    c.icon(r.center(), icon, 18.0, if resp.hovered() { th.primary } else { th.text_secondary });
    widgets::tooltip(ui, th, id.with("tip"), resp.hovered(), r, tip, 400.0, TipPos::Above, Vec2::ZERO);
    resp.clicked()
}

pub fn show(app: &mut App, ui: &mut Ui, root: Rect) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let lib = &mut app.library;
    let actions = &mut app.actions;
    lib.root = Some(root);
    let c = Canvas::new(ui.painter());

    // ── Header ──────────────────────────────────────────────────────────
    let header = rect_xywh(root.left(), root.top(), root.width(), 32.0);
    c.text_spaced(Pos2::new(header.left(), header.center().y), Align2::LEFT_CENTER, "LIBRARY", fb(10.0), th.text_secondary, 1.5);
    let add_sn = rect_xywh(header.right() - 28.0, header.center().y - 14.0, 28.0, 28.0);
    let add_f = add_sn.translate(Vec2::new(-32.0, 0.0));
    let toggle = add_f.translate(Vec2::new(-32.0, 0.0));
    let all = lib.all_expanded();
    if round_btn(ui, &th, &c, Id::new("lib-toggle"), toggle, if all { "unfold_less" } else { "unfold_more" }, if all { "Collapse All" } else { "Expand All" }) {
        lib.toggle_all();
    }
    if round_btn(ui, &th, &c, Id::new("lib-addf"), add_f, "create_new_folder", "New Folder") {
        actions.push(Action::AddFolder("New Folder".into()));
    }
    if round_btn(ui, &th, &c, Id::new("lib-addsn"), add_sn, "add_box", "New Snippet") {
        let mut target = lib.selected_folder.clone();
        if target.is_empty() {
            if let Some(f) = lib.folders.first() {
                target = f.name.clone();
            }
        }
        if let Some(r) = lib.folders.iter().find(|r| r.name == target) {
            let mut new_abbrev = ".new".to_string();
            let mut counter = 1;
            while r.snippets.iter().any(|s| s.name == new_abbrev) {
                new_abbrev = format!(".new{counter}");
                counter += 1;
            }
            actions.push(Action::AddSnippetAndSelect(target, new_abbrev));
        }
    }

    // ── Search bar ──────────────────────────────────────────────────────
    let search = rect_xywh(root.left(), header.bottom() + 12.0, root.width(), 32.0);
    c.rect(search, 8.0, th.surface);
    c.icon_left(search.left() + 10.0, search.center().y, "search", 16.0, th.text_secondary);
    let input_r = Rect::from_min_max(Pos2::new(search.left() + 10.0 + 16.0 + 6.0, search.top()), Pos2::new(search.right() - 10.0, search.bottom()));
    let sid = Id::new("lib-search");
    let mut search_text = lib.search_raw();
    let out = widgets::line_edit(ui, sid, input_r, &mut search_text, f(11.0), th.text_primary, Some(("Search snippets…", f(11.0), th.text_secondary, true)));
    if out.changed {
        lib.search = search_text.to_lowercase();
        lib.set_search_raw(search_text);
    }
    if ui.interact(search, sid.with("bg"), Sense::CLICK).clicked() {
        widgets::request_focus(ui, sid);
    }

    // ── List area ───────────────────────────────────────────────────────
    let list = Rect::from_min_max(Pos2::new(root.left(), search.bottom() + 16.0), root.max);

    if lib.folders.is_empty() {
        let cy = list.center().y;
        c.faded(0.4).icon(Pos2::new(list.center().x, cy - 28.0), "inbox", 40.0, th.text_secondary);
        c.text(Pos2::new(list.center().x, cy + 4.0), Align2::CENTER_CENTER, "No folders yet", fb(13.0), th.text_primary);
        c.text(Pos2::new(list.center().x, cy + 24.0), Align2::CENTER_CENTER, "Click + to create your first snippet", f(11.0), th.text_secondary);
    }

    // Animated heights
    let mut heights = vec![];
    for r in lib.folders.clone().iter() {
        let target = lib.target_height(r);
        let h = anim::anim(&ctx, Id::new(("fh", &r.name)), target, 200.0, Ease::OutCubic);
        lib.heights.insert(r.name.clone(), h);
        heights.push(h);
    }
    let content_h: f32 = heights.iter().sum::<f32>() + 6.0 * (heights.len().saturating_sub(1)) as f32;
    let dragging = !lib.drag_folder.is_empty() || !lib.drag_snippet.is_empty();
    let max_scroll = (content_h - list.height()).max(0.0);
    let list_resp = ui.interact(list, Id::new("lib-list"), Sense::hover());
    if !dragging && list_resp.contains_pointer() {
        let dy = ctx.input(|i| i.smooth_scroll_delta.y);
        lib.scroll -= dy;
    }
    lib.scroll = lib.scroll.clamp(0.0, max_scroll);

    let lc = c.clipped(list);
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_released = ctx.input(|i| i.pointer.primary_released());

    let mut header_rects: Vec<(String, Rect)> = vec![];
    let mut row_rects: Vec<(String, String, Rect)> = vec![];
    let mut layout_rows: Vec<(String, f32, f32)> = vec![]; // name, top, height (no displacement)

    let mut y = list.top() - lib.scroll;
    let folders_snapshot = lib.folders.clone();
    for (fi, r) in folders_snapshot.iter().enumerate() {
        let h = heights[fi];
        let off = lib.disp_offset(now, &format!("f:{}", r.name));
        let row_top = y + off;
        layout_rows.push((r.name.clone(), y, h));
        y += h + 6.0;
        if !lib.folder_visible(r) || h <= 0.01 {
            continue;
        }
        let fname = r.name.clone();
        let hr = rect_xywh(list.left(), row_top, list.width(), 40.0);
        let selected = lib.selected_folder == fname;

        // interaction (only when inside the visible list viewport)
        let in_view = hr.intersects(list);
        let resp = if in_view {
            Some(ui.interact(hr.intersect(list), Id::new(("fhdr", &fname)), Sense::CLICK))
        } else {
            None
        };
        let hovered = resp.as_ref().map(|r| r.hovered()).unwrap_or(false);
        header_rects.push((fname.clone(), hr));

        let is_drop_target = !lib.drag_snippet.is_empty()
            && lib.drag_snippet_folder != fname
            && pointer.map(|p| hr.contains(p) && list.contains(p)).unwrap_or(false);
        let bg = if is_drop_target {
            theme::darker(th.primary, 1.1)
        } else if selected {
            th.primary
        } else if hovered {
            th.hover_bg
        } else {
            th.surface
        };
        let hidden = lib.drag_folder == fname;
        if !hidden {
            lc.rect(hr, 12.0, bg);
            let icol = if selected {
                Color32::WHITE
            } else if hovered {
                if th.is_dark { Color32::WHITE } else { theme::hex(0x475569) }
            } else {
                th.text_secondary
            };
            let rot = anim::anim(&ctx, Id::new(("chev", &fname)), if r.expanded { 0.0 } else { -90.0 }, 180.0, Ease::Linear);
            let inner = hr.shrink(12.0);
            let isz = crate::widgets::icon_text_size(&lc, "expand_more", 16.0);
            lc.icon_rot(Pos2::new(inner.left() + isz.x / 2.0, inner.center().y), "expand_more", 16.0, icol, rot);
            let fx = inner.left() + isz.x + 10.0;
            lc.icon_left(fx, inner.center().y, "folder", 16.0, icol);
            let nx = fx + isz.x + 10.0;
            let badge = rect_xywh(inner.right() - 22.0, inner.center().y - 9.0, 22.0, 18.0);
            lc.text_elide(Pos2::new(nx, inner.center().y), badge.left() - 10.0 - nx, &fname, fb(11.0), if selected { Color32::WHITE } else { th.text_primary });
            lc.rect(badge, 9.0, if selected { Color32::from_rgba_unmultiplied(255, 255, 255, 64) } else { th.hover_bg });
            lc.text(badge.center(), Align2::CENTER_CENTER, &r.real_count().to_string(), fb(10.0), if selected { Color32::WHITE } else { th.text_secondary });
        }

        if let Some(resp) = &resp {
            if hovered && primary_pressed && !dragging {
                if let Some(p) = pointer {
                    lib.press = Some((PressKind::Folder(fname.clone()), p));
                }
            }
            if resp.secondary_clicked() {
                if let Some(p) = pointer {
                    lib.ctx_menu = Some(CtxMenu { folder: fname.clone(), snippet: String::new(), view: 0, pos: p, opened: true });
                }
            }
        }

        // Snippet list
        if r.expanded {
            let list_top = row_top + 40.0 + 4.0;
            if r.real_count() == 0 {
                lc.faded(0.7).text(
                    Pos2::new(hr.center().x, list_top + 13.0),
                    Align2::CENTER_CENTER,
                    "No snippets yet — right-click for options",
                    f(10.0),
                    th.text_secondary,
                );
            }
            let mut sy = 0.0;
            for s in r.snippets.iter() {
                let vis = lib.snippet_visible(s);
                let soff = lib.disp_offset(now, &format!("s:{}:{}", fname, s.name));
                let sr = rect_xywh(hr.left(), list_top + sy + soff, hr.width(), 34.0);
                sy += if vis { 34.0 } else { 0.0 } + 4.0;
                if !vis {
                    continue;
                }
                row_rects.push((fname.clone(), s.name.clone(), sr));
                let is_preview = s.name == PREVIEW;
                if is_preview {
                    continue;
                }
                let sresp = if sr.intersects(list) {
                    Some(ui.interact(sr.intersect(list), Id::new(("snrow", &fname, &s.name)), Sense::CLICK))
                } else {
                    None
                };
                let shover = sresp.as_ref().map(|r| r.hovered()).unwrap_or(false);
                let ssel = lib.selected_abbrev == s.name && lib.selected_folder == fname;
                let being_dragged = lib.drag_snippet == s.name && lib.drag_snippet_folder == fname;
                if !being_dragged {
                    lc.rect(sr, 10.0, if ssel || shover { th.hover_bg } else { th.surface });
                    let inner_l = sr.left() + 12.0;
                    let inner_r = sr.right() - 12.0;
                    let ng = crate::draw::layout_elide(&lc.p, &s.name, mono_b(12.0), th.text_primary, inner_r - inner_l);
                    let nw = ng.size().x;
                    lc.galley(Pos2::new(inner_l, sr.center().y - ng.size().y / 2.0), ng, th.text_primary);
                    if !s.content.is_empty() {
                        let ax = inner_l + nw + 8.0;
                        let ar = lc.faded(0.6).text(Pos2::new(ax, sr.center().y), Align2::LEFT_CENTER, "\u{2192}", f(10.0), th.primary);
                        let px = ar.right() + 8.0;
                        let first = s.content.split('\n').next().unwrap_or("");
                        if inner_r - px > 4.0 {
                            lc.text_elide(Pos2::new(px, sr.center().y), inner_r - px, first, f(10.0), th.text_primary);
                        }
                    }
                }
                if let Some(sresp) = &sresp {
                    if shover && primary_pressed && !dragging {
                        if let Some(p) = pointer {
                            lib.press = Some((PressKind::Snippet(fname.clone(), s.name.clone()), p));
                        }
                    }
                    if sresp.secondary_clicked() {
                        if let Some(p) = pointer {
                            lib.ctx_menu = Some(CtxMenu { folder: fname.clone(), snippet: s.name.clone(), view: 0, pos: p, opened: true });
                        }
                    }
                }
            }
        }
    }

    // ── Press / drag state machine ──────────────────────────────────────
    if let Some((kind, origin)) = lib.press.clone() {
        if primary_down && lib.drag_folder.is_empty() && lib.drag_snippet.is_empty() {
            if let Some(p) = pointer {
                if p.distance(origin) > 8.0 {
                    match &kind {
                        PressKind::Folder(n) => lib.drag_folder = n.clone(),
                        PressKind::Snippet(fo, n) => {
                            lib.drag_snippet = n.clone();
                            lib.drag_snippet_folder = fo.clone();
                            lib.last_target = None;
                        }
                    }
                    lib.ghost_shown_at = now;
                }
            }
        }
        if primary_released {
            if lib.drag_folder.is_empty() && lib.drag_snippet.is_empty() {
                // plain click
                match kind {
                    PressKind::Folder(n) => {
                        if let Some(r) = lib.folders.iter_mut().find(|r| r.name == n) {
                            r.expanded = !r.expanded;
                        }
                        lib.selected_folder = n;
                        lib.update_expanded_count();
                    }
                    PressKind::Snippet(fo, n) => {
                        lib.selected_folder = fo.clone();
                        lib.selected_abbrev = n.clone();
                        actions.push(Action::SelectSnippet(fo, n));
                    }
                }
            }
            lib.press = None;
        }
    }

    if let Some(p) = pointer {
        lib.drag_pointer = p;
    }

    // Folder drag: live reorder
    if !lib.drag_folder.is_empty() {
        let name = lib.drag_folder.clone();
        if let Some(p) = pointer {
            let mut target = None;
            for (i, (_, top, h)) in layout_rows.iter().enumerate() {
                if *h > 0.0 && p.y >= *top && p.y < top + h {
                    target = Some(i);
                    break;
                }
            }
            if target.is_none() {
                if let Some((_, top, h)) = layout_rows.last() {
                    if p.y >= top + h {
                        target = Some(layout_rows.len() - 1);
                    }
                }
                if let Some((_, top, _)) = layout_rows.first() {
                    if p.y < *top {
                        target = Some(0);
                    }
                }
            }
            if let (Some(t), Some(from)) = (target, lib.folders.iter().position(|r| r.name == name)) {
                if from != t {
                    lib.move_folder(now, from, t);
                }
            }
        }
        if primary_released || !primary_down {
            let names: Vec<String> = lib.folders.iter().map(|r| r.name.clone()).collect();
            actions.push(Action::ReorderFolders(names));
            lib.drag_folder.clear();
        }
        ctx.request_repaint();
    }

    // Snippet drag: DropArea enter / drop
    if !lib.drag_snippet.is_empty() {
        let target = pointer.filter(|p| list.contains(*p)).and_then(|p| {
            // later-drawn items are on top
            for (f, n, r) in row_rects.iter().rev() {
                if r.contains(p) {
                    return Some(DropTarget::Row(f.clone(), n.clone()));
                }
            }
            for (f, r) in header_rects.iter().rev() {
                if r.contains(p) {
                    return Some(DropTarget::Header(f.clone()));
                }
            }
            None
        });
        if target != lib.last_target {
            if let Some(t) = &target {
                lib.on_entered(now, t);
            }
            lib.last_target = target.clone();
        }
        if primary_released || !primary_down {
            let sn = lib.drag_snippet.clone();
            let from = lib.drag_snippet_folder.clone();
            let mut deferred_move = None;
            match &target {
                Some(DropTarget::Header(f)) => {
                    lib.clear_drag_preview();
                    if &from != f {
                        deferred_move = Some(f.clone());
                    }
                }
                Some(DropTarget::Row(f, _)) => {
                    if &from != f {
                        lib.clear_drag_preview();
                        deferred_move = Some(f.clone());
                    }
                }
                None => {}
            }
            lib.clear_drag_preview();
            if let Some(r) = lib.folders.iter().find(|r| r.name == from) {
                let names: Vec<String> = r.snippets.iter().map(|s| s.name.clone()).collect();
                actions.push(Action::ReorderSnippets(from.clone(), names));
            }
            if let Some(to) = deferred_move {
                actions.push(Action::MoveSnippet(sn, from, to));
            }
            lib.drag_snippet.clear();
            lib.drag_snippet_folder.clear();
            lib.last_target = None;
        }
        ctx.request_repaint();
    }

    // ── Ghosts (z 3000, clipped by the library container) ───────────────
    let gc = c.clipped(root.expand(16.0));
    let gscale = {
        let el = ((now - lib.ghost_shown_at) * 1000.0) as f32;
        if el < 100.0 {
            ctx.request_repaint();
            1.0 - 0.05 * (el / 100.0)
        } else {
            0.95
        }
    };
    if !lib.drag_folder.is_empty() {
        let gr = Rect::from_center_size(lib.drag_pointer, Vec2::new(240.0, 42.0));
        let g = gc.scaled(gr.center(), gscale).faded(0.95);
        g.rect(gr, 10.0, th.primary);
        let inner = gr.shrink(12.0);
        let ir = g.icon_left(inner.left(), inner.center().y, "folder", 16.0, Color32::WHITE);
        g.text_elide(Pos2::new(ir.right() + 10.0, inner.center().y), inner.right() - ir.right() - 10.0, &lib.drag_folder, fb(12.0), Color32::WHITE);
    }
    if !lib.drag_snippet.is_empty() {
        let gr = Rect::from_center_size(lib.drag_pointer, Vec2::new(220.0, 38.0));
        let g = gc.scaled(gr.center(), gscale).faded(0.95);
        g.rect_fs(gr, 10.0, th.surface, 1.5, th.primary);
        let inner = gr.shrink(12.0);
        let sr = g.text(Pos2::new(inner.right(), inner.center().y), Align2::RIGHT_CENTER, "SNIPPET", fb(9.0), th.text_secondary);
        g.text_elide(Pos2::new(inner.left(), inner.center().y), sr.left() - inner.left() - 4.0, &lib.drag_snippet, mono_b(12.0), th.text_primary);
    }
}

impl LibraryState {
    fn search_raw(&self) -> String {
        self.search_raw_text.clone()
    }
    fn set_search_raw(&mut self, s: String) {
        self.search_raw_text = s;
    }
}

/// Context menu + rename popup (Popups live on the window overlay).
pub fn show_overlays(app: &mut App, ui: &mut Ui) {
    let th = app.th;
    let ctx = ui.ctx().clone();
    let screen = ctx.content_rect();
    let folder_names: Vec<String> = app.library.folders.iter().map(|r| r.name.clone()).collect();
    let lib = &mut app.library;
    let actions = &mut app.actions;

    // ── Right-click context menu ────────────────────────────────────────
    let mut close_menu = false;
    let mut open_rename: Option<String> = None;
    if let Some(m) = &mut lib.ctx_menu {
        // rows: (icon, label, action)
        let rows: Vec<(String, String, String)> = if m.view == 1 {
            folder_names
                .iter()
                .filter(|f| **f != m.folder)
                .map(|f| ("folder".to_string(), f.clone(), format!("moveTo:{f}")))
                .collect()
        } else if !m.snippet.is_empty() {
            vec![
                ("content_copy".into(), "Duplicate".into(), "duplicate".into()),
                ("drive_file_move".into(), "Move to…".into(), "move".into()),
                ("delete_outline".into(), "Delete".into(), "delete".into()),
            ]
        } else {
            vec![
                ("drive_file_rename_outline".into(), "Rename Folder".into(), "rename".into()),
                ("delete_outline".into(), "Delete Folder".into(), "deleteFolder".into()),
            ]
        };
        let count = rows.len() + if m.view == 1 { 1 } else { 0 };
        let h = 10.0 + count as f32 * 36.0;
        let w = 210.0;
        let root = lib.root.unwrap_or(Rect::NOTHING);
        let x = (m.pos.x - root.left()).min(root.width() - w - 8.0).max(0.0) + root.left();
        let y = (m.pos.y - root.top()).min(root.height() - h - 8.0).max(0.0) + root.top();
        let mr = rect_xywh(x, y, w, h);
        let layer = LayerId::new(Order::Foreground, Id::new("lib-ctx"));
        let mut lu = Ui::new(ctx.clone(), Id::new("lib-ctx-ui"), UiBuilder::new().layer_id(layer).max_rect(screen));
        let c = Canvas::new(lu.painter());
        c.rect_fs(mr, 10.0, th.surface, 1.0, th.hover_bg);
        let _bg = lu.interact(mr, Id::new("lib-ctx-bg"), Sense::CLICK);
        let mut ry = mr.top() + 5.0;
        let mut trigger: Option<String> = None;
        if m.view == 1 {
            let rr = rect_xywh(mr.left() + 5.0, ry, w - 10.0, 34.0);
            let resp = lu.interact(rr, Id::new("lib-ctx-back"), Sense::CLICK);
            if resp.hovered() {
                c.rect(rr, 8.0, th.hover_bg);
            }
            let ir = c.icon_left(rr.left() + 10.0, rr.center().y, "arrow_back", 15.0, th.text_secondary);
            c.text(Pos2::new(ir.right() + 5.0, rr.center().y), Align2::LEFT_CENTER, "Back", fb(11.0), th.text_primary);
            if resp.clicked() {
                trigger = Some("back".into());
            }
            ry += 36.0;
        }
        for (k, (icon, label, act)) in rows.iter().enumerate() {
            let rr = rect_xywh(mr.left() + 5.0, ry, w - 10.0, 34.0);
            let resp = lu.interact(rr, Id::new(("lib-ctx-row", k, m.view)), Sense::CLICK);
            if resp.hovered() {
                c.rect(rr, 8.0, th.hover_bg);
            }
            let danger = act == "delete" || act == "deleteFolder";
            let ir = c.icon_left(rr.left() + 10.0, rr.center().y, icon, 15.0, if danger { th.danger } else { th.text_secondary });
            c.text_elide(Pos2::new(ir.right() + 5.0, rr.center().y), rr.right() - 10.0 - ir.right() - 5.0, label, fb(11.0), th.text_primary);
            if resp.clicked() {
                trigger = Some(act.clone());
            }
            ry += 36.0;
        }
        if let Some(t) = trigger {
            match t.as_str() {
                "back" => m.view = 0,
                "duplicate" => {
                    actions.push(Action::DuplicateSnippet(m.folder.clone(), m.snippet.clone()));
                    close_menu = true;
                }
                "move" => m.view = 1,
                "delete" => {
                    actions.push(Action::DeleteSnippet(m.folder.clone(), m.snippet.clone()));
                    close_menu = true;
                }
                "rename" => {
                    close_menu = true;
                    open_rename = Some(m.folder.clone());
                }
                "deleteFolder" => {
                    actions.push(Action::DeleteFolder(m.folder.clone()));
                    close_menu = true;
                }
                other => {
                    if let Some(to) = other.strip_prefix("moveTo:") {
                        actions.push(Action::MoveSnippet(m.snippet.clone(), m.folder.clone(), to.to_string()));
                        close_menu = true;
                    }
                }
            }
        } else if !m.opened {
            let outside = ctx.input(|i| {
                (i.pointer.any_pressed()) && i.pointer.interact_pos().map(|p| !mr.contains(p)).unwrap_or(false)
            });
            if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_menu = true;
            }
        }
        m.opened = false;
    }
    if close_menu {
        lib.ctx_menu = None;
    }
    if let Some(name) = open_rename {
        lib.rename = Some(Rename { original: name.clone(), input: name, focus_pending: true });
    }

    // ── Rename popup (modal) ────────────────────────────────────────────
    let mut close_rename = false;
    if let Some(rn) = &mut lib.rename {
        let (mut mu, clicked_outside) = widgets::modal_layer(&ctx, Id::new("rename-modal"), screen, true);
        let pr = Rect::from_center_size(lib.root.unwrap_or(screen).center(), Vec2::new(340.0, 170.0));
        let c = Canvas::new(mu.painter());
        c.rect(pr, 16.0, th.surface);
        let _eat = mu.interact(pr, Id::new("rename-box"), Sense::CLICK);
        let inner = pr.shrink(22.0);
        let tr = c.text(inner.left_top(), Align2::LEFT_TOP, "Rename Folder", fb(15.0), th.text_primary);
        let ir = rect_xywh(inner.left(), tr.bottom() + 14.0, inner.width(), 36.0);
        let iid = Id::new("rename-input");
        let focused = ctx.memory(|m| m.has_focus(iid));
        c.rect_fs(ir, 10.0, if th.is_dark { theme::hex(0x1a1d23) } else { Color32::WHITE }, 1.0, if focused { th.primary } else { th.hover_bg });
        let out = widgets::line_edit(&mut mu, iid, ir.shrink(10.0), &mut rn.input, f(12.0), th.text_primary, None);
        if rn.focus_pending {
            widgets::request_focus(&mu, iid);
            widgets::select_all(&ctx, iid, &rn.input);
            rn.focus_pending = false;
        }
        let by = ir.bottom() + 14.0;
        let ok_r = rect_xywh(inner.right() - 90.0, by, 90.0, 34.0);
        let cancel_r = ok_r.translate(Vec2::new(-100.0, 0.0));
        c.rect(cancel_r, 10.0, th.hover_bg);
        c.text(cancel_r.center(), Align2::CENTER_CENTER, "Cancel", fb(11.0), th.text_primary);
        c.rect(ok_r, 10.0, th.primary);
        c.text(ok_r.center(), Align2::CENTER_CENTER, "OK", fb(11.0), Color32::WHITE);
        let cancel = mu.interact(cancel_r, Id::new("rename-cancel"), Sense::CLICK).clicked();
        let ok = mu.interact(ok_r, Id::new("rename-ok"), Sense::CLICK).clicked() || out.submitted;
        if ok {
            let val = rn.input.trim().to_string();
            if !val.is_empty() && val != rn.original {
                actions.push(Action::RenameFolder(rn.original.clone(), val));
            }
            close_rename = true;
        } else if cancel || out.escaped || clicked_outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_rename = true;
        }
    }
    if close_rename {
        lib.rename = None;
    }
}
