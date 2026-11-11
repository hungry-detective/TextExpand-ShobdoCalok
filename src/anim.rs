//! QML `Behavior { NumberAnimation / ColorAnimation }` equivalents.

use egui::{Color32, Context, Id};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ease {
    Linear,
    OutCubic,
    OutQuad,
    InOutQuad,
    InOutSine,
    OutBack,
}

pub fn ease(e: Ease, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Ease::Linear => t,
        Ease::OutCubic => 1.0 - (1.0 - t).powi(3),
        Ease::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Ease::InOutQuad => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        Ease::InOutSine => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
        Ease::OutBack => {
            // Qt's default overshoot 1.70158
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AnimF {
    from: f32,
    to: f32,
    start: f64,
    dur: f32,
    ease: Ease,
}

impl AnimF {
    fn value(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) as f32 / self.dur).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * ease(self.ease, t)
    }
    fn done(&self, now: f64) -> bool {
        (now - self.start) as f32 >= self.dur
    }
}

/// Animated value that follows `target` (Behavior on x { NumberAnimation }).
/// The first time an id is seen the target is returned without animating.
pub fn anim(ctx: &Context, id: Id, target: f32, dur_ms: f32, e: Ease) -> f32 {
    let now = ctx.input(|i| i.time);
    let dur = dur_ms / 1000.0;
    let st = ctx.data(|d| d.get_temp::<AnimF>(id));
    let mut s = match st {
        None => {
            let s = AnimF { from: target, to: target, start: now, dur, ease: e };
            ctx.data_mut(|d| d.insert_temp(id, s));
            return target;
        }
        Some(s) => s,
    };
    if (s.to - target).abs() > f32::EPSILON {
        let cur = s.value(now);
        s = AnimF { from: cur, to: target, start: now, dur, ease: e };
        ctx.data_mut(|d| d.insert_temp(id, s));
    }
    let v = s.value(now);
    if !s.done(now) {
        ctx.request_repaint();
    }
    v
}

/// Force an animated value to jump (no animation).
pub fn set_now(ctx: &Context, id: Id, value: f32) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(id, AnimF { from: value, to: value, start: now, dur: 0.0, ease: Ease::Linear }));
}

/// Start an explicit animation from `from` to `to` (SequentialAnimation step).
pub fn start_from(ctx: &Context, id: Id, from: f32, to: f32, dur_ms: f32, e: Ease) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(id, AnimF { from, to, start: now, dur: dur_ms / 1000.0, ease: e }));
    ctx.request_repaint();
}

#[derive(Clone, Copy, Debug)]
struct AnimC {
    from: Color32,
    to: Color32,
    start: f64,
    dur: f32,
}

/// Behavior on color { ColorAnimation { duration } }
pub fn anim_color(ctx: &Context, id: Id, target: Color32, dur_ms: f32) -> Color32 {
    let now = ctx.input(|i| i.time);
    let dur = dur_ms / 1000.0;
    let st = ctx.data(|d| d.get_temp::<AnimC>(id));
    let val = |s: &AnimC| {
        let t = if s.dur <= 0.0 { 1.0 } else { ((now - s.start) as f32 / s.dur).clamp(0.0, 1.0) };
        crate::theme::lerp_color(s.from, s.to, t)
    };
    let mut s = match st {
        None => {
            ctx.data_mut(|d| d.insert_temp(id, AnimC { from: target, to: target, start: now, dur }));
            return target;
        }
        Some(s) => s,
    };
    if s.to != target {
        let cur = val(&s);
        s = AnimC { from: cur, to: target, start: now, dur };
        ctx.data_mut(|d| d.insert_temp(id, s));
    }
    if ((now - s.start) as f32) < s.dur {
        ctx.request_repaint();
    }
    val(&s)
}

/// Infinite `SequentialAnimation { to: hi; to: lo }` (InOutSine), each leg `leg_ms`.
pub fn ping_pong(ctx: &Context, lo: f32, hi: f32, leg_ms: f32, e: Ease) -> f32 {
    let t = ctx.input(|i| i.time) as f32 * 1000.0;
    ctx.request_repaint();
    let period = leg_ms * 2.0;
    let ph = t % period;
    if ph < leg_ms {
        lo + (hi - lo) * ease(e, ph / leg_ms)
    } else {
        hi + (lo - hi) * ease(e, (ph - leg_ms) / leg_ms)
    }
}

/// Infinite `from → to` loop that restarts (RotationAnimation with loops).
pub fn looping(ctx: &Context, from: f32, to: f32, dur_ms: f32, e: Ease) -> f32 {
    let t = ctx.input(|i| i.time) as f32 * 1000.0;
    ctx.request_repaint();
    from + (to - from) * ease(e, (t % dur_ms) / dur_ms)
}

/// Blink like `SequentialAnimation on opacity { to: 0; to: 1 }` (linear legs).
pub fn blink(ctx: &Context, leg_ms: f32, lo: f32, hi: f32) -> f32 {
    let t = ctx.input(|i| i.time) as f32 * 1000.0;
    ctx.request_repaint();
    let ph = t % (leg_ms * 2.0);
    if ph < leg_ms {
        hi + (lo - hi) * (ph / leg_ms)
    } else {
        lo + (hi - lo) * ((ph - leg_ms) / leg_ms)
    }
}

/// One-shot timeline helper: seconds since `key` was (re)started.
pub fn since(ctx: &Context, id: Id) -> Option<f64> {
    let now = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<f64>(id)).map(|s| now - s)
}

pub fn restart(ctx: &Context, id: Id) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(id, now));
    ctx.request_repaint();
}

/// Pulse: 1 → peak (dur1, e1) → 1 (dur2, e2), started with `restart(id)`.
pub fn pulse(ctx: &Context, id: Id, peak: f32, dur1: f32, e1: Ease, dur2: f32, e2: Ease) -> f32 {
    let Some(el) = since(ctx, id) else { return 1.0 };
    let ms = el as f32 * 1000.0;
    if ms < dur1 {
        ctx.request_repaint();
        1.0 + (peak - 1.0) * ease(e1, ms / dur1)
    } else if ms < dur1 + dur2 {
        ctx.request_repaint();
        peak + (1.0 - peak) * ease(e2, (ms - dur1) / dur2)
    } else {
        1.0
    }
}
