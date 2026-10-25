//! Look of the dashboard. It reproduces the former web dashboard, which used Bootstrap 4.5 with
//! two variants: the stock "classic" theme and the Bootswatch "darkly" theme.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Response, RichText,
    Sense, Stroke, TextStyle, Ui, Vec2,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Classic,
    Darkly,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub kind: ThemeKind,
    pub bg: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub link: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_active: Color32,
    pub card_bg: Color32,
    pub card_header: Color32,
    pub card_border: Color32,
    pub tab_border: Color32,
    pub tab_active_bg: Color32,
    pub tab_active_text: Color32,
    pub tab_hover_border: Color32,
    pub input_bg: Color32,
    pub input_text: Color32,
    pub sidebar_bg: Color32,
    pub sidebar_text: Color32,
    pub sidebar_border: Color32,
    pub sidebar_hover: Color32,
    pub table_stripe: Color32,
}

impl Palette {
    pub fn classic() -> Self {
        Self {
            kind: ThemeKind::Classic,
            bg: Color32::WHITE,
            text: rgb(0x21, 0x25, 0x29),
            muted: Color32::GRAY,
            link: rgb(0x00, 0x7b, 0xff),
            primary: rgb(0x00, 0x7b, 0xff),
            primary_hover: rgb(0x00, 0x69, 0xd9),
            primary_active: rgb(0x00, 0x62, 0xcc),
            card_bg: Color32::WHITE,
            card_header: rgb(0xf7, 0xf7, 0xf7),
            card_border: rgb(0xdf, 0xdf, 0xdf),
            tab_border: rgb(0xde, 0xe2, 0xe6),
            tab_active_bg: Color32::WHITE,
            tab_active_text: rgb(0x49, 0x50, 0x57),
            tab_hover_border: rgb(0xe9, 0xec, 0xef),
            input_bg: Color32::WHITE,
            input_text: rgb(0x49, 0x50, 0x57),
            sidebar_bg: rgb(0x2e, 0x35, 0x3d),
            sidebar_text: rgb(0xe1, 0xff, 0xff),
            sidebar_border: rgb(0x23, 0x28, 0x2e),
            sidebar_hover: rgb(0x4f, 0x5b, 0x69),
            table_stripe: rgb(0xf2, 0xf2, 0xf2),
        }
    }

    pub fn darkly() -> Self {
        Self {
            kind: ThemeKind::Darkly,
            bg: rgb(0x22, 0x22, 0x22),
            text: Color32::WHITE,
            muted: Color32::GRAY,
            link: rgb(0x00, 0xbc, 0x8c),
            primary: rgb(0x37, 0x5a, 0x7f),
            primary_hover: rgb(0x2b, 0x47, 0x64),
            primary_active: rgb(0x28, 0x41, 0x5b),
            card_bg: rgb(0x30, 0x30, 0x30),
            card_header: rgb(0x44, 0x44, 0x44),
            card_border: rgb(0x1b, 0x1b, 0x1b),
            tab_border: rgb(0x44, 0x44, 0x44),
            tab_active_bg: rgb(0x22, 0x22, 0x22),
            tab_active_text: Color32::WHITE,
            tab_hover_border: rgb(0x44, 0x44, 0x44),
            input_bg: Color32::WHITE,
            input_text: rgb(0x44, 0x44, 0x44),
            sidebar_bg: rgb(0x24, 0x26, 0x29),
            sidebar_text: rgb(0xe1, 0xff, 0xff),
            sidebar_border: rgb(0x23, 0x28, 0x2e),
            sidebar_hover: rgb(0x4f, 0x5b, 0x69),
            table_stripe: rgb(0x2a, 0x2a, 0x2a),
        }
    }

