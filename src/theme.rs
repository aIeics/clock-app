//! The app's look: black background, white text, and small custom-painted
//! widgets (buttons, toggles, segmented controls, icons) shared by every tab.
//!
//! Animations here only request repaints while they are moving, so a view
//! with nothing changing stays idle.

use std::f32::consts::TAU;

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Id, Margin, Painter, Pos2, Rect, Response, Sense, Shadow,
    Stroke, StrokeKind, Theme, Ui, Vec2, emath,
};

pub const BG: Color32 = Color32::BLACK;
pub const SURFACE: Color32 = Color32::from_rgb(14, 14, 14);
pub const SURFACE_HI: Color32 = Color32::from_rgb(26, 26, 26);
pub const BORDER: Color32 = Color32::from_rgb(46, 46, 46);
pub const TEXT: Color32 = Color32::WHITE;
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 140);
/// Only for alerts and errors.
pub const DANGER: Color32 = Color32::from_rgb(244, 63, 94);

const RADIUS: u8 = 10;

/// Content never gets wider than this, so wide windows keep a centred column.
pub const MAX_WIDTH: f32 = 560.0;

/// Runs `add` in a column centred in the remaining space, at most `max_width`
/// wide, with its widgets centred horizontally.
pub fn column<R>(ui: &mut Ui, max_width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(max_width);
    let rect = Rect::from_min_size(
        Pos2::new(avail.center().x - width / 2.0, avail.top()),
        Vec2::new(width, avail.height()),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        add,
    )
    .inner
}

/// Installs the style. The theme is forced to dark so the look doesn't
/// depend on the Windows light/dark setting.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(Theme::Dark);
    ctx.style_mut_of(Theme::Dark, |style| {
        style.animation_time = 0.18;

        let spacing = &mut style.spacing;
        spacing.item_spacing = Vec2::new(8.0, 8.0);
        spacing.button_padding = Vec2::new(12.0, 6.0);
        spacing.interact_size.y = 30.0;
        spacing.combo_width = 90.0;

        let text = &mut style.text_styles;
        text.insert(egui::TextStyle::Heading, FontId::proportional(20.0));
        text.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        text.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        text.insert(egui::TextStyle::Small, FontId::proportional(12.0));

        let visuals = &mut style.visuals;
        visuals.dark_mode = true;
        visuals.override_text_color = None;
        visuals.panel_fill = BG;
        visuals.window_fill = SURFACE_HI;
        visuals.window_stroke = Stroke::new(1.0, BORDER);
        visuals.window_corner_radius = CornerRadius::same(14);
        visuals.menu_corner_radius = CornerRadius::same(12);
        visuals.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(160),
        };
        visuals.popup_shadow = visuals.window_shadow;
        visuals.extreme_bg_color = SURFACE;
        visuals.text_edit_bg_color = Some(SURFACE);
        visuals.faint_bg_color = SURFACE;
        visuals.error_fg_color = DANGER;
        visuals.selection.bg_fill = Color32::from_white_alpha(60);
        visuals.selection.stroke = Stroke::new(1.0, TEXT);

        let widgets = &mut visuals.widgets;
        for (state, fill, stroke) in [
            (&mut widgets.noninteractive, SURFACE, BORDER),
            (&mut widgets.inactive, SURFACE_HI, BORDER),
            (
                &mut widgets.hovered,
                Color32::from_rgb(36, 36, 36),
                TEXT_DIM,
            ),
            (&mut widgets.active, Color32::from_rgb(44, 44, 44), TEXT),
            (&mut widgets.open, SURFACE_HI, TEXT_DIM),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.corner_radius = CornerRadius::same(RADIUS);
            state.fg_stroke = Stroke::new(1.0, TEXT);
            state.expansion = 0.0;
        }
    });
}

/// Eased value animation with a `cubic_out` curve. Returns `target` the
/// first time an id is seen.
pub fn animate(ctx: &egui::Context, id: Id, target: f32, duration: f32) -> f32 {
    animate_with(ctx, id, target, duration, emath::easing::cubic_out)
}

