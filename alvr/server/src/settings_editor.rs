//! Settings page of the dashboard. It walks the `SchemaNode` tree produced by the settings derive
//! macro and edits the session settings JSON in place, laying the controls out like the former web
//! dashboard did: one tab per top-level section, accordions for nested sections, button groups
//! for choices, and a help / reset pair on every row.
//!
//! The JSON being edited is the serialization of the generated `*Default` structs, not of the
//! final settings types:
//! - switch: `{ "enabled": bool, "content": .. }`
//! - optional: `{ "set": bool, "content": .. }`
//! - choice: `{ "variant": "name", "<variant>": <content>, .. }` (one key per variant with data)
//! - vector: `{ "element": .., "content": [..] }`
//! - dictionary: `{ "key": .., "value": .., "content": [[key, value], ..] }`

use crate::theme::{self, Palette};
use alvr_common::lazy_static;
use eframe::egui::{
    self, collapsing_header::CollapsingState, Align, Layout, RichText, Sense, Stroke, Ui, Vec2,
};
use serde_json::{json, Map, Value};
use settings_schema::{NumericGuiType, SchemaNode};
use std::collections::HashMap;

lazy_static! {
    // Names and descriptions of the settings, taken from the web dashboard's English strings.
    static ref NAMES: HashMap<String, String> =
        serde_json::from_str(include_str!("settings_names.json")).unwrap_or_default();
}

fn name_of(key: &str, fallback: &str) -> String {
    NAMES
        .get(&format!("{key}.name"))
        .cloned()
        .unwrap_or_else(|| fallback.replace("-choice-", ""))
}

fn description_of(key: &str) -> Option<&'static str> {
    NAMES.get(&format!("{key}.description")).map(String::as_str)
}

/// Builds the JSON value that a settings node holds when it has its default value.
pub fn default_value(node: &SchemaNode) -> Value {
    match node {
        SchemaNode::Section { entries } => Value::Object(
            entries
                .iter()
                .filter_map(|(name, data)| {
                    data.as_ref()
                        .map(|data| (name.clone(), default_value(&data.content)))
                })
                .collect::<Map<_, _>>(),
        ),
        SchemaNode::Choice { default, variants } => {
            let mut map = Map::new();
            map.insert("variant".into(), Value::String(default.clone()));
            for (name, data) in variants {
                if let Some(data) = data {
                    map.insert(name.clone(), default_value(&data.content));
                }
            }
            Value::Object(map)
        }
        SchemaNode::Optional {
            default_set,
            content,
        } => json!({ "set": default_set, "content": default_value(content) }),
        SchemaNode::Switch {
            default_enabled,
            content,
            ..
        } => json!({ "enabled": default_enabled, "content": default_value(content) }),
        SchemaNode::Boolean { default } => Value::Bool(*default),
        SchemaNode::Integer { default, .. } => json!(*default as i64),
        SchemaNode::Float { default, .. } => json!(*default),
        SchemaNode::Text { default } => Value::String(default.clone()),
        SchemaNode::Array(elements) => Value::Array(elements.iter().map(default_value).collect()),
        SchemaNode::Vector { default, .. } | SchemaNode::Dictionary { default, .. } => {
            default.clone()
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Parent {
    Section,
    Switch,
    Optional,
    Array,
    Choice,
}

pub struct SettingsEditor {
    pub show_advanced: bool,
    tab: usize,
}

struct Pass<'a> {
    p: &'a Palette,
    show_advanced: bool,
    changed: bool,
}

impl SettingsEditor {
    pub fn new() -> Self {
        Self {
            show_advanced: false,
            tab: 0,
        }
    }

    #[cfg(test)]
    fn set_tab(&mut self, tab: usize) {
        self.tab = tab;
    }

    /// Draws the settings page. Returns true when the user modified a value.
    pub fn ui(&mut self, ui: &mut Ui, p: &Palette, schema: &SchemaNode, draft: &mut Value) -> bool {
        let SchemaNode::Section { entries } = schema else {
            return false;
        };
        let Some(root) = draft.as_object_mut() else {
            ui.label("Settings are not available.");
            return false;
        };

        let tabs = entries
            .iter()
            .filter_map(|(name, data)| data.as_ref().map(|data| (name, data)))
            .filter(|(_, data)| !data.advanced || self.show_advanced)
            .collect::<Vec<_>>();
        if tabs.is_empty() {
            return false;
        }
        self.tab = self.tab.min(tabs.len() - 1);

        let labels = tabs
            .iter()
            .map(|(name, _)| name_of(&format!("_root_{name}_tab"), name))
            .collect::<Vec<_>>();
        let label_refs = labels.iter().map(String::as_str).collect::<Vec<_>>();

        if let Some(clicked) = theme::nav_tabs(ui, p, &label_refs, self.tab) {
            self.tab = clicked;
        }

        let mut pass = Pass {
            p,
            show_advanced: self.show_advanced,
            changed: false,
        };

        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(30.0);
                        let (state, color) = if self.show_advanced {
                            ("\u{25C9}", p.link)
                        } else {
                            ("\u{25CB}", p.muted)
                        };
                        let toggle = ui.add(
                            egui::Label::new(
                                RichText::new(format!("Show advanced options  {state}"))
                                    .color(color),
                            )
                            .sense(Sense::click()),
                        );
                        if toggle.clicked() {
                            self.show_advanced = !self.show_advanced;
                        }
                        if toggle.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });
                });
                ui.add_space(10.0);

                ui.indent("settings_content", |ui| {
                    ui.set_max_width(ui.available_width() - 30.0);
                    let (name, data) = tabs[self.tab];
                    if let Some(value) = root.get_mut(name.as_str()) {
                        pass.fill(
                            ui,
                            &data.content,
                            name,
                            1,
                            "_root",
                            Parent::Section,
                            value,
                            false,
                        );
                    }
                });
                ui.add_space(20.0);
            });

        pass.changed
    }
}

