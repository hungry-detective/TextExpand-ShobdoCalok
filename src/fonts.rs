//! Font setup. The QML app asks for "Inter" (falls back to the system UI font,
//! Segoe UI, when Inter is not installed), "Courier New" for abbreviations and
//! "Material Symbols Outlined" for every icon glyph.
//!
//! egui has no OpenType ligature support, so icon names are mapped to their
//! Private-Use-Area code points (extracted from the font's GSUB table).

use egui::{FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

static MATERIAL: &[u8] = include_bytes!("../assets/MaterialSymbolsOutlined-subset.ttf");

pub const ICONS: &[(&str, u32)] = &[
    ("add_box", 0xe146),
    ("arrow_back", 0xe5c4),
    ("auto_awesome", 0xe65f),
    ("auto_fix_high", 0xe663),
    ("backup", 0xe864),
    ("bolt", 0xea0b),
    ("calendar_month", 0xebcc),
    ("calendar_today", 0xe935),
    ("chat", 0xe0c9),
    ("check", 0xe5ca),
    ("check_circle", 0xf0be),
    ("close", 0xe5cd),
    ("cloud", 0xf15c),
    ("cloud_done", 0xe2bf),
    ("cloud_upload", 0xe2c3),
    ("code_blocks", 0xf84d),
    ("content_copy", 0xe14d),
    ("create_new_folder", 0xe2cc),
    ("dark_mode", 0xe51c),
    ("delete", 0xe92e),
    ("delete_outline", 0xe92e),
    ("delete_sweep", 0xe16c),
    ("description", 0xe873),
    ("download", 0xf090),
    ("drive_file_move", 0xe9a1),
    ("drive_file_rename_outline", 0xe9a2),
    ("edit_note", 0xe745),
    ("expand_more", 0xe5cf),
    ("folder", 0xe2c7),
    ("forum", 0xe8af),
    ("history", 0xe8b3),
    ("inbox", 0xe156),
    ("info", 0xe88e),
    ("input", 0xe890),
    ("keyboard", 0xe312),
    ("light_mode", 0xe518),
    ("mail", 0xe159),
    ("more_vert", 0xe5d4),
    ("preview", 0xf1c5),
    ("refresh", 0xe5d5),
    ("rocket_launch", 0xeb9b),
    ("schedule", 0xefd6),
    ("school", 0xe80c),
    ("search", 0xe8b6),
    ("send", 0xe163),
    ("settings", 0xe8b8),
    ("smartphone", 0xe7ba),
    ("system_update", 0xf2cd),
    ("text_fields", 0xe262),
    ("touch_app", 0xe913),
    ("unfold_less", 0xe5d6),
    ("unfold_more", 0xe5d7),
    ("upload_file", 0xe9fc),
];

/// Material Symbols glyph for an icon name (as used in the QML `text:` property).
pub fn icon(name: &str) -> String {
    ICONS
        .iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, cp)| char::from_u32(*cp))
        .map(|c| c.to_string())
        .unwrap_or_default()
}

pub const UI: &str = "ui";
pub const UI_BOLD: &str = "ui_bold";
pub const UI_SEMIBOLD: &str = "ui_semibold";
pub const UI_ITALIC: &str = "ui_italic";
pub const MONO_BOLD: &str = "mono_bold";
pub const ICON_FAMILY: &str = "icons";

/// Regular UI font ("Inter" → Segoe UI)
pub fn f(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(UI.into()))
}
/// Bold UI font
pub fn fb(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(UI_BOLD.into()))
}
/// Medium (500) UI font
pub fn fm(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(UI_SEMIBOLD.into()))
}
/// Italic UI font
pub fn fi(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(UI_ITALIC.into()))
}
/// Courier New Bold
pub fn mono_b(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MONO_BOLD.into()))
}
/// Material Symbols Outlined
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON_FAMILY.into()))
}

fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = vec![];
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    dirs.push(std::path::Path::new(&windir).join("Fonts"));
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(std::path::Path::new(&local).join("Microsoft\\Windows\\Fonts"));
    }
    dirs
}

fn read_first(names: &[&str]) -> Option<Vec<u8>> {
    for d in font_dirs() {
        for n in names {
            if let Ok(b) = std::fs::read(d.join(n)) {
                return Some(b);
            }
        }
    }
    None
}

pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let mut add = |defs: &mut FontDefinitions, key: &str, bytes: Option<Vec<u8>>| -> bool {
        if let Some(b) = bytes {
            defs.font_data.insert(key.to_owned(), Arc::new(FontData::from_owned(b)));
            true
        } else {
            false
        }
    };

    // "Inter" if the user has it installed (that's what Qt would pick), else Segoe UI.
    let has_ui = add(&mut defs, "ui-regular", read_first(&["Inter-Regular.ttf", "Inter-Regular.otf", "segoeui.ttf"]));
    let has_bold = add(&mut defs, "ui-bold", read_first(&["Inter-Bold.ttf", "Inter-Bold.otf", "segoeuib.ttf"]));
    let has_semi = add(&mut defs, "ui-semibold", read_first(&["Inter-Medium.ttf", "Inter-SemiBold.ttf", "seguisb.ttf", "segoeui.ttf"]));
    let has_italic = add(&mut defs, "ui-italic", read_first(&["Inter-Italic.ttf", "segoeuii.ttf"]));
    let has_mono_b = add(&mut defs, "mono-bold", read_first(&["courbd.ttf", "cour.ttf"]));
    let has_sym = add(&mut defs, "symbols", read_first(&["seguisym.ttf"]));
    let has_emoji = add(&mut defs, "emoji-win", read_first(&["seguiemj.ttf"]));
    defs.font_data.insert(
        "material".to_owned(),
        Arc::new(FontData::from_static(MATERIAL)),
    );

    let builtin: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let mut fallbacks: Vec<String> = vec![];
    if has_sym {
        fallbacks.push("symbols".into());
    }
    if has_emoji {
        fallbacks.push("emoji-win".into());
    }
    fallbacks.extend(builtin.iter().cloned());

    let chain = |primary: Option<&str>| -> Vec<String> {
        let mut v = vec![];
        if let Some(p) = primary {
            v.push(p.to_owned());
        }
        v.extend(fallbacks.iter().cloned());
        v
    };

    let ui = has_ui.then_some("ui-regular");
    defs.families.insert(FontFamily::Name(UI.into()), chain(ui));
    defs.families.insert(FontFamily::Name(UI_BOLD.into()), chain(has_bold.then_some("ui-bold").or(ui)));
    defs.families.insert(FontFamily::Name(UI_SEMIBOLD.into()), chain(has_semi.then_some("ui-semibold").or(ui)));
    defs.families.insert(FontFamily::Name(UI_ITALIC.into()), chain(has_italic.then_some("ui-italic").or(ui)));
    defs.families.insert(FontFamily::Name(MONO_BOLD.into()), {
        let mut v = chain(has_mono_b.then_some("mono-bold"));
        v.extend(defs.families.get(&FontFamily::Monospace).cloned().unwrap_or_default());
        v
    });
    defs.families.insert(FontFamily::Name(ICON_FAMILY.into()), vec!["material".into()]);
    // Make the default proportional family the UI font too (TextEdit etc.)
    defs.families.insert(FontFamily::Proportional, chain(ui));

    ctx.set_fonts(defs);
}