/// Like [`animate`] with any easing curve. A jump of more than 100 (such as a
/// counter wrapping at midnight) snaps instead of animating.
pub fn animate_with(
    ctx: &egui::Context,
    id: Id,
    target: f32,
    duration: f32,
    easing: fn(f32) -> f32,
) -> f32 {
    #[derive(Clone, Copy)]
    struct Anim {
        from: f32,
        to: f32,
        start: f64,
    }

    let now = ctx.input(|input| input.time);
    let anim = ctx.data_mut(|data| {
        let anim = data.get_temp_mut_or_insert_with(id, || Anim {
            from: target,
            to: target,
            start: now,
        });
        if anim.to != target {
            let t = ((now - anim.start) as f32 / duration).clamp(0.0, 1.0);
            anim.from = if (target - anim.to).abs() > 100.0 {
                target
            } else {
                emath::lerp(anim.from..=anim.to, easing(t))
            };
            anim.to = target;
            anim.start = now;
        }
        *anim
    });

    let t = ((now - anim.start) as f32 / duration).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    emath::lerp(anim.from..=anim.to, easing(t))
}

/// Progress from 0 to 1 of a one-shot animation that started at `start`
/// (egui input time). Repaints only until it reaches 1.
pub fn since(ctx: &egui::Context, start: f64, duration: f32) -> f32 {
    let t = ((ctx.input(|input| input.time) - start) as f32 / duration).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    emath::easing::cubic_out(t)
}

/// A card: a slightly raised, rounded panel with a thin border.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(16)
        .inner_margin(Margin::same(16))
}

/// Small, dimmed, upper-case label above a group of controls.
pub fn section_label(ui: &mut Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(11.0)
            .color(TEXT_DIM)
            .extra_letter_spacing(1.2),
    );
}

/// Draws `text` centred on `center`, with every digit in a slot of the same
/// width so changing numbers don't make the text wobble. Returns its width.
pub fn tabular_text(
    painter: &Painter,
    center: Pos2,
    text: &str,
    font: FontId,
    color: Color32,
) -> f32 {
    let digit = painter
        .layout_no_wrap("0".into(), font.clone(), color)
        .size()
        .x;
    let pieces: Vec<_> = text
        .chars()
        .map(|c| {
            let galley = painter.layout_no_wrap(c.to_string(), font.clone(), color);
            let width = if c.is_ascii_digit() {
                digit
            } else {
                galley.size().x
            };
            (galley, width)
        })
        .collect();

    let total: f32 = pieces.iter().map(|(_, w)| w).sum();
    let height = pieces.iter().map(|(g, _)| g.size().y).fold(0.0, f32::max);
    let mut x = center.x - total / 2.0;
    for (galley, width) in pieces {
        let pos = Pos2::new(x + (width - galley.size().x) / 2.0, center.y - height / 2.0);
        painter.galley(pos, galley, color);
        x += width;
    }
    total
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Solid white with black text: the main action on a screen.
    Primary,
    /// Dark fill with a border that lights up on hover.
    Ghost,
    /// Red, for destructive actions.
    Danger,
}

/// A rounded button with hover and press feedback.
pub fn button(ui: &mut Ui, text: &str, kind: Kind) -> Response {
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::PLACEHOLDER);
    let size = Vec2::new((galley.size().x + 28.0).max(64.0), 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let hover = ui
            .ctx()
            .animate_bool_responsive(response.id.with("hover"), enabled && response.hovered());
        let rect = if response.is_pointer_button_down_on() {
            rect.shrink(1.0)
        } else {
            rect
        };
        let painter = ui.painter();
        let radius = f32::from(RADIUS);

        let text_color = match kind {
            Kind::Primary => {
                painter.rect_filled(rect, radius, TEXT.lerp_to_gamma(TEXT_DIM, 0.25 * hover));
                BG
            }
            Kind::Ghost | Kind::Danger => {
                let accent = if kind == Kind::Danger { DANGER } else { TEXT };
                painter.rect_filled(rect, radius, SURFACE_HI.lerp_to_gamma(accent, 0.08 * hover));
                painter.rect_stroke(
                    rect,
                    radius,
                    Stroke::new(1.0, BORDER.lerp_to_gamma(accent, hover)),
                    StrokeKind::Inside,
                );
                if kind == Kind::Danger { DANGER } else { TEXT }
            }
        };
        let text_color = if enabled {
            text_color
        } else {
            text_color.gamma_multiply(0.4)
        };
        painter.galley(rect.center() - galley.size() / 2.0, galley, text_color);
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    ChevronLeft,
    ChevronRight,
    Play,
    Pause,
    Reset,
    Plus,
    Minus,
    Edit,
    Trash,
    Close,
    Bell,
}

