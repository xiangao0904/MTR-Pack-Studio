use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrainDefinition {
    pub id: String,
    pub revision: u64,
    pub export_id: String,
    pub name: String,
    #[serde(default)] pub description: String,
    #[serde(default = "default_color")] pub color: String,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub mtr3_base_train_type: String,
    #[serde(default)] pub carriages: Vec<CarriageDefinition>,
    #[serde(default)] pub preview_consist: Vec<PreviewCarriage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CarriageDefinition {
    pub id: String,
    pub export_id: String,
    pub name: String,
    #[serde(default)] pub thumbnail_hash: Option<String>,
    #[serde(default = "default_length")] pub length: f32,
    #[serde(default = "default_width")] pub width: f32,
    #[serde(default = "default_bogie_one")] pub bogie_1_position: f32,
    #[serde(default = "default_bogie_two")] pub bogie_2_position: f32,
    #[serde(default)] pub coupling_padding_1: f32,
    #[serde(default)] pub coupling_padding_2: f32,
    #[serde(default)] pub end_1: EndConfiguration,
    #[serde(default)] pub end_2: EndConfiguration,
    #[serde(default)] pub placement: CarPlacementRule,
    #[serde(default)] pub body_models: Vec<ModelLayer>,
    #[serde(default)] pub bogie_1_models: Vec<ModelLayer>,
    #[serde(default)] pub bogie_2_models: Vec<ModelLayer>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EndConfiguration {
    #[serde(default)] pub gangway: bool,
    #[serde(default)] pub barrier: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelLayer {
    pub id: String,
    pub name: String,
    pub asset_id: String,
    #[serde(default)] pub flip_texture_v: bool,
    #[serde(default = "default_true")] pub visible: bool,
    #[serde(default)] pub material_bindings: Vec<MaterialBinding>,
    #[serde(default)] pub part_rules: BTreeMap<String, CarPlacementRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MaterialBinding {
    pub material_id: String,
    #[serde(default)] pub texture_asset_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CarPlacementRule {
    #[serde(default)] pub preset: PlacementPreset,
    #[serde(default)] pub every: Option<u32>,
    #[serde(default)] pub offset: i32,
    #[serde(default)] pub whitelist: String,
    #[serde(default)] pub blacklist: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PlacementPreset { #[default] All, First, Last, Odd, Even, Every, Custom }

impl CarPlacementRule {
    pub fn expressions(&self) -> Result<(String, String), String> {
        Ok(match self.preset {
            PlacementPreset::All => (String::new(), String::new()),
            PlacementPreset::First => ("1".into(), String::new()),
            PlacementPreset::Last => ("-1".into(), String::new()),
            PlacementPreset::Odd => ("%2+1".into(), String::new()),
            PlacementPreset::Even => ("%2".into(), String::new()),
            PlacementPreset::Every => {
                let every = self.every.filter(|value| *value > 0).ok_or_else(|| "Every placement requires a positive interval.".to_string())?;
                let offset = if self.offset == 0 { String::new() } else { format!("{:+}", self.offset) };
                (format!("%{every}{offset}"), String::new())
            }
            PlacementPreset::Custom => {
                validate_filter(&self.whitelist)?;
                validate_filter(&self.blacklist)?;
                (self.whitelist.trim().into(), self.blacklist.trim().into())
            }
        })
    }
}

fn validate_filter(filter: &str) -> Result<(), String> {
    for token in filter.split(',').map(str::trim).filter(|token| !token.is_empty()) {
        let valid = if let Some(rest) = token.strip_prefix('%') {
            let split_at = rest.char_indices().skip(1).find(|(_, c)| matches!(c, '+' | '-')).map(|(index, _)| index);
            let (divisor, offset) = split_at.map_or((rest, ""), |index| (&rest[..index], &rest[index..]));
            divisor.parse::<u32>().is_ok_and(|value| value > 0) && (offset.is_empty() || offset.parse::<i32>().is_ok())
        } else { token.parse::<i32>().is_ok_and(|value| value != 0) };
        if !valid { return Err(format!("Invalid car placement expression: {token}")); }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewCarriage { pub carriage_id: String, #[serde(default)] pub reversed: bool }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetDefinition {
    pub id: String,
    pub name: String,
    pub source_format: ModelFormat,
    pub source_hash: String,
    pub document_hash: String,
    pub preview_hash: String,
    #[serde(default)] pub dependencies: Vec<AssetDependency>,
    #[serde(default)] pub parts: Vec<ModelPartSummary>,
    #[serde(default)] pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetDependency { pub name: String, pub hash: String, pub media_type: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelPartSummary { pub id: String, pub name: String, pub triangle_count: usize }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat { Obj, Fbx, Mqo }

impl TrainDefinition {
    pub fn new(name: &str, export_id: &str) -> Self {
        let carriage = CarriageDefinition::new("Carriage", "carriage");
        Self {
            id: Uuid::new_v4().to_string(), revision: 1, export_id: export_id.into(), name: name.into(),
            description: String::new(), color: default_color(), tags: Vec::new(), mtr3_base_train_type: String::new(),
            preview_consist: vec![PreviewCarriage { carriage_id: carriage.id.clone(), reversed: false }], carriages: vec![carriage],
        }
    }
}

impl CarriageDefinition {
    pub fn new(name: &str, export_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(), export_id: export_id.into(), name: name.into(), thumbnail_hash: None, length: default_length(), width: default_width(),
            bogie_1_position: default_bogie_one(), bogie_2_position: default_bogie_two(), coupling_padding_1: 0.0, coupling_padding_2: 0.0,
            end_1: EndConfiguration::default(), end_2: EndConfiguration::default(), placement: CarPlacementRule::default(),
            body_models: Vec::new(), bogie_1_models: Vec::new(), bogie_2_models: Vec::new(),
        }
    }
}

pub fn slugify(value: &str, fallback: &str) -> String {
    let mut result = String::new(); let mut separator = false;
    for character in value.to_lowercase().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') { result.push(character); separator = false; }
        else if !separator && !result.is_empty() { result.push('_'); separator = true; }
    }
    while result.ends_with('_') { result.pop(); }
    if result.is_empty() { fallback.into() } else { result }
}

fn default_color() -> String { "FFFFFF".into() }
fn default_length() -> f32 { 20.0 }
fn default_width() -> f32 { 3.0 }
fn default_bogie_one() -> f32 { 7.0 }
fn default_bogie_two() -> f32 { -7.0 }
fn default_true() -> bool { true }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn placement_presets_compile_to_mtr_filters() {
        assert_eq!(CarPlacementRule { preset: PlacementPreset::Last, ..Default::default() }.expressions().unwrap().0, "-1");
        assert_eq!(CarPlacementRule { preset: PlacementPreset::Every, every: Some(3), offset: 1, ..Default::default() }.expressions().unwrap().0, "%3+1");
        assert!(CarPlacementRule { preset: PlacementPreset::Custom, whitelist: "%0".into(), ..Default::default() }.expressions().is_err());
    }
    #[test] fn slugs_are_valid_resource_identifiers() { assert_eq!(slugify("Urban Rail / 2026", "train"), "urban_rail_2026"); }
}