    pub fn of(kind: ThemeKind) -> Self {
        match kind {
            ThemeKind::Classic => Self::classic(),
            ThemeKind::Darkly => Self::darkly(),
        }
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Bootstrap's `.25rem` border radius at the 15 px root size used by the dashboard.
pub const RADIUS: u8 = 4;

pub fn apply(ctx: &egui::Context, p: &Palette) {
    let mut style = (*ctx.global_style()).clone();

    let mut visuals = match p.kind {
        ThemeKind::Classic => egui::Visuals::light(),
        ThemeKind::Darkly => egui::Visuals::dark(),
    };
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.card_bg;
    visuals.window_stroke = Stroke::new(1.0, p.card_border);
    visuals.extreme_bg_color = p.input_bg;
    visuals.text_edit_bg_color = Some(p.input_bg);
    visuals.hyperlink_color = p.link;
    visuals.faint_bg_color = p.table_stripe;
    visuals.selection.bg_fill = p.primary;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.striped = false;

    let radius = CornerRadius::same(RADIUS);
    for (state, fill) in [
        (&mut visuals.widgets.inactive, p.primary),
        (&mut visuals.widgets.hovered, p.primary_hover),
        (&mut visuals.widgets.active, p.primary_active),
        (&mut visuals.widgets.open, p.primary_hover),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, fill);
        state.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        state.corner_radius = radius;
    }
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.card_border);
    visuals.widgets.noninteractive.corner_radius = radius;

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(11.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(24.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_global_style(style);
}

/// A small badge (`<span class="badge">`).
pub fn badge(ui: &mut Ui, text: &str, color: Color32) {
    let font = FontId::new(11.0, FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE);
    let size = galley.size() + Vec2::new(8.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(RADIUS), color);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
}

/// Bootstrap `nav-tabs`. Returns the index of the clicked tab, if any.
pub fn nav_tabs(ui: &mut Ui, p: &Palette, labels: &[&str], selected: usize) -> Option<usize> {
    let font = FontId::new(15.0, FontFamily::Proportional);
    let height = 41.0;
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(row);

    // the tabs sit on a 1px line, active tab covers it
    let baseline = row.bottom() - 0.5;
    painter.hline(row.x_range(), baseline, Stroke::new(1.0, p.tab_border));

    let mut clicked = None;
    let mut x = row.left();
    for (index, label) in labels.iter().enumerate() {
        let active = index == selected;
        let color = if active { p.tab_active_text } else { p.link };
        let galley = painter.layout_no_wrap(label.to_string(), font.clone(), color);
        let tab_width = galley.size().x + 32.0;
        let rect =
            egui::Rect::from_min_size(egui::pos2(x, row.top()), Vec2::new(tab_width, height));
        let response = ui.interact(rect, ui.scope_id().with(("nav_tab", index)), Sense::click());

        let rounding = CornerRadius {
            nw: RADIUS,
            ne: RADIUS,
            sw: 0,
            se: 0,
        };
        if active {
            painter.rect(
                rect,
                rounding,
                p.tab_active_bg,
                Stroke::new(1.0, p.tab_border),
                egui::StrokeKind::Inside,
            );
            // erase the baseline below the active tab
            painter.hline(
                (rect.left() + 1.0)..=(rect.right() - 1.0),
                baseline,
                Stroke::new(1.5, p.tab_active_bg),
            );
        } else if response.hovered() {
            painter.rect(
                rect.shrink2(Vec2::new(0.0, 0.0)),
                rounding,
                Color32::TRANSPARENT,
                Stroke::new(1.0, p.tab_hover_border),
                egui::StrokeKind::Inside,
            );
        }
        painter.galley(
            egui::pos2(rect.left() + 16.0, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );

        if response.clicked() {
            clicked = Some(index);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        x += tab_width + 2.0;
    }

    clicked
}

/// Bootstrap `card` with a header.
pub fn card<R>(ui: &mut Ui, p: &Palette, header: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    let outer = Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.card_border))
        .corner_radius(CornerRadius::same(RADIUS));

    outer
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.set_width(ui.available_width());

            Frame::new()
                .fill(p.card_header)
                .inner_margin(Margin::symmetric(20, 5))
                .corner_radius(CornerRadius {
                    nw: RADIUS,
                    ne: RADIUS,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(header).color(p.text));
                });
            ui.painter().hline(
                ui.min_rect().x_range(),
                ui.min_rect().bottom(),
                Stroke::new(1.0, p.card_border),
            );

            Frame::new()
                .inner_margin(Margin {
                    left: 20,
                    right: 20,
                    top: 12,
                    bottom: 12,
                })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 6.0;
                    body(ui)
                })
                .inner
        })
        .inner
}

/// A `card` that only has a body.
pub fn card_body<R>(ui: &mut Ui, p: &Palette, body: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.card_border))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui)
        })
        .inner
}

/// A button the size of the monitor tables' buttons (100 x 23).
pub fn small_button(ui: &mut Ui, text: &str) -> Response {
    ui.add_sized(Vec2::new(100.0, 23.0), egui::Button::new(text))
}

pub fn text_input(ui: &mut Ui, p: &Palette, text: &mut String, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .desired_width(width)
            .text_color(p.input_text)
            .background_color(p.input_bg)
            .margin(Margin::symmetric(8, 5)),
    )
}

/// Sidebar entry. Mirrors `.nav-side-menu li`: 35 px high, 3 px left border that lights up on
/// hover, a darker 1 px bottom line.
pub fn sidebar_item(ui: &mut Ui, p: &Palette, icon: &str, label: &str) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let painter = ui.painter_at(rect);

    if response.hovered() {
        painter.rect_filled(rect, 0.0, p.sidebar_hover);
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
            0.0,
            p.sidebar_text,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, p.sidebar_border),
    );

    let font = FontId::new(12.0, FontFamily::Proportional);
    painter.text(
        egui::pos2(rect.left() + 3.0 + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        icon,
        font.clone(),
        p.sidebar_text,
    );
    painter.text(
        egui::pos2(rect.left() + 3.0 + 10.0 + 20.0 + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        p.sidebar_text,
    );

    response
}
