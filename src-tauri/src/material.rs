use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Persisted source properties and per-layer overrides use the same optional channels.
/// Missing fields retain the legacy preview defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MaterialProperties {
    pub metalness: Option<f32>,
    pub roughness: Option<f32>,
    pub emissive: Option<[f32; 3]>,
    pub opacity: Option<f32>,
    pub alpha_mode: Option<AlphaMode>,
    pub alpha_cutoff: Option<f32>,
    pub normal_scale: Option<f32>,
    pub double_sided: Option<bool>,
    /// Source values are dependency filenames; layer overrides are image blob hashes.
    pub maps: BTreeMap<TextureChannel, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum TextureChannel {
    Normal,
    Metalness,
    Roughness,
    Emissive,
    Occlusion,
}

impl TextureChannel {
    pub fn key(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Metalness => "metalness",
            Self::Roughness => "roughness",
            Self::Emissive => "emissive",
            Self::Occlusion => "occlusion",
        }
    }
}

impl MaterialProperties {
    pub fn overlay(&mut self, edit: &Self) {
        macro_rules! field { ($($name:ident),*) => { $(if edit.$name.is_some() { self.$name = edit.$name; })* }; }
        field!(
            metalness,
            roughness,
            emissive,
            opacity,
            alpha_mode,
            alpha_cutoff,
            normal_scale,
            double_sided
        );
        self.maps.extend(edit.maps.clone());
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("metalness", self.metalness),
            ("roughness", self.roughness),
            ("opacity", self.opacity),
            ("alpha cutoff", self.alpha_cutoff),
        ] {
            if value.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v)) {
                return Err(format!("Material {name} must be between 0 and 1."));
            }
        }
        if self
            .normal_scale
            .is_some_and(|v| !v.is_finite() || !(0.0..=4.0).contains(&v))
        {
            return Err("Material normal scale must be between 0 and 4.".into());
        }
        if self
            .emissive
            .is_some_and(|v| v.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)))
        {
            return Err("Material emission must be between 0 and 1.".into());
        }
        if self.maps.values().any(|v| v.trim().is_empty()) {
            return Err("Material texture reference is empty.".into());
        }
        Ok(())
    }
}

/// glTF stores roughness in G and metalness in B. Independent source maps use R.
pub fn pack_metallic_roughness(
    metal: Option<&Vec<u8>>,
    rough: Option<&Vec<u8>>,
) -> Result<Option<Vec<u8>>, String> {
    if metal.is_none() && rough.is_none() {
        return Ok(None);
    }
    let decode = |bytes: &Vec<u8>| {
        image::load_from_memory(bytes)
            .map(|image| image.to_rgba8())
            .map_err(|e| e.to_string())
    };
    let metal = metal.map(decode).transpose()?;
    let rough = rough.map(decode).transpose()?;
    let width = metal
        .as_ref()
        .map_or(1, |v| v.width())
        .max(rough.as_ref().map_or(1, |v| v.width()));
    let height = metal
        .as_ref()
        .map_or(1, |v| v.height())
        .max(rough.as_ref().map_or(1, |v| v.height()));
    let resize = |image: image::RgbaImage| {
        image::imageops::resize(&image, width, height, image::imageops::FilterType::Triangle)
    };
    let metal = metal.map(resize);
    let rough = rough.map(resize);
    let packed = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([
            255,
            rough.as_ref().map_or(255, |v| v.get_pixel(x, y)[0]),
            metal.as_ref().map_or(255, |v| v.get_pixel(x, y)[0]),
            255,
        ])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    packed
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(Some(bytes.into_inner()))
}