impl Pass<'_> {
    #[allow(clippy::too_many_arguments)]
    fn fill(
        &mut self,
        ui: &mut Ui,
        node: &SchemaNode,
        name: &str,
        level: u32,
        path: &str,
        parent: Parent,
        value: &mut Value,
        advanced: bool,
    ) {
        if advanced && !self.show_advanced {
            return;
        }

        // optional and switch values are named "<name>_content"
        let name = if matches!(parent, Parent::Switch | Parent::Optional) {
            format!("{name}_content")
        } else {
            name.to_owned()
        };
        let key = format!("{path}_{name}");
        let p = *self.p;

        match node {
            SchemaNode::Section { entries } => {
                let reset_value = default_value(node);
                let mut body = |this: &mut Self, ui: &mut Ui| {
                    let Some(fields) = value.as_object_mut() else {
                        return;
                    };
                    for (entry_name, data) in entries {
                        let Some(data) = data else { continue };
                        if let Some(entry_value) = fields.get_mut(entry_name) {
                            this.fill(
                                ui,
                                &data.content,
                                entry_name,
                                level + 1,
                                &key,
                                Parent::Section,
                                entry_value,
                                data.advanced,
                            );
                        }
                    }
                };

                if level > 1 && !matches!(parent, Parent::Switch | Parent::Optional) {
                    let reset = self.accordion(
                        ui,
                        &key,
                        &name_of(&key, &name),
                        description_of(&key),
                        None,
                        true,
                        &mut body,
                    );
                    if reset {
                        // body borrowed `value`; reset after it is released
                        self.changed = true;
                        pending_reset(value, reset_value);
                    }
                } else {
                    body(self, ui);
                }
            }
            SchemaNode::Switch {
                content,
                content_advanced,
                default_enabled,
            } => {
                let mut enabled = value
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or_default();
                let before = enabled;
                let mut reset = false;

                if let Some(content_value) = value.get_mut("content") {
                    let mut checkbox = enabled;
                    reset = self.accordion(
                        ui,
                        &key,
                        &name_of(&key, &name),
                        description_of(&key),
                        Some(&mut checkbox),
                        enabled,
                        |this, ui| {
                            this.fill(
                                ui,
                                content,
                                &name_without_content(&name),
                                level + 1,
                                path,
                                Parent::Switch,
                                content_value,
                                *content_advanced,
                            );
                        },
                    );
                    enabled = checkbox;
                }

                if reset {
                    enabled = *default_enabled;
                }
                if enabled != before {
                    if let Some(map) = value.as_object_mut() {
                        map.insert("enabled".into(), Value::Bool(enabled));
                    }
                    self.changed = true;
                }
            }
            SchemaNode::Optional {
                content,
                default_set,
            } => {
                let mut is_set = value
                    .get("set")
                    .and_then(Value::as_bool)
                    .unwrap_or_default();
                let before = is_set;
                let title = name_of(&key, &name);
                let help = description_of(&key);
                let mut reset = false;

                if let Some(content_value) = value.get_mut("content") {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        // Set / Default button group
                        let fill = |on: bool| if on { p.primary_active } else { p.primary };
                        if ui
                            .add(egui::Button::new("Set").fill(fill(is_set)).small())
                            .clicked()
                        {
                            is_set = true;
                        }
                        if ui
                            .add(egui::Button::new("Default").fill(fill(!is_set)).small())
                            .clicked()
                        {
                            is_set = false;
                        }
                        ui.label(RichText::new(&title).color(p.link));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            reset = help_reset(ui, &p, help);
                        });
                    });
                    if is_set {
                        ui.indent(&key, |ui| {
                            self.fill(
                                ui,
                                content,
                                &name_without_content(&name),
                                level + 1,
                                path,
                                Parent::Optional,
                                content_value,
                                false,
                            );
                        });
                    }
                }

                if reset {
                    is_set = *default_set;
                }
                if is_set != before {
                    if let Some(map) = value.as_object_mut() {
                        map.insert("set".into(), Value::Bool(is_set));
                    }
                    self.changed = true;
                }
            }
            SchemaNode::Choice { default, variants } => {
                let choice_key = format!("{key}-choice-");
                let current = value
                    .get("variant")
                    .and_then(Value::as_str)
                    .unwrap_or(default)
                    .to_owned();
                let mut selected = current.clone();
                let help = description_of(&choice_key);

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name_of(&choice_key, &name)).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if help_reset(ui, &p, help) {
                            selected = default.clone();
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (variant, _) in variants {
                        let label = name_of(&format!("{key}_{variant}-choice-"), variant);
                        let fill = if *variant == selected {
                            p.primary_active
                        } else {
                            p.primary
                        };
                        if ui.add(egui::Button::new(label).fill(fill)).clicked() {
                            selected = variant.clone();
                        }
                    }
                });

                if selected != current {
                    if let Some(map) = value.as_object_mut() {
                        map.insert("variant".into(), Value::String(selected.clone()));
                    }
                    self.changed = true;
                }

                let content = variants
                    .iter()
                    .find(|(variant, _)| *variant == selected)
                    .and_then(|(_, data)| data.as_ref());
                if let (Some(data), Some(variant_value)) =
                    (content, value.get_mut(selected.as_str()))
                {
                    ui.indent(&key, |ui| {
                        self.fill(
                            ui,
                            &data.content,
                            &selected,
                            level + 1,
                            &key,
                            Parent::Choice,
                            variant_value,
                            data.advanced,
                        );
                    });
                }
            }
            SchemaNode::Array(elements) => {
                let items = value.as_array_mut();
                self.accordion(
                    ui,
                    &key,
                    &name_of(&key, &name),
                    description_of(&key),
                    None,
                    true,
                    |this, ui| {
                        let Some(items) = items else { return };
                        for (index, (element, item)) in
                            elements.iter().zip(items.iter_mut()).enumerate()
                        {
                            this.fill(
                                ui,
                                element,
                                &format!("{name}_{index}"),
                                level + 1,
                                path,
                                Parent::Array,
                                item,
                                false,
                            );
                        }
                    },
                );
            }
            SchemaNode::Boolean { .. } => {
                let mut flag = value.as_bool().unwrap_or_default();
                let label = name_of(&key, &name);
                let help = description_of(&key);
                let mut reset = false;
                ui.horizontal(|ui| {
                    ui.checkbox(&mut flag, RichText::new(label).color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        reset = help_reset(ui, &p, help);
                    });
                });
                if reset {
                    *value = default_value(node);
                    self.changed = true;
                } else if value.as_bool() != Some(flag) {
                    *value = Value::Bool(flag);
                    self.changed = true;
                }
            }
            SchemaNode::Integer {
                min,
                max,
                step,
                gui,
                ..
            } => {
                let label = numeric_label(
                    &name_of(&key, &name),
                    min.map(|v| v as f64),
                    max.map(|v| v as f64),
                );
                let mut number = value.as_i64().unwrap_or_default();
                let before = number;
                let (min, max, step) = (
                    min.map(|v| v as i64),
                    max.map(|v| v as i64),
                    step.map(|v| v as i64),
                );
                let kind = gui_kind(gui, min.is_some() && max.is_some(), step.is_some());
                let reset = numeric_row(ui, &p, &label, description_of(&key), kind, |ui| {
                    numeric_widget(ui, &p, kind, &mut number, min, max, step);
                });
                if reset {
                    *value = default_value(node);
                    self.changed = true;
                } else if number != before {
                    *value = json!(number);
                    self.changed = true;
                }
            }
            SchemaNode::Float {
                min,
                max,
                step,
                gui,
                ..
            } => {
                let label = numeric_label(&name_of(&key, &name), *min, *max);
                let mut number = value.as_f64().unwrap_or_default();
                let before = number;
                let kind = gui_kind(gui, min.is_some() && max.is_some(), step.is_some());
                let reset = numeric_row(ui, &p, &label, description_of(&key), kind, |ui| {
                    numeric_widget(ui, &p, kind, &mut number, *min, *max, *step);
                });
                if reset {
                    *value = default_value(node);
                    self.changed = true;
                } else if number != before {
                    *value = json!(number);
                    self.changed = true;
                }
            }
            SchemaNode::Text { .. } => {
                let label = name_of(&key, &name);
                let mut text = value.as_str().unwrap_or_default().to_owned();
                let before = text.clone();
                let help = description_of(&key);
                let mut reset = false;
                ui.horizontal(|ui| {
                    label_column(ui, &p, &label);
                    theme::text_input(ui, &p, &mut text, 100.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        reset = help_reset(ui, &p, help);
                    });
                });
                if reset {
                    *value = default_value(node);
                    self.changed = true;
                } else if text != before {
                    *value = Value::String(text);
                    self.changed = true;
                }
            }
            SchemaNode::Vector {
                default_element, ..
            } => {
                let mut changed = false;
                let element_default = default_value(default_element);
                let items = value.get_mut("content").and_then(Value::as_array_mut);
                self.accordion(
                    ui,
                    &key,
                    &name_of(&key, &name),
                    description_of(&key),
                    None,
                    true,
                    |this, ui| {
                        let Some(items) = items else { return };
                        let mut remove = None;
                        for (index, item) in items.iter_mut().enumerate() {
                            if ui.small_button("Remove").clicked() {
                                remove = Some(index);
                            }
                            ui.indent(index, |ui| {
                                this.fill(
                                    ui,
                                    default_element,
                                    &format!("{name}_{index}"),
                                    level + 1,
                                    path,
                                    Parent::Array,
                                    item,
                                    false,
                                );
                            });
                        }
                        if let Some(index) = remove {
                            items.remove(index);
                            changed = true;
                        }
                        if ui.small_button("Add").clicked() {
                            items.push(element_default.clone());
                            changed = true;
                        }
                    },
                );
                self.changed |= changed;
            }
            SchemaNode::Dictionary {
                default_key,
                default_value: default_entry,
                ..
            } => {
                let mut changed = false;
                let new_entry = json!([default_key, default_value(default_entry)]);
                let entries = value.get_mut("content").and_then(Value::as_array_mut);
                self.accordion(
                    ui,
                    &key,
                    &name_of(&key, &name),
                    description_of(&key),
                    None,
                    true,
                    |this, ui| {
                        let Some(entries) = entries else { return };
                        let mut remove = None;
                        for (index, entry) in entries.iter_mut().enumerate() {
                            let Some(pair) = entry.as_array_mut() else {
                                continue;
                            };
                            if pair.len() != 2 {
                                continue;
                            }
                            let (entry_key, entry_value) = pair.split_at_mut(1);
                            ui.horizontal(|ui| {
                                if ui.small_button("Remove").clicked() {
                                    remove = Some(index);
                                }
                                let mut text = entry_key[0].as_str().unwrap_or_default().to_owned();
                                if theme::text_input(ui, &p, &mut text, 150.0).changed() {
                                    entry_key[0] = Value::String(text);
                                    changed = true;
                                }
                            });
                            ui.indent(index, |ui| {
                                this.fill(
                                    ui,
                                    default_entry,
                                    &format!("{name}_value"),
                                    level + 1,
                                    path,
                                    Parent::Array,
                                    &mut entry_value[0],
                                    false,
                                );
                            });
                        }
                        if let Some(index) = remove {
                            entries.remove(index);
                            changed = true;
                        }
                        if ui.small_button("Add").clicked() {
                            entries.push(new_entry.clone());
                            changed = true;
                        }
                    },
                );
                self.changed |= changed;
            }
        }
    }

    /// An accordion like the web dashboard's `card-title` + collapsible `card-body`. When
    /// `checkbox` is given it is drawn before the title (switch containers). Returns true if the
    /// reset icon was clicked.
    fn accordion(
        &mut self,
        ui: &mut Ui,
        id: &str,
        title: &str,
        help: Option<&str>,
        checkbox: Option<&mut bool>,
        show_body: bool,
        body: impl FnOnce(&mut Self, &mut Ui),
    ) -> bool {
        let p = *self.p;
        let mut reset = false;
        ui.add_space(4.0);
        let state =
            CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(id), true);
        let header = state.show_header(ui, |ui| {
            if let Some(checkbox) = checkbox {
                ui.checkbox(checkbox, "");
            }
            ui.label(RichText::new(title).color(p.link));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reset = help_reset(ui, &p, help);
            });
        });
        header.body(|ui| {
            if show_body {
                body(self, ui);
            }
        });
        reset
    }
}

