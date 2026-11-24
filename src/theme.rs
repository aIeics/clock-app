//! The app's look: a dark palette, the egui style, and small custom-painted
//! widgets (buttons, toggles, segmented controls, icons) shared by every tab.
//!
//! Animations here only request repaints while they are moving, so a view
//! with nothing changing stays idle.

use std::f32::consts::TAU;

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Id, Margin, Mesh, Painter, Pos2, Rect, Response, Sense,
    Shadow, Stroke, StrokeKind, Theme, Ui, Vec2, emath,
};

pub const BG: Color32 = Color32::from_rgb(9, 10, 17);
pub const SURFACE: Color32 = Color32::from_rgb(19, 21, 32);
pub const SURFACE_HI: Color32 = Color32::from_rgb(28, 31, 46);
pub const BORDER: Color32 = Color32::from_rgb(40, 44, 62);
pub const TEXT: Color32 = Color32::from_rgb(236, 238, 246);
pub const TEXT_DIM: Color32 = Color32::from_rgb(138, 144, 166);
pub const ACCENT_A: Color32 = Color32::from_rgb(139, 92, 246);
pub const ACCENT_B: Color32 = Color32::from_rgb(34, 211, 238);
pub const DANGER: Color32 = Color32::from_rgb(244, 63, 94);

const RADIUS: u8 = 10;

/// Installs the dark style. The theme is forced to dark so the look doesn't
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
            color: Color32::from_black_alpha(120),
        };
        visuals.popup_shadow = visuals.window_shadow;
        visuals.extreme_bg_color = Color32::from_rgb(13, 14, 23);
        visuals.text_edit_bg_color = Some(Color32::from_rgb(13, 14, 23));
        visuals.faint_bg_color = SURFACE;
        visuals.error_fg_color = DANGER;
        visuals.selection.bg_fill = ACCENT_A.gamma_multiply(0.6);
        visuals.selection.stroke = Stroke::new(1.0, ACCENT_B);

        let widgets = &mut visuals.widgets;
        for (state, fill, stroke) in [
            (&mut widgets.noninteractive, SURFACE, BORDER),
            (&mut widgets.inactive, SURFACE_HI, BORDER),
            (
                &mut widgets.hovered,
                Color32::from_rgb(36, 40, 60),
                ACCENT_A,
            ),
            (&mut widgets.active, Color32::from_rgb(44, 48, 72), ACCENT_B),
            (&mut widgets.open, SURFACE_HI, ACCENT_A),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, stroke);
            state.corner_radius = CornerRadius::same(RADIUS);
        }
        widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        widgets.hovered.fg_stroke = Stroke::new(1.5, TEXT);
        widgets.active.fg_stroke = Stroke::new(1.5, TEXT);
        widgets.hovered.expansion = 0.0;
        widgets.active.expansion = 0.0;
    });
}

/// Eased value animation. Like `animate_value_with_time` but with a
/// `cubic_out` curve. Returns `target` the first time an id is seen.
pub fn animate(ctx: &egui::Context, id: Id, target: f32, duration: f32) -> f32 {
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
            anim.from = emath::lerp(anim.from..=anim.to, emath::easing::cubic_out(t));
            anim.to = target;
            anim.start = now;
        }
        *anim
    });

    let t = ((now - anim.start) as f32 / duration).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    emath::lerp(anim.from..=anim.to, emath::easing::cubic_out(t))
}

/// Paints the window background: a dark fill with two soft colour glows.
pub fn backdrop(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, BG);
    let size = rect.width().max(rect.height());
    glow(
        painter,
        rect.left_top() + Vec2::new(rect.width() * 0.1, -size * 0.05),
        size * 0.75,
        ACCENT_A.gamma_multiply(0.16),
    );
    glow(
        painter,
        rect.right_bottom() + Vec2::new(-rect.width() * 0.05, size * 0.05),
        size * 0.7,
        ACCENT_B.gamma_multiply(0.10),
    );
}

