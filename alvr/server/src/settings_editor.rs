//! Generic egui editor for the session settings tree. It walks the `SchemaNode` tree produced by
//! the settings derive macro and edits the matching `serde_json::Value` in place.

use eframe::egui::{self, DragValue, Ui};
use serde_json::{json, Map, Value};
use settings_schema::{EntryData, NumericGuiType, SchemaNode};

/// Builds the JSON value that a settings field holds when it has its default value.
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
        SchemaNode::Choice { default, variants } => variant_value(default, variants),
        SchemaNode::Optional {
            default_set,
            content,
        } => {
            if *default_set {
                default_value(content)
            } else {
                Value::Null
            }
        }
        SchemaNode::Switch {
            default_enabled,
            content,
            ..
        } => {
            if *default_enabled {
                json!({ "state": "Enabled", "content": default_value(content) })
            } else {
                json!({ "state": "Disabled" })
            }
        }
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

fn variant_value(name: &str, variants: &[(String, Option<EntryData>)]) -> Value {
    match variants.iter().find(|(variant, _)| variant == name) {
        Some((_, Some(data))) => json!({ name: default_value(&data.content) }),
        _ => Value::String(name.to_owned()),
    }
}

fn selected_variant(value: &Value) -> Option<&str> {
    match value {
        Value::String(name) => Some(name),
        Value::Object(map) => map.keys().next().map(String::as_str),
        _ => None,
    }
}