fn pending_reset(value: &mut Value, default: Value) {
    *value = default;
}

fn name_without_content(name: &str) -> String {
    name.strip_suffix("_content").unwrap_or(name).to_owned()
}

fn numeric_label(name: &str, min: Option<f64>, max: Option<f64>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("{name} ({min}-{max})"),
        _ => name.to_owned(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum GuiKind {
    Slider,
    UpDown,
    TextBox,
}

fn gui_kind(gui: &Option<NumericGuiType>, has_range: bool, has_step: bool) -> GuiKind {
    match gui {
        Some(NumericGuiType::Slider) => GuiKind::Slider,
        Some(NumericGuiType::UpDown) => GuiKind::UpDown,
        Some(NumericGuiType::TextBox) => GuiKind::TextBox,
        None if has_range && has_step => GuiKind::Slider,
        None if has_range => GuiKind::UpDown,
        None => GuiKind::TextBox,
    }
}

/// `.parameter label:first-of-type { min-width: 50% }`
fn label_column(ui: &mut Ui, p: &Palette, text: &str) {
    let width = (ui.available_width() * 0.5).max(120.0);
    ui.allocate_ui_with_layout(
        Vec2::new(width, 24.0),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.label(RichText::new(text).color(p.text));
        },
    );
}

/// Label, control and help / reset icons of a numeric setting. Returns true if reset was clicked.
fn numeric_row(
    ui: &mut Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    kind: GuiKind,
    widget: impl FnOnce(&mut Ui),
) -> bool {
    let mut reset = false;
    if kind == GuiKind::Slider {
        // label and value on the first line, slider on the second
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{label}:")).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    reset = help_reset(ui, p, help);
                });
            });
            ui.horizontal(|ui| widget(ui));
        });
    } else {
        ui.horizontal(|ui| {
            label_column(ui, p, &format!("{label}:"));
            widget(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                reset = help_reset(ui, p, help);
            });
        });
    }
    reset
}