/// A radial glow: `color` in the middle fading to transparent at `radius`.
pub fn glow(painter: &Painter, center: Pos2, radius: f32, color: Color32) {
    const RINGS: [(f32, f32); 4] = [(0.0, 1.0), (0.35, 0.6), (0.7, 0.22), (1.0, 0.0)];
    const SEGMENTS: u32 = 48;

    let mut mesh = Mesh::default();
    for (ring, (r, alpha)) in RINGS.iter().enumerate() {
        for i in 0..SEGMENTS {
            let angle = i as f32 / SEGMENTS as f32 * TAU;
            let pos = center + Vec2::angled(angle) * radius * *r;
            mesh.colored_vertex(pos, color.gamma_multiply(*alpha));
        }
        if ring > 0 {
            let (inner, outer) = ((ring as u32 - 1) * SEGMENTS, ring as u32 * SEGMENTS);
            for i in 0..SEGMENTS {
                let j = (i + 1) % SEGMENTS;
                mesh.add_triangle(inner + i, outer + i, outer + j);
                mesh.add_triangle(inner + i, outer + j, inner + j);
            }
        }
    }
    painter.add(mesh);
}

/// A rounded rectangle filled with a left-to-right gradient.
pub fn gradient_rect(painter: &Painter, rect: Rect, radius: f32, left: Color32, right: Color32) {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right_bottom() - Vec2::splat(radius), 0.0),
        (rect.left_bottom() + Vec2::new(radius, -radius), 0.25),
        (rect.left_top() + Vec2::splat(radius), 0.5),
        (rect.right_top() + Vec2::new(-radius, radius), 0.75),
    ];

    let mut mesh = Mesh::default();
    let color_at = |x: f32| {
        let t = ((x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
        left.lerp_to_gamma(right, t)
    };
    mesh.colored_vertex(rect.center(), color_at(rect.center().x));
    const STEPS: usize = 8;
    for (corner, start) in corners {
        for step in 0..=STEPS {
            let angle = (start + step as f32 / STEPS as f32 * 0.25) * TAU;
            let pos = corner + Vec2::angled(angle) * radius;
            mesh.colored_vertex(pos, color_at(pos.x));
        }
    }
    let count = mesh.vertices.len() as u32 - 1;
    for i in 1..=count {
        let next = if i == count { 1 } else { i + 1 };
        mesh.add_triangle(0, i, next);
    }
    painter.add(mesh);
}

/// A card: a slightly raised, rounded panel with a thin border.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE.gamma_multiply(0.92))
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(16)
        .inner_margin(Margin::same(16))
        .shadow(Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
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
/// width so changing numbers don't make the text wobble.
pub fn tabular_text(painter: &Painter, center: Pos2, text: &str, font: FontId, color: Color32) {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Gradient fill: the main action on a screen.
    Primary,
    /// Quiet fill with a border that lights up on hover.
    Ghost,
    /// Red, for destructive actions.
    Danger,
}

/// A rounded button with a hover glow and press feedback.
pub fn button(ui: &mut Ui, text: &str, kind: Kind) -> Response {
    let font = FontId::proportional(14.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, Color32::WHITE);
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

        match kind {
            Kind::Primary => {
                if hover > 0.0 {
                    painter.rect_filled(
                        rect.expand(4.0 * hover).translate(Vec2::new(0.0, 3.0)),
                        radius + 4.0,
                        ACCENT_A.gamma_multiply(0.25 * hover),
                    );
                }
                let lift = 0.14 * hover;
                gradient_rect(
                    painter,
                    rect,
                    radius,
                    ACCENT_A.lerp_to_gamma(Color32::WHITE, lift),
                    ACCENT_B
                        .lerp_to_gamma(ACCENT_A, 0.35)
                        .lerp_to_gamma(Color32::WHITE, lift),
                );
            }
            Kind::Ghost | Kind::Danger => {
                let accent = if kind == Kind::Danger {
                    DANGER
                } else {
                    ACCENT_A
                };
                painter.rect_filled(rect, radius, SURFACE_HI.lerp_to_gamma(accent, 0.12 * hover));
                painter.rect_stroke(
                    rect,
                    radius,
                    Stroke::new(1.0, BORDER.lerp_to_gamma(accent, hover)),
                    StrokeKind::Inside,
                );
            }
        }

        let color = match kind {
            Kind::Primary => Color32::WHITE,
            Kind::Danger => DANGER.lerp_to_gamma(TEXT, 0.3),
            Kind::Ghost => TEXT,
        };
        let color = if enabled {
            color
        } else {
            color.gamma_multiply(0.4)
        };
        painter.galley(rect.center() - galley.size() / 2.0, galley, color);
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

/// A round button showing one icon.
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

        if primary {
            painter.circle_filled(
                center + Vec2::new(0.0, 3.0),
                radius + 5.0 * hover,
                ACCENT_A.gamma_multiply(0.28 * hover),
            );
            let disc = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
            gradient_rect(painter, disc, radius, ACCENT_A, ACCENT_B);
        } else {
            painter.circle(
                center,
                radius,
                SURFACE_HI.lerp_to_gamma(ACCENT_A, 0.18 * hover),
                Stroke::new(1.0, BORDER.lerp_to_gamma(ACCENT_A, hover)),
            );
        }

        let color = if primary {
            Color32::WHITE
        } else {
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

        painter.rect_filled(track_rect, r, SURFACE_HI);
        if t > 0.0 {
            gradient_rect(
                painter,
                track_rect,
                r,
                ACCENT_A.gamma_multiply(t),
                ACCENT_B.gamma_multiply(t),
            );
        }
        painter.rect_stroke(
            track_rect,
            r,
            Stroke::new(1.0, BORDER.lerp_to_gamma(ACCENT_A, hover * (1.0 - t))),
            StrokeKind::Inside,
        );
        let knob_x = emath::lerp((track_rect.left() + r)..=(track_rect.right() - r), t);
        let knob = Pos2::new(knob_x, track_rect.center().y);
        painter.circle_filled(
            knob + Vec2::new(0.0, 1.0),
            r - 2.0,
            Color32::from_black_alpha(70),
        );
        painter.circle_filled(knob, r - 3.0 + hover, Color32::WHITE);

        let text_pos = Pos2::new(
            track_rect.right() + 10.0,
            rect.center().y - galley.size().y / 2.0,
        );
        painter.galley(text_pos, galley, TEXT);
    }

    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of options with a pill that slides to the selected one.
/// Returns `true` when the selection changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: Id,
    value: &mut T,
    options: &[(T, &str)],
    width: f32,
    height: f32,
) -> bool {
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
    let radius = height / 2.0;
    painter.rect_filled(rect, radius, SURFACE);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, BORDER), StrokeKind::Inside);

    let pos = animate(ui.ctx(), id.with("pill"), selected as f32, 0.28);
    let pill = Rect::from_min_size(
        Pos2::new(rect.left() + pad + pos * slot, rect.top() + pad),
        Vec2::new(slot, height - pad * 2.0),
    );
    painter.rect_filled(
        pill.translate(Vec2::new(0.0, 2.0)).expand(2.0),
        radius,
        ACCENT_A.gamma_multiply(0.18),
    );
    gradient_rect(
        &painter,
        pill,
        radius - pad,
        ACCENT_A,
        ACCENT_B.lerp_to_gamma(ACCENT_A, 0.35),
    );

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
        // Text turns white as the pill passes under it.
        let under = (1.0 - (pos - i as f32).abs()).clamp(0.0, 1.0);
        let color = TEXT_DIM.lerp_to_gamma(TEXT, hover.max(under));
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
