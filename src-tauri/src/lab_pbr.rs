//! LabPBR 1.3 data textures. UI/glTF roughness is perceptual; GGX squares it.
//! Therefore Lab perceptual smoothness is 1 - UI roughness (not 1 - sqrt(UI roughness)).
use crate::material::{MaterialProperties, TextureChannel as C};
use std::collections::BTreeMap;

pub fn png(image: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}
pub fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("Cannot decode PBR texture: {e}"))
}
pub fn has_lab(p: &MaterialProperties) -> bool {
    p.maps.contains_key(&C::LabNormal) || p.maps.contains_key(&C::LabSpecular)
}
pub fn has_pbr(p: &MaterialProperties) -> bool {
    p.metalness.is_some() || p.roughness.is_some() || p.emissive.is_some() || !p.maps.is_empty()
}

/// Export imported Lab channels unchanged; otherwise bake conventional maps and factors.
/// Lab supports discrete conductors. Fractional conventional metalness uses a 0.5 threshold.
pub fn encode(
    p: &MaterialProperties,
    maps: &BTreeMap<C, Vec<u8>>,
    size: (u32, u32),
) -> Result<Option<[Vec<u8>; 2]>, String> {
    if !has_pbr(p) {
        return Ok(None);
    }
    let maps = maps
        .iter()
        .map(|(c, b)| {
            Ok((
                *c,
                image::imageops::resize(
                    &decode(b)?,
                    size.0,
                    size.1,
                    image::imageops::FilterType::Nearest,
                ),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let sample = |c, x, y, default| maps.get(&c).map_or(default, |i| i.get_pixel(x, y).0);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let locked = has_lab(p);
    let normal = image::RgbaImage::from_fn(size.0, size.1, |x, y| {
        if let Some(i) = maps.get(&C::LabNormal) {
            return *i.get_pixel(x, y);
        }
        if !maps.contains_key(&C::Normal) {
            return image::Rgba([128, 128, sample(C::Occlusion, x, y, [255; 4])[0], 255]);
        }
        let n = sample(C::Normal, x, y, [128, 128, 255, 255]);
        let strength = if locked {
            1.0
        } else {
            p.normal_scale.unwrap_or(1.0)
        };
        let nx = (n[0] as f32 / 255.0 * 2.0 - 1.0) * strength;
        let ny = (n[1] as f32 / 255.0 * 2.0 - 1.0) * strength;
        let length = (nx * nx + ny * ny).sqrt().max(1.0);
        image::Rgba([
            byte(nx / length * 0.5 + 0.5),
            byte(-ny / length * 0.5 + 0.5),
            sample(C::Occlusion, x, y, [255; 4])[0],
            255,
        ])
    });
    let spec = image::RgbaImage::from_fn(size.0, size.1, |x, y| {
        if let Some(i) = maps.get(&C::LabSpecular) {
            return *i.get_pixel(x, y);
        }
        let rough =
            sample(C::Roughness, x, y, [255; 4])[0] as f32 / 255.0 * p.roughness.unwrap_or(0.8);
        let metal =
            sample(C::Metalness, x, y, [255; 4])[0] as f32 / 255.0 * p.metalness.unwrap_or(0.0);
        let emission = p.emissive.unwrap_or([0.0; 3]);
        let emap = sample(C::Emissive, x, y, [255; 4]);
        let e = (0..3)
            .map(|i| emission[i] * srgb_to_linear(emap[i] as f32 / 255.0))
            .fold(0.0_f32, f32::max);
        image::Rgba([
            byte(1.0 - rough),
            if metal >= 0.5 { 255 } else { 10 },
            0,
            if e > 0.0 {
                (e.clamp(0.0, 1.0) * 254.0).round() as u8
            } else {
                255
            },
        ])
    });
    Ok(Some([png(&normal)?, png(&spec)?]))
}

pub struct Preview {
    pub normal: Option<Vec<u8>>,
    pub ao: Option<Vec<u8>>,
    pub rough_metal: Option<Vec<u8>>,
    pub emission: Option<Vec<u8>>,
    pub reflectance: Option<Vec<u8>>,
}
/// Decode Lab data to OpenGL normals, linear AO, glTF perceptual roughness/metalness,
/// sRGB emission masks and KHR_materials_specular dielectric F0.
pub fn preview(normal: Option<&Vec<u8>>, spec: Option<&Vec<u8>>) -> Result<Preview, String> {
    let mut result = Preview {
        normal: None,
        ao: None,
        rough_metal: None,
        emission: None,
        reflectance: None,
    };
    if let Some(bytes) = normal {
        let source = decode(bytes)?;
        let n = image::RgbaImage::from_fn(source.width(), source.height(), |x, y| {
            let p = source.get_pixel(x, y);
            let nx = p[0] as f32 / 255.0 * 2.0 - 1.0;
            let ny = p[1] as f32 / 255.0 * 2.0 - 1.0;
            image::Rgba([
                p[0],
                255 - p[1],
                (((1.0 - nx * nx - ny * ny).max(0.0).sqrt() * 0.5 + 0.5) * 255.0).round() as u8,
                255,
            ])
        });
        result.normal = Some(png(&n)?);
        result.ao = Some(png(&image::RgbaImage::from_fn(
            source.width(),
            source.height(),
            |x, y| {
                let a = source.get_pixel(x, y)[2];
                image::Rgba([a, a, a, 255])
            },
        ))?);
    }
    if let Some(bytes) = spec {
        let source = decode(bytes)?;
        let map = |f: fn([u8; 4]) -> [u8; 4]| {
            png(&image::RgbaImage::from_fn(
                source.width(),
                source.height(),
                |x, y| image::Rgba(f(source.get_pixel(x, y).0)),
            ))
        };
        result.rough_metal = Some(map(|p| {
            [255, 255 - p[0], if p[1] >= 230 { 255 } else { 0 }, 255]
        })?);
        result.emission = Some(map(|p| {
            let e = if p[3] == 255 {
                0
            } else {
                (linear_to_srgb(p[3] as f32 / 254.0) * 255.0).round() as u8
            };
            [e, e, e, 255]
        })?);
        // Three's physical material multiplies dielectric F0 by specularColor.
        // IOR=100 gives a base F0 near 1; compensate it in the decoded texture.
        result.reflectance = Some(map(|p| {
            let f = if p[1] < 230 {
                (p[1] as f32 / 255.0 / 0.9607882).min(1.0)
            } else {
                1.0
            };
            let s = (if f <= 0.0031308 {
                12.92 * f
            } else {
                1.055 * f.powf(1.0 / 2.4) - 0.055
            } * 255.0)
                .round() as u8;
            [s, s, s, 255]
        })?);
    }
    Ok(result)
}
// Predefined conductor F0 at normal incidence, computed from Lab's published n/k.
// Lab albedo tints these reflections. glTF expresses the result as metallic base color.
pub fn conductor_base(base: Option<&Vec<u8>>, spec: &Vec<u8>) -> Result<Option<Vec<u8>>, String> {
    let spec = decode(spec)?;
    if !spec.pixels().any(|p| (230..=237).contains(&p[1])) {
        return Ok(None);
    }
    let base = base.map(|b| decode(b)).transpose()?;
    let nk = [
        ([2.9114, 2.9497, 2.5845], [3.0893, 2.9318, 2.7670]),
        ([0.18299, 0.42108, 1.3734], [3.4242, 2.3459, 1.7704]),
        ([1.3456, 0.96521, 0.61722], [7.4746, 6.3995, 5.3031]),
        ([3.1071, 3.1812, 2.3230], [3.3314, 3.3291, 3.1350]),
        ([0.27105, 0.67693, 1.3164], [3.6092, 2.6248, 2.2921]),
        ([1.9100, 1.8300, 1.4400], [3.5100, 3.4000, 3.1800]),
        ([2.3757, 2.0847, 1.8453], [4.2655, 3.7153, 3.1365]),
        ([0.15943, 0.14512, 0.13547], [3.9291, 3.1900, 2.3808]),
    ];
    let size = base.as_ref().map_or((spec.width(), spec.height()), |i| {
        (i.width().max(spec.width()), i.height().max(spec.height()))
    });
    let image = image::RgbaImage::from_fn(size.0, size.1, |x, y| {
        let mut pixel = base
            .as_ref()
            .map(|i| {
                i.get_pixel(x * i.width() / size.0, y * i.height() / size.1)
                    .0
            })
            .unwrap_or([255; 4]);
        let metal = spec.get_pixel(x * spec.width() / size.0, y * spec.height() / size.1)[1];
        if (230..=237).contains(&metal) {
            let (n, k) = nk[(metal - 230) as usize];
            for c in 0..3 {
                let f0 =
                    ((n[c] - 1.0_f32).powi(2) + k[c] * k[c]) / ((n[c] + 1.0).powi(2) + k[c] * k[c]);
                pixel[c] = (linear_to_srgb(srgb_to_linear(pixel[c] as f32 / 255.0) * f0) * 255.0)
                    .round() as u8;
            }
        }
        image::Rgba(pixel)
    });
    Ok(Some(png(&image)?))
}
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pixel(p: [u8; 4]) -> Vec<u8> {
        png(&image::RgbaImage::from_pixel(1, 1, image::Rgba(p))).unwrap()
    }
    #[test]
    fn scalar_material_encodes_lab_defaults_and_perceptual_roughness() {
        let p = MaterialProperties {
            roughness: Some(0.25),
            metalness: Some(1.0),
            ..Default::default()
        };
        let [n, s] = encode(&p, &BTreeMap::new(), (2, 3)).unwrap().unwrap();
        assert_eq!(decode(&n).unwrap().get_pixel(0, 0).0, [128, 128, 255, 255]);
        assert_eq!(decode(&s).unwrap().get_pixel(0, 0).0, [191, 255, 0, 255]);
        assert_eq!(decode(&s).unwrap().dimensions(), (2, 3));
        assert!(
            encode(&MaterialProperties::default(), &BTreeMap::new(), (1, 1))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn lab_channels_override_scalars_and_preserve_height_porosity_and_alpha() {
        let n = pixel([170, 100, 150, 42]);
        let s = pixel([77, 231, 200, 254]);
        let p = MaterialProperties {
            roughness: Some(1.0),
            maps: BTreeMap::from([(C::LabNormal, "n".into()), (C::LabSpecular, "s".into())]),
            ..Default::default()
        };
        let result = encode(
            &p,
            &BTreeMap::from([(C::LabNormal, n.clone()), (C::LabSpecular, s.clone())]),
            (1, 1),
        )
        .unwrap()
        .unwrap();
        assert_eq!(decode(&result[0]).unwrap(), decode(&n).unwrap());
        assert_eq!(decode(&result[1]).unwrap(), decode(&s).unwrap());
        let decoded = preview(Some(&n), Some(&s)).unwrap();
        assert_eq!(
            decode(&decoded.normal.unwrap()).unwrap().get_pixel(0, 0)[1],
            155
        );
        assert_eq!(
            decode(&decoded.ao.unwrap()).unwrap().get_pixel(0, 0)[0],
            150
        );
        assert_eq!(
            decode(&decoded.rough_metal.unwrap())
                .unwrap()
                .get_pixel(0, 0)
                .0,
            [255, 178, 255, 255]
        );
        assert_eq!(
            decode(&decoded.emission.unwrap()).unwrap().get_pixel(0, 0)[0],
            255
        );
        let decoded = preview(None, Some(&pixel([0, 10, 0, 255]))).unwrap();
        assert_eq!(
            decode(&decoded.emission.unwrap()).unwrap().get_pixel(0, 0)[0],
            0
        );
    }
    #[test]
    fn conductor_ids_produce_distinct_reflection_colors() {
        let gold = decode(
            &conductor_base(None, &pixel([0, 231, 0, 255]))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let p = gold.get_pixel(0, 0);
        assert!(p[0] > p[1] && p[1] > p[2]);
        assert!(conductor_base(None, &pixel([0, 255, 0, 255]))
            .unwrap()
            .is_none());
    }
}