trait Numeric:
    egui::emath::Numeric + std::fmt::Display + std::str::FromStr + Copy + PartialOrd
{
    fn one() -> Self;
    fn from_step(step: Option<Self>) -> Self {
        step.unwrap_or_else(Self::one)
    }
    fn plus(self, other: Self) -> Self;
    fn minus(self, other: Self) -> Self;
}

impl Numeric for i64 {
    fn one() -> Self {
        1
    }
    fn plus(self, other: Self) -> Self {
        self + other
    }
    fn minus(self, other: Self) -> Self {
        self - other
    }
}

impl Numeric for f64 {
    fn one() -> Self {
        1.0
    }
    fn plus(self, other: Self) -> Self {
        self + other
    }
    fn minus(self, other: Self) -> Self {
        self - other
    }
}

fn numeric_widget<T: Numeric>(
    ui: &mut Ui,
    p: &Palette,
    kind: GuiKind,
    number: &mut T,
    min: Option<T>,
    max: Option<T>,
    step: Option<T>,
) {
    let step_by = T::from_step(step);
    match kind {
        GuiKind::Slider => {
            ui.label(RichText::new(format!("[{number}]")).color(p.text));
            if let (Some(low), Some(high)) = (min, max) {
                ui.spacing_mut().slider_width = (ui.available_width() * 0.5).max(150.0);
                ui.add(
                    egui::Slider::new(number, low..=high)
                        .step_by(step_by.to_f64())
                        .show_value(false),
                );
            }
        }
        GuiKind::UpDown => {
            if ui.small_button("\u{2212}").clicked() {
                *number = number.minus(step_by);
            }
            let mut drag = egui::DragValue::new(number).speed(step_by.to_f64());
            if let (Some(low), Some(high)) = (min, max) {
                drag = drag.range(low..=high);
            }
            ui.add_sized(Vec2::new(100.0, 28.0), drag);
            if ui.small_button("+").clicked() {
                *number = number.plus(step_by);
            }
            if let Some(low) = min {
                if *number < low {
                    *number = low;
                }
            }
            if let Some(high) = max {
                if *number > high {
                    *number = high;
                }
            }
        }
        GuiKind::TextBox => {
            let mut text = number.to_string();
            if theme::text_input(ui, p, &mut text, 100.0).changed() {
                if let Ok(parsed) = text.trim().parse::<T>() {
                    *number = parsed;
                }
            }
        }
    }
}