/// A round button showing one icon. `primary` is solid white.
pub fn icon_button(ui: &mut Ui, icon: Icon, diameter: f32, primary: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click());

    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let hover = ui
            .ctx()
            .animate_bool_responsive(response.id.with("hover"), enabled && response.hovered());
        let pressed = response.is_pointer_button_down_on();
        let radius = diameter / 2.0 - if pressed { 1.5 } else { 0.0 };
        let painter = ui.painter();
        let center = rect.center();

        let color = if primary {
            painter.circle_filled(center, radius, TEXT.lerp_to_gamma(TEXT_DIM, 0.25 * hover));
            BG
        } else {
            painter.circle(
                center,
                radius,
                SURFACE_HI.lerp_to_gamma(TEXT, 0.08 * hover),
                Stroke::new(1.0, BORDER.lerp_to_gamma(TEXT_DIM, hover)),
            );
            TEXT_DIM.lerp_to_gamma(TEXT, hover)
        };
        let color = if enabled {
            color
        } else {
            color.gamma_multiply(0.4)
        };
        let glyph = Rect::from_center_size(center, Vec2::splat(diameter * 0.42));
        paint_icon(painter, glyph, icon, color);
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Draws a simple line icon inside `rect`.
pub fn paint_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width();
    let c = rect.center();
    let stroke = Stroke::new((s * 0.12).clamp(1.4, 2.4), color);
    let p = |x: f32, y: f32| c + Vec2::new(x, y) * s;

    match icon {
        Icon::ChevronLeft => {
            painter.line(vec![p(0.15, -0.35), p(-0.15, 0.0), p(0.15, 0.35)], stroke);
        }
        Icon::ChevronRight => {
            painter.line(vec![p(-0.15, -0.35), p(0.15, 0.0), p(-0.15, 0.35)], stroke);
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(-0.28, -0.42), p(0.45, 0.0), p(-0.28, 0.42)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            for x in [-0.2, 0.2] {
                painter.rect_filled(
                    Rect::from_center_size(p(x, 0.0), Vec2::new(s * 0.16, s * 0.8)),
                    s * 0.05,
                    color,
                );
            }
        }
        Icon::Reset => {
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let angle = (-0.15 + i as f32 / 20.0 * 0.78) * TAU;
                    c + Vec2::angled(angle) * s * 0.38
                })
                .collect();
            let tip = points[0];
            painter.line(points, stroke);
            painter.line(
                vec![
                    tip + Vec2::new(-0.2, -0.08) * s,
                    tip,
                    tip + Vec2::new(0.06, -0.24) * s,
                ],
                stroke,
            );
        }
        Icon::Plus => {
            painter.line_segment([p(-0.38, 0.0), p(0.38, 0.0)], stroke);
            painter.line_segment([p(0.0, -0.38), p(0.0, 0.38)], stroke);
        }
        Icon::Minus => {
            painter.line_segment([p(-0.38, 0.0), p(0.38, 0.0)], stroke);
        }
        Icon::Close => {
            painter.line_segment([p(-0.3, -0.3), p(0.3, 0.3)], stroke);
            painter.line_segment([p(0.3, -0.3), p(-0.3, 0.3)], stroke);
        }
        Icon::Edit => {
            painter.line_segment([p(-0.32, 0.32), p(0.28, -0.28)], stroke);
            painter.line_segment([p(0.14, -0.42), p(0.42, -0.14)], stroke);
            painter.line_segment([p(-0.42, 0.42), p(-0.2, 0.42)], stroke);
        }
        Icon::Trash => {
            painter.line_segment([p(-0.42, -0.26), p(0.42, -0.26)], stroke);
            painter.line_segment([p(-0.12, -0.42), p(0.12, -0.42)], stroke);
            painter.line(
                vec![p(-0.3, -0.26), p(-0.22, 0.42), p(0.22, 0.42), p(0.3, -0.26)],
                stroke,
            );
        }
        Icon::Bell => {
            let body: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let angle = (0.5 + i as f32 / 16.0 * 0.5) * TAU;
                    p(0.0, -0.05) + Vec2::angled(angle) * s * 0.3
                })
                .collect();
            painter.line(body, stroke);
            painter.line(
                vec![p(-0.3, -0.05), p(-0.3, 0.2), p(-0.42, 0.3), p(0.42, 0.3)],
                stroke,
            );
            painter.line(vec![p(0.3, -0.05), p(0.3, 0.2), p(0.42, 0.3)], stroke);
            painter.circle_filled(p(0.0, 0.42), s * 0.08, color);
        }
    }
}

