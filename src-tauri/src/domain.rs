use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RailDefinition {
    pub id: String,
    pub revision: u64,
    pub export_id: String,
    pub name: String,
    #[serde(default)] pub description: String,
    #[serde(default = "default_repeat_interval")] pub repeat_interval: f32,
    #[serde(default)] pub models: Vec<ModelLayer>,
}

fn default_repeat_interval() -> f32 { 0.6 }

impl RailDefinition {
    pub fn new(name: &str, export_id: &str) -> Self {
        Self { id: Uuid::new_v4().to_string(), revision: 1, export_id: export_id.into(), name: name.into(), description: String::new(), repeat_interval: default_repeat_interval(), models: Vec::new() }
    }
}

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
    #[serde(default)] pub thumbnail_model_signature: Option<String>,
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
    #[serde(default)] pub hidden_parts: Vec<String>,
    #[serde(default)] pub transform: ModelTransform,
    #[serde(default)] pub part_transforms: BTreeMap<String, ModelTransform>,
    #[serde(default)] pub render_stage: RenderStage,
    #[serde(default)] pub part_render_stages: BTreeMap<String, RenderStage>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RenderStage { #[default] Exterior, Interior, InteriorTranslucent, Light, AlwaysOnLight }

impl RenderStage {
    pub fn mtr4(self) -> &'static str { match self { Self::Exterior => "EXTERIOR", Self::Interior => "INTERIOR", Self::InteriorTranslucent => "INTERIOR_TRANSLUCENT", Self::Light => "LIGHT", Self::AlwaysOnLight => "ALWAYS_ON_LIGHT" } }
    pub fn mtr3(self) -> &'static str { match self { Self::Exterior => "exterior", Self::Interior => "interior", Self::InteriorTranslucent => "interior_translucent", Self::Light => "lights", Self::AlwaysOnLight => "always_on_lights" } }
}