/// Draws `node` bound to `value`. Returns true if the value was modified.
pub fn node_ui(
    ui: &mut Ui,
    id: &str,
    label: &str,
    node: &SchemaNode,
    value: &mut Value,
    show_advanced: bool,
) -> bool {
    let mut changed = false;

    match node {
        SchemaNode::Section { entries } => {
            let Some(fields) = value.as_object_mut() else {
                ui.colored_label(egui::Color32::RED, format!("{label}: unexpected value"));
                return false;
            };
            egui::CollapsingHeader::new(label)
                .id_salt(id)
                .show(ui, |ui| {
                    for (name, data) in entries {
                        let Some(data) = data else { continue };
                        if data.advanced && !show_advanced {
                            continue;
                        }
                        if let Some(field) = fields.get_mut(name) {
                            changed |= node_ui(
                                ui,
                                &format!("{id}/{name}"),
                                name,
                                &data.content,
                                field,
                                show_advanced,
                            );
                        }
                    }
                });
        }
        SchemaNode::Choice { variants, .. } => {
            let current = selected_variant(value)
                .map(str::to_owned)
                .unwrap_or_default();
            let mut selected = current.clone();

            ui.horizontal(|ui| {
                ui.label(label);
                egui::ComboBox::from_id_salt(id)
                    .selected_text(&selected)
                    .show_ui(ui, |ui| {
                        for (name, _) in variants {
                            ui.selectable_value(&mut selected, name.clone(), name);
                        }
                    });
            });

            if selected != current {
                *value = variant_value(&selected, variants);
                changed = true;
            }

            let content = variants
                .iter()
                .find(|(name, _)| *name == selected)
                .and_then(|(_, data)| data.as_ref());
            if let (Some(data), Some(inner)) = (content, value.get_mut(&selected)) {
                ui.indent(id, |ui| {
                    changed |= node_ui(
                        ui,
                        &format!("{id}/{selected}"),
                        &selected,
                        &data.content,
                        inner,
                        show_advanced,
                    );
                });
            }
        }
        SchemaNode::Optional { content, .. } => {
            let mut is_set = !value.is_null();
            if ui.checkbox(&mut is_set, label).changed() {
                *value = if is_set {
                    default_value(content)
                } else {
                    Value::Null
                };
                changed = true;
            }
            if is_set {
                ui.indent(id, |ui| {
                    changed |= node_ui(ui, id, "value", content, value, show_advanced);
                });
            }
        }
        SchemaNode::Switch {
            content,
            content_advanced,
            ..
        } => {
            let mut enabled = value.get("state").and_then(Value::as_str) == Some("Enabled");
            if ui.checkbox(&mut enabled, label).changed() {
                *value = if enabled {
                    json!({ "state": "Enabled", "content": default_value(content) })
                } else {
                    json!({ "state": "Disabled" })
                };
                changed = true;
            }
            if enabled && (!content_advanced || show_advanced) {
                if let Some(inner) = value.get_mut("content") {
                    ui.indent(id, |ui| {
                        changed |= node_ui(ui, id, "content", content, inner, show_advanced);
                    });
                }
            }
        }
        SchemaNode::Boolean { .. } => {
            let mut flag = value.as_bool().unwrap_or_default();
            if ui.checkbox(&mut flag, label).changed() {
                *value = Value::Bool(flag);
                changed = true;
            }
        }
        SchemaNode::Integer {
            min,
            max,
            step,
            gui,
            ..
        } => {
            let mut number = value.as_i64().unwrap_or_default();
            let min = min.map(|v| v as i64);
            let max = max.map(|v| v as i64);
            let response = ui
                .horizontal(|ui| {
                    ui.label(label);
                    match (gui, min, max) {
                        (Some(NumericGuiType::Slider), Some(min), Some(max)) => {
                            let mut slider = egui::Slider::new(&mut number, min..=max);
                            if let Some(step) = step {
                                slider = slider.step_by(*step as f64);
                            }
                            ui.add(slider)
                        }
                        _ => {
                            let mut drag = DragValue::new(&mut number);
                            if let (Some(min), Some(max)) = (min, max) {
                                drag = drag.range(min..=max);
                            }
                            if let Some(step) = step {
                                drag = drag.speed(*step as f64);
                            }
                            ui.add(drag)
                        }
                    }
                })
                .inner;
            if response.changed() {
                *value = json!(number);
                changed = true;
            }
        }
        SchemaNode::Float {
            min,
            max,
            step,
            gui,
            ..
        } => {
            let mut number = value.as_f64().unwrap_or_default();
            let response = ui
                .horizontal(|ui| {
                    ui.label(label);
                    match (gui, min, max) {
                        (Some(NumericGuiType::Slider), Some(min), Some(max)) => {
                            let mut slider = egui::Slider::new(&mut number, *min..=*max);
                            if let Some(step) = step {
                                slider = slider.step_by(*step);
                            }
                            ui.add(slider)
                        }
                        _ => {
                            let mut drag = DragValue::new(&mut number);
                            if let (Some(min), Some(max)) = (min, max) {
                                drag = drag.range(*min..=*max);
                            }
                            if let Some(step) = step {
                                drag = drag.speed(*step);
                            }
                            ui.add(drag)
                        }
                    }
                })
                .inner;
            if response.changed() {
                *value = json!(number);
                changed = true;
            }
        }
        SchemaNode::Text { .. } => {
            let mut text = value.as_str().unwrap_or_default().to_owned();
            let response = ui
                .horizontal(|ui| {
                    ui.label(label);
                    ui.text_edit_singleline(&mut text)
                })
                .inner;
            if response.changed() {
                *value = Value::String(text);
                changed = true;
            }
        }
        SchemaNode::Array(elements) => {
            if let Some(items) = value.as_array_mut() {
                egui::CollapsingHeader::new(label)
                    .id_salt(id)
                    .show(ui, |ui| {
                        for (index, (element, item)) in
                            elements.iter().zip(items.iter_mut()).enumerate()
                        {
                            changed |= node_ui(
                                ui,
                                &format!("{id}/{index}"),
                                &index.to_string(),
                                element,
                                item,
                                show_advanced,
                            );
                        }
                    });
            }
        }
        SchemaNode::Vector {
            default_element, ..
        } => {
            if let Some(items) = value.as_array_mut() {
                egui::CollapsingHeader::new(label)
                    .id_salt(id)
                    .show(ui, |ui| {
                        let mut remove = None;
                        for (index, item) in items.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.small_button("🗑").clicked() {
                                    remove = Some(index);
                                }
                                changed |= node_ui(
                                    ui,
                                    &format!("{id}/{index}"),
                                    &index.to_string(),
                                    default_element,
                                    item,
                                    show_advanced,
                                );
                            });
                        }
                        if let Some(index) = remove {
                            items.remove(index);
                            changed = true;
                        }
                        if ui.button("Add").clicked() {
                            items.push(default_value(default_element));
                            changed = true;
                        }
                    });
            }
        }
        SchemaNode::Dictionary {
            default_key,
            default_value: default_entry,
            ..
        } => {
            if let Some(entries) = value.as_array_mut() {
                egui::CollapsingHeader::new(label)
                    .id_salt(id)
                    .show(ui, |ui| {
                        let mut remove = None;
                        for (index, entry) in entries.iter_mut().enumerate() {
                            let Some(pair) = entry.as_array_mut() else {
                                continue;
                            };
                            if pair.len() != 2 {
                                continue;
                            }
                            let (key, content) = pair.split_at_mut(1);
                            ui.horizontal(|ui| {
                                if ui.small_button("🗑").clicked() {
                                    remove = Some(index);
                                }
                                let mut text = key[0].as_str().unwrap_or_default().to_owned();
                                if ui.text_edit_singleline(&mut text).changed() {
                                    key[0] = Value::String(text);
                                    changed = true;
                                }
                            });
                            ui.indent(format!("{id}/{index}"), |ui| {
                                changed |= node_ui(
                                    ui,
                                    &format!("{id}/{index}"),
                                    "value",
                                    default_entry,
                                    &mut content[0],
                                    show_advanced,
                                );
                            });
                        }
                        if let Some(index) = remove {
                            entries.remove(index);
                            changed = true;
                        }
                        if ui.button("Add").clicked() {
                            entries.push(json!([default_key, default_value(default_entry)]));
                            changed = true;
                        }
                    });
            }
        }
    }

    changed
}