/// The grey question mark (tooltip) and redo (reset to default) icons floated to the right of a
/// row. Must be called inside a right-to-left layout. Returns true if reset was clicked.
fn help_reset(ui: &mut Ui, p: &Palette, help: Option<&str>) -> bool {
    let reset = ui
        .add(
            egui::Label::new(RichText::new("\u{21BB}").size(17.0).color(p.muted))
                .sense(Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();

    if let Some(help) = help {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
        ui.painter()
            .circle_stroke(rect.center(), 8.0, Stroke::new(1.5, p.muted));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(12.0),
            p.muted,
        );
        response.on_hover_ui(|ui| {
            ui.set_max_width(320.0);
            ui.label(help);
        });
    }

    reset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_runs_on_default_settings() {
        let schema = alvr_session::settings_schema(alvr_session::session_settings_default());
        let mut draft = serde_json::to_value(alvr_session::session_settings_default()).unwrap();

        // the value layout must match what default_value() builds from the schema
        assert_eq!(default_value(&schema), draft);

        let ctx = egui::Context::default();
        let mut editor = SettingsEditor::new();
        for advanced in [false, true] {
            editor.show_advanced = advanced;
            for tab in 0..5 {
                editor.set_tab(tab);
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    let p = Palette::classic();
                    theme::apply(ui.ctx(), &p);
                    editor.ui(ui, &p, &schema, &mut draft);
                });
                output.textures_delta.clear();
            }
        }
    }
}