impl ModelLayer {
    pub fn render_stage_for(&self, part_id: &str) -> RenderStage { self.part_render_stages.get(part_id).copied().unwrap_or(self.render_stage) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelTransform {
    #[serde(default)] pub translation: [f32; 3],
    #[serde(default)] pub rotation: [f32; 3],
    #[serde(default = "identity_scale")] pub scale: [f32; 3],
}

fn identity_scale() -> [f32; 3] { [1.0; 3] }
impl Default for ModelTransform {
    fn default() -> Self { Self { translation: [0.0; 3], rotation: [0.0; 3], scale: identity_scale() } }
}
impl ModelTransform {
    pub fn validate(&self) -> Result<(), String> {
        if self.translation.iter().chain(&self.rotation).chain(&self.scale).any(|v| !v.is_finite()) || self.scale.iter().any(|v| *v <= 0.0) {
            return Err("Model transforms require finite values and positive scales.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MaterialBinding {
    pub material_id: String,
    #[serde(default)] pub texture_asset_id: Option<String>,
    #[serde(default)] pub properties: crate::material::MaterialProperties,
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
            PlacementPreset::First => ("1".into(), "%1".into()),
            PlacementPreset::Last => ("-1".into(), "%1".into()),
            PlacementPreset::Odd => ("%2+1".into(), "%1".into()),
            PlacementPreset::Even => ("%2".into(), "%1".into()),
            PlacementPreset::Every => {
                let every = self.every.filter(|value| *value > 0 && *value <= i32::MAX as u32).ok_or_else(|| "Every placement requires a positive interval.".to_string())?;
                let additional = (-(self.offset as i64)).rem_euclid(every as i64);
                let offset = if additional == 0 { String::new() } else { format!("+{additional}") };
                (format!("%{every}{offset}"), "%1".into())
            }
            PlacementPreset::Custom => {
                (normalize_filter(&self.whitelist)?, normalize_filter(&self.blacklist)?)
            }
        })
    }
}

// MTR uses match strengths: exact position (3), periodic match (2), no match (0).
// A part is hidden only if the blacklist strength is greater; equal matches remain visible.
fn normalize_filter(filter: &str) -> Result<String, String> {
    let mut normalized = Vec::new();
    for token in filter.split(',').map(str::trim).filter(|token| !token.is_empty()) {
        let error = || format!("Invalid car placement expression: {token}");
        if let Some(rest) = token.strip_prefix('%') {
            let split_at = rest.char_indices().skip(1).find(|(_, c)| matches!(c, '+' | '-')).map(|(index, _)| index);
            let (divisor, suffix) = split_at.map_or((rest, ""), |index| (&rest[..index], &rest[index..]));
            let every = divisor.parse::<i32>().ok().filter(|n| *n > 0).ok_or_else(error)?;
            if suffix == "+" || suffix == "-" { return Err(error()); }
            let suffix = suffix.strip_prefix('+').unwrap_or(suffix);
            let additional = if suffix.is_empty() { 0 } else { suffix.parse::<i32>().map_err(|_|error())? };
            // Native parser splits on '+', so a negative offset is emitted as '+-N'.
            normalized.push(if additional==0 { format!("%{every}") } else { format!("%{every}+{additional}") });
        } else {
            let position=token.parse::<i32>().ok().filter(|n|*n!=0).ok_or_else(error)?;
            normalized.push(position.to_string());
        }
    }
    Ok(normalized.join(","))
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
    #[serde(default)] pub legacy_uv_correction: bool,
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
            id: Uuid::new_v4().to_string(), export_id: export_id.into(), name: name.into(), thumbnail_hash: None, thumbnail_model_signature: None, length: default_length(), width: default_width(),
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
        assert_eq!(CarPlacementRule { preset: PlacementPreset::Every, every: Some(3), offset: 1, ..Default::default() }.expressions().unwrap().0, "%3+2");
        assert!(CarPlacementRule { preset: PlacementPreset::Custom, whitelist: "%0".into(), ..Default::default() }.expressions().is_err());
    }
    #[test] fn slugs_are_valid_resource_identifiers() { assert_eq!(slugify("Urban Rail / 2026", "train"), "urban_rail_2026"); }
    // Mirrors the official MTR 3 DynamicTrainModel strength calculation.
    fn native_strength(filter: &str, position: i32, count: i32) -> u8 {
        let mut strength = 0;
        for token in filter.split(',').filter(|token|!token.is_empty()) {
            if let Some(periodic) = token.strip_prefix('%') {
                let (multiple,additional) = periodic.split_once('+').unwrap_or((periodic,"0"));
                if (position+additional.parse::<i32>().unwrap())%multiple.parse::<i32>().unwrap()==0 {strength=strength.max(2);}
            } else {
                let number=token.parse::<i32>().unwrap();
                if number==position || number==position-count-1 {return 3;}
            }
        }
        strength
    }
    fn native_positions(rule: CarPlacementRule) -> Vec<i32> {
        let (white,black)=rule.expressions().unwrap();
        (1..=6).filter(|position| native_strength(&black,*position,6)<=native_strength(&white,*position,6)).collect()
    }
    #[test]
    fn placement_presets_render_only_the_intended_positions_in_mtr3() {
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::First,..Default::default()}),vec![1]);
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::Last,..Default::default()}),vec![6]);
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::Odd,..Default::default()}),vec![1,3,5]);
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::Even,..Default::default()}),vec![2,4,6]);
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::Every,every:Some(3),offset:1,..Default::default()}),vec![1,4]);
        assert_eq!(native_positions(CarPlacementRule{preset:PlacementPreset::Every,every:Some(3),offset:-1,..Default::default()}),vec![2,5]);
        let custom=CarPlacementRule{preset:PlacementPreset::Custom,whitelist:" 1, %3-1 ".into(),blacklist:"%1".into(),..Default::default()};
        assert_eq!(custom.expressions().unwrap(),("1,%3+-1".into(),"%1".into()));
        assert_eq!(native_positions(custom),vec![1,4]);
        assert!(CarPlacementRule{preset:PlacementPreset::Custom,whitelist:"%3+".into(),..Default::default()}.expressions().is_err());
    }

}