/// An animated on/off switch with a label to its right.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(14.0), TEXT);
    let track = Vec2::new(40.0, 22.0);
    let size = Vec2::new(
        track.x + 10.0 + galley.size().x,
        track.y.max(galley.size().y),
    );
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        let t = ui
            .ctx()
            .animate_bool_with_easing(response.id, *on, emath::easing::cubic_out);
        let hover = ui
            .ctx()
            .animate_bool_responsive(response.id.with("hover"), response.hovered());
        let painter = ui.painter();
        let track_rect = Rect::from_min_size(
            Pos2::new(rect.left(), rect.center().y - track.y / 2.0),
            track,
        );
        let r = track.y / 2.0;

        painter.rect_filled(track_rect, r, SURFACE_HI.lerp_to_gamma(TEXT, t));
        painter.rect_stroke(
            track_rect,
            r,
            Stroke::new(1.0, BORDER.lerp_to_gamma(TEXT_DIM, hover * (1.0 - t))),
            StrokeKind::Inside,
        );
        let knob_x = emath::lerp((track_rect.left() + r)..=(track_rect.right() - r), t);
        let knob = Pos2::new(knob_x, track_rect.center().y);
        painter.circle_filled(knob, r - 3.0 + hover, TEXT_DIM.lerp_to_gamma(BG, t));

        let text_pos = Pos2::new(
            track_rect.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley, TEXT);
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Width of a [`check`] with this label.
pub fn check_width(ui: &Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(label.to_string(), FontId::proportional(13.0), TEXT)
        .size()
        .x
        + 17.0
}

/// A small square checkbox with a label, filled white when on.
pub fn check(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(13.0),
        Color32::PLACEHOLDER,
    );
    let size = Vec2::new(galley.size().x + 17.0, 28.0);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_responsive(response.id, *on);
        let hover = ui
            .ctx()
            .animate_bool_responsive(response.id.with("hover"), response.hovered());
        let painter = ui.painter();
        let square = Rect::from_center_size(
            Pos2::new(rect.left() + 5.0, rect.center().y),
            Vec2::splat(10.0),
        );
        painter.rect_filled(square, 0.0, BG.lerp_to_gamma(TEXT, t));
        painter.rect_stroke(
            square,
            0.0,
            Stroke::new(1.0, TEXT_DIM.lerp_to_gamma(TEXT, t.max(hover))),
            StrokeKind::Inside,
        );
        let color = TEXT_DIM.lerp_to_gamma(TEXT, t.max(hover));
        painter.galley(
            Pos2::new(rect.left() + 17.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of options with a white pill that slides to the selected one.
/// `radius` 0 gives square corners. Returns `true` when the selection changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: Id,
    value: &mut T,
    options: &[(T, &str)],
    size: Vec2,
    radius: f32,
) -> bool {
    let (width, height) = (size.x, size.y);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let mut changed = false;
    let count = options.len().max(1) as f32;
    let pad = 3.0;
    let slot = (rect.width() - pad * 2.0) / count;
    let selected = options
        .iter()
        .position(|(option, _)| option == value)
        .unwrap_or(0);

    let painter = ui.painter().clone();
    painter.rect_filled(rect, radius, SURFACE);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, BORDER), StrokeKind::Inside);

    let pos = animate(ui.ctx(), id.with("pill"), selected as f32, 0.28);
    let pill = Rect::from_min_size(
        Pos2::new(rect.left() + pad + pos * slot, rect.top() + pad),
        Vec2::new(slot, height - pad * 2.0),
    );
    painter.rect_filled(pill, (radius - pad).max(0.0), TEXT);

    for (i, (option, label)) in options.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(rect.left() + pad + i as f32 * slot, rect.top()),
            Vec2::new(slot, height),
        );
        let response = ui
            .interact(cell, id.with(i), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if response.clicked() && *option != *value {
            *value = *option;
            changed = true;
        }
        let hover = ui
            .ctx()
            .animate_bool_responsive(id.with(i).with("hover"), response.hovered());
        // Text turns black as the white pill passes under it.
        let under = (1.0 - (pos - i as f32).abs()).clamp(0.0, 1.0);
        let color = TEXT_DIM.lerp_to_gamma(TEXT, hover).lerp_to_gamma(BG, under);
        painter.text(
            cell.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            FontId::proportional(13.5),
            color,
        );
    }

    changed
}

/// A text field with the app's styling.
pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(egui::RichText::new(hint).color(TEXT_DIM.gamma_multiply(0.7)))
            .desired_width(width)
            .margin(Margin::symmetric(10, 7)),
    )
}

/// Red error text.
pub fn error_text(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(DANGER).size(13.0));
}
