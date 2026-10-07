//! MTR's Blockbench loader uses cuboids, box UVs and top-level named groups.
//! Its renderer stores model coordinates as (-X, -Y, Z); canonical editor
//! coordinates are the negation of those coordinates, in metres.
use crate::{
    domain::{ModelLayer, ModelTransform},
    model::{ModelDocument, ModelMaterial, ModelPart},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn read(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("Unable to read BBMODEL: {e}"))?)
        .map_err(|e| format!("Invalid BBMODEL JSON: {e}"))
}

fn vector(value: &Value, field: &str, default: [f32; 3]) -> Result<[f32; 3], String> {
    let Some(raw) = value.get(field) else {
        return Ok(default);
    };
    let values = raw
        .as_array()
        .filter(|a| a.len() == 3)
        .ok_or_else(|| format!("BBMODEL {field} must contain three numbers."))?;
    let mut result = [0.0; 3];
    for (i, v) in values.iter().enumerate() {
        result[i] = v
            .as_f64()
            .filter(|n| n.is_finite() && n.abs() <= 1e7)
            .ok_or_else(|| format!("Invalid BBMODEL {field}."))? as f32;
    }
    Ok(result)
}

pub fn resolution(model: &Value) -> Result<[u32; 2], String> {
    let mut size = [16; 2];
    for (i, key) in ["width", "height"].iter().enumerate() {
        if let Some(value) = model.get("resolution").and_then(|r| r.get(key)) {
            size[i] = value
                .as_u64()
                .filter(|n| *n > 0 && *n <= 8192)
                .ok_or("BBMODEL texture resolution must be between 1 and 8192 pixels.")?
                as u32;
        }
    }
    Ok(size)
}

fn texture_name(texture: &Value) -> String {
    let requested = texture
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| texture.get("relative_path").and_then(Value::as_str))
        .or_else(|| texture.get("path").and_then(Value::as_str))
        .unwrap_or("texture.png");
    requested
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("texture.png")
        .into()
}

pub fn dependencies(
    path: &Path,
    overrides: &BTreeMap<String, String>,
) -> Result<(Vec<PathBuf>, Vec<(String, Vec<u8>)>), String> {
    let model = read(path)?;
    let Some(texture) = model
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    else {
        return Ok((vec![], vec![]));
    };
    let name = texture_name(texture);
    if let Some(source) = texture
        .get("source")
        .and_then(Value::as_str)
        .filter(|s| s.starts_with("data:"))
    {
        let (header, payload) = source
            .split_once(',')
            .ok_or("Invalid embedded BBMODEL texture.")?;
        if !header.ends_with(";base64") {
            return Err("Embedded BBMODEL textures must use base64.".into());
        }
        let bytes = STANDARD
            .decode(payload)
            .map_err(|_| "Invalid embedded BBMODEL texture data.")?;
        image::load_from_memory(&bytes)
            .map_err(|_| "The embedded BBMODEL texture is not a supported image.")?;
        return Ok((vec![], vec![(name, bytes)]));
    }
    if let Some(chosen) = overrides.get(&name) {
        return Ok((vec![PathBuf::from(chosen)], vec![]));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let candidates: Vec<_> = ["relative_path", "path", "source"]
        .into_iter()
        .filter_map(|key| texture.get(key).and_then(Value::as_str))
        .filter(|s| !s.is_empty() && !s.starts_with("data:"))
        .map(|s| parent.join(s.replace('\\', "/")))
        .chain(std::iter::once(parent.join(&name)))
        .collect();
    let resolved = candidates
        .iter()
        .find(|p| p.is_file())
        .cloned()
        .unwrap_or_else(|| parent.join(name));
    Ok((vec![resolved], vec![]))
}

struct Source {
    model: Value,
    elements: BTreeMap<String, Value>,
    roots: Vec<Value>,
    warnings: Vec<String>,
}

fn resolve_group(
    node: &Value,
    groups: &BTreeMap<String, Value>,
    depth: usize,
) -> Result<Value, String> {
    if depth > 64 {
        return Err("BBMODEL group nesting is too deep or cyclic.".into());
    }
    if node.is_string() {
        return Ok(node.clone());
    }
    let id = node.get("uuid").and_then(Value::as_str).unwrap_or("");
    let mut value = groups.get(id).cloned().unwrap_or_else(|| node.clone());
    if let Some(object) = node.as_object() {
        for (key, child) in object {
            value[key] = child.clone();
        }
    }
    if !value.is_object() {
        return Err("Invalid BBMODEL group.".into());
    }
    let children = value
        .get("children")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    value["children"] = Value::Array(
        children
            .iter()
            .map(|child| resolve_group(child, groups, depth + 1))
            .collect::<Result<_, _>>()?,
    );
    Ok(value)
}

fn source(mut model: Value) -> Result<Source, String> {
    resolution(&model)?;
    let mut elements = BTreeMap::new();
    for (index, element) in model
        .get("elements")
        .and_then(Value::as_array)
        .ok_or("BBMODEL has no elements array.")?
        .iter()
        .enumerate()
    {
        if element
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|s| s != "cube")
        {
            return Err("MTR BBMODEL import supports cuboid elements. Export meshes as OBJ from Blockbench.".into());
        }
        let id = element
            .get("uuid")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("BBMODEL element {index} has no UUID."))?;
        if elements.insert(id.into(), element.clone()).is_some() {
            return Err("BBMODEL element UUIDs must be unique.".into());
        }
    }
    if elements.is_empty() {
        return Err("The BBMODEL file contains no cuboid elements.".into());
    }
    let groups: BTreeMap<_, _> = model
        .get("groups")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|g| Some((g.get("uuid")?.as_str()?.to_owned(), g.clone())))
        .collect();
    let mut roots = Vec::new();
    for (index, node) in model
        .get("outliner")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let resolved = resolve_group(node, &groups, 0)?;
        roots.push(if let Some(id)=resolved.as_str() {json!({"name":elements.get(id).and_then(|e|e.get("name")).and_then(Value::as_str).unwrap_or("Element"),"uuid":format!("loose-{index}-{id}"),"children":[id]})}else{resolved});
    }
    if roots.is_empty() {
        roots.push(
            json!({"name":"Model","uuid":"model","children":elements.keys().collect::<Vec<_>>()}),
        );
    }
    let mut warnings = Vec::new();
    if model
        .get("textures")
        .and_then(Value::as_array)
        .is_some_and(|a| a.len() > 1)
    {
        warnings
            .push("MTR's native BBMODEL renderer uses the first texture for every cuboid.".into());
    }
    if model
        .get("animations")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        warnings.push(
            "Blockbench timeline animations are not played by MTR's native model loader.".into(),
        );
    }
    if elements
        .values()
        .any(|e| e.get("box_uv").and_then(Value::as_bool) == Some(false))
        || model.pointer("/meta/box_uv").and_then(Value::as_bool) == Some(false)
    {
        warnings.push("MTR uses box UV offsets; custom per-face UV layouts are not read by its BBMODEL loader.".into());
    }
    model["outliner"] = Value::Array(roots.clone());
    model
        .as_object_mut()
        .ok_or("BBMODEL must be an object.")?
        .remove("groups");
    Ok(Source {
        model,
        elements,
        roots,
        warnings,
    })
}

// Vanilla rotates local X, then Y, then Z. Group rotations are ignored at the root.
fn rotate(mut v: [f32; 3], r: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = r[0].sin_cos();
    let (sy, cy) = r[1].sin_cos();
    let (sz, cz) = r[2].sin_cos();
    v = [v[0], v[1] * cx - v[2] * sx, v[1] * sx + v[2] * cx];
    v = [v[0] * cy + v[2] * sy, v[1], -v[0] * sy + v[2] * cy];
    [v[0] * cz - v[1] * sz, v[0] * sz + v[1] * cz, v[2]]
}

fn engine_origin(v: [f32; 3]) -> [f32; 3] {
    [-v[0], -v[1], v[2]]
}
fn engine_rotation(v: [f32; 3]) -> [f32; 3] {
    [-v[0].to_radians(), -v[1].to_radians(), v[2].to_radians()]
}

fn append_cube(
    part: &mut ModelPart,
    cube: &Value,
    pivot: [f32; 3],
    group_rotation: [f32; 3],
    size: [u32; 2],
) -> Result<(), String> {
    let from = vector(cube, "from", [0.0; 3])?;
    let to = vector(cube, "to", [0.0; 3])?;
    if (0..3).any(|i| to[i] < from[i]) {
        return Err("BBMODEL cuboid dimensions cannot be negative.".into());
    }
    let dims: [f32; 3] = std::array::from_fn(|i| (to[i] - from[i]).round());
    let inflate = cube
        .get("inflate")
        .map(|v| {
            v.as_f64()
                .filter(|v| v.is_finite())
                .ok_or("Invalid BBMODEL inflation.")
        })
        .transpose()?
        .unwrap_or(0.0) as f32;
    let origin = engine_origin(vector(cube, "origin", [0.0; 3])?);
    let rotation = engine_rotation(vector(cube, "rotation", [0.0; 3])?);
    let low = [-to[0] - inflate, -to[1] - inflate, from[2] - inflate];
    let high = [
        -to[0] + dims[0] + inflate,
        -to[1] + dims[1] + inflate,
        from[2] + dims[2] + inflate,
    ];
    if (0..3).any(|i| high[i] < low[i]) {
        return Err("BBMODEL inflation produces negative dimensions.".into());
    }
    let mirror = cube.get("shade").and_then(Value::as_bool) == Some(false)
        || cube.get("mirror_uv").and_then(Value::as_bool) == Some(true);
    let (x0, x1) = if mirror {
        (high[0], low[0])
    } else {
        (low[0], high[0])
    };
    let (y0, y1, z0, z1) = (low[1], high[1], low[2], high[2]);
    let vertices = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y1, z0],
        [x0, y1, z0],
        [x0, y0, z1],
        [x1, y0, z1],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let uv = cube.get("uv_offset").and_then(Value::as_array);
    let u = uv
        .and_then(|v| v.first())
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    let v = uv
        .and_then(|v| v.get(1))
        .and_then(Value::as_f64)
        .unwrap_or(0.0) as f32;
    if !u.is_finite() || !v.is_finite() {
        return Err("Invalid BBMODEL UV offset.".into());
    }
    let [dx, dy, dz] = dims;
    let faces = [
        ([5, 4, 0, 1], [u + dz, v, u + dz + dx, v + dz]),
        ([2, 3, 7, 6], [u + dz + dx, v + dz, u + dz + dx * 2.0, v]),
        ([0, 4, 7, 3], [u, v + dz, u + dz, v + dz + dy]),
        ([1, 0, 3, 2], [u + dz, v + dz, u + dz + dx, v + dz + dy]),
        (
            [5, 1, 2, 6],
            [u + dz + dx, v + dz, u + dz + dx + dz, v + dz + dy],
        ),
        (
            [4, 5, 6, 7],
            [
                u + dz + dx + dz,
                v + dz,
                u + dz + dx * 2.0 + dz,
                v + dz + dy,
            ],
        ),
    ];
    for (indices, rect) in faces {
        let [u1, v1, u2, v2] = rect;
        let uvs = [[u2, v1], [u1, v1], [u1, v2], [u2, v2]];
        let base = part.positions.len() as u32;
        for i in 0..4 {
            let index = if mirror { 3 - i } else { i };
            let pos = vertices[indices[index]];
            let local = rotate(std::array::from_fn(|i| pos[i] - origin[i]), rotation);
            let grouped = rotate(
                std::array::from_fn(|i| local[i] + origin[i] - pivot[i]),
                group_rotation,
            );
            part.positions
                .push(std::array::from_fn(|i| -(grouped[i] + pivot[i]) / 16.0));
            part.texcoords.push([
                uvs[index][0] / size[0] as f32,
                uvs[index][1] / size[1] as f32,
            ]);
        }
        // Conversion negates three axes and reverses polygon winding.
        part.indices
            .extend([base, base + 2, base + 1, base, base + 3, base + 2]);
        let a = part.positions[base as usize];
        let b = part.positions[base as usize + 2];
        let c = part.positions[base as usize + 1];
        let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
        let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        let n = if length > 0.0 {
            n.map(|v| v / length)
        } else {
            [0.0, 1.0, 0.0]
        };
        part.normals.extend([n; 4]);
    }
    Ok(())
}

fn visit(
    group: &Value,
    previous: Option<&Value>,
    source: &Source,
    part: &mut ModelPart,
    visited: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
    depth: usize,
) -> Result<(), String> {
    if depth > 64 {
        return Err("BBMODEL group nesting is too deep.".into());
    }
    let origin = vector(group, "origin", [0.0; 3])?;
    let rotation = vector(group, "rotation", [0.0; 3])?;
    let (origin, rotation) = if let Some(parent) = previous {
        let parent = vector(parent, "origin", [0.0; 3])?;
        (std::array::from_fn(|i| origin[i] - parent[i]), rotation)
    } else {
        if rotation.iter().any(|v| v.abs() > 1.0) {
            warnings.push(format!(
                "MTR ignores the root rotation of group {}.",
                part.name
            ));
        }
        (origin, [0.0; 3])
    };
    for child in group
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = child.as_str() {
            if !visited.insert(id.into()) {
                return Err("A BBMODEL element is referenced more than once.".into());
            }
            let cube = source
                .elements
                .get(id)
                .ok_or_else(|| format!("BBMODEL group references missing element {id}."))?;
            append_cube(
                part,
                cube,
                engine_origin(origin),
                engine_rotation(rotation),
                resolution(&source.model)?,
            )?;
        } else {
            visit(
                child,
                Some(group),
                source,
                part,
                visited,
                warnings,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

pub fn parse(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let source = source(read(path)?)?;
    let mut warnings = source.warnings.clone();
    let mut parts = Vec::new();
    let mut visited = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (index, root) in source.roots.iter().enumerate() {
        let id = root
            .get("uuid")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("group-{index}"));
        if !ids.insert(id.clone()) {
            return Err("BBMODEL root group UUIDs must be unique.".into());
        }
        let mut part = ModelPart {
            id,
            name: root
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Group")
                .into(),
            positions: vec![],
            normals: vec![],
            texcoords: vec![],
            indices: vec![],
            material: Some(0),
        };
        visit(
            root,
            None,
            &source,
            &mut part,
            &mut visited,
            &mut warnings,
            0,
        )?;
        if !part.indices.is_empty() {
            parts.push(part);
        }
    }
    if visited.len() != source.elements.len() {
        return Err(
            "Some BBMODEL elements are outside the outliner. Add them to a group before importing."
                .into(),
        );
    }
    let texture = source
        .model
        .get("textures")
        .and_then(Value::as_array)
        .and_then(|a| a.first());
    let resolved_texture = if texture.is_some() {
        let (references, embedded) = dependencies(path, overrides)?;
        embedded.first().map(|(name, _)| name.clone()).or_else(|| {
            references
                .first()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
    } else {
        None
    };
    let materials = vec![ModelMaterial {
        id: "material-0".into(),
        name: texture
            .map(texture_name)
            .unwrap_or_else(|| "Default".into()),
        color: [1.0; 4],
        texture: resolved_texture,
        properties: Default::default(),
    }];
    Ok(ModelDocument {
        parts,
        materials,
        warnings,
        uv_origin_top_left: true,
    })
}

fn collect(
    group: &Value,
    elements: &BTreeMap<String, Value>,
    result: &mut Vec<Value>,
) -> Result<(), String> {
    for child in group
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = child.as_str() {
            result.push(elements.get(id).ok_or("Missing BBMODEL element.")?.clone());
        } else {
            collect(child, elements, result)?;
        }
    }
    Ok(())
}

pub fn native_model(
    bytes: &[u8],
    parts: &[ModelPart],
    layer: &ModelLayer,
    mtr3: bool,
) -> Result<Value, String> {
    if layer.transform != ModelTransform::default()
        || layer
            .part_transforms
            .iter()
            .any(|(id, v)| parts.iter().any(|p| &p.id == id) && *v != ModelTransform::default())
    {
        return Err("Native BBMODEL export cannot preserve edited transforms. Choose OBJ to bake these transforms.".into());
    }
    let source =
        source(serde_json::from_slice(bytes).map_err(|e| format!("Invalid BBMODEL source: {e}"))?)?;
    let mut roots = Vec::new();
    let mut elements = Vec::new();
    for (index, mut root) in source.roots.iter().cloned().enumerate() {
        let id = root
            .get("uuid")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("group-{index}"));
        let Some(part) = parts.iter().find(|p| p.id == id) else {
            continue;
        };
        root["name"] = part.name.clone().into();
        root["uuid"] = id.into();
        if mtr3 {
            validate_mtr3_groups(&root, true)?;
        }
        collect(&root, &source.elements, &mut elements)?;
        roots.push(root);
    }
    if !parts.is_empty() && elements.is_empty() {
        return Err("Native BBMODEL groups do not match the imported model. Choose OBJ or reimport the source.".into());
    }
    for element in &mut elements {
        element["inflate"] = element.get("inflate").cloned().unwrap_or(json!(0));
        if mtr3 {
            // UVs outside a tile would sample another model's texture in the atlas.
            let size = resolution(&source.model)?;
            let from = vector(element, "from", [0.0; 3])?;
            let to = vector(element, "to", [0.0; 3])?;
            let dims: [f32; 3] = std::array::from_fn(|i| (to[i] - from[i]).round());
            let uv = element.get("uv_offset").and_then(Value::as_array);
            let u = uv
                .and_then(|a| a.first())
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let v = uv
                .and_then(|a| a.get(1))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if u < 0.0
                || v < 0.0
                || u + 2.0 * (dims[0] + dims[2]) as f64 > size[0] as f64
                || v + (dims[1] + dims[2]) as f64 > size[1] as f64
            {
                return Err("Native MTR 3 texture atlases require box UVs to fit inside the model resolution. Choose OBJ to preserve repeating UVs.".into());
            }
        }
        if mtr3 && element.get("mirror_uv").and_then(Value::as_bool) == Some(true) {
            element["shade"] = json!(false);
        }
    }
    let mut model = json!({"meta":{"format_version":"4.10","model_format":"modded_entity","box_uv":true},"resolution":{"width":resolution(&source.model)?[0],"height":resolution(&source.model)?[1]}});
    model["elements"] = Value::Array(elements);
    model["outliner"] = Value::Array(roots);
    // Texture bytes are exported separately, never retain machine-local paths.
    model["textures"] = json!([]);
    model["animations"] = json!([]);
    Ok(model)
}

fn validate_mtr3_groups(group: &Value, root: bool) -> Result<(), String> {
    if !root
        && vector(group, "rotation", [0.0; 3])?
            .iter()
            .any(|v| v.abs() > 0.0001)
    {
        return Err("MTR 3 ignores nested BBMODEL group rotations. Choose OBJ to preserve the preview geometry.".into());
    }
    for child in group
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|v| v.is_object())
    {
        validate_mtr3_groups(child, false)?;
    }
    Ok(())
}

/// Shift every cube for static bogie placement and move its UVs into a texture atlas.
pub fn shift_model(
    model: &mut Value,
    z_offset: f32,
    uv_y: u32,
    identity: &str,
) -> Result<(), String> {
    let elements = model
        .get_mut("elements")
        .and_then(Value::as_array_mut)
        .ok_or("Missing BBMODEL elements.")?;
    let mut ids = BTreeMap::new();
    for element in elements {
        let old = element
            .get("uuid")
            .and_then(Value::as_str)
            .ok_or("Missing BBMODEL UUID.")?
            .to_owned();
        let new = format!("{}_{}", identity, old);
        ids.insert(old, new.clone());
        element["uuid"] = new.into();
        for field in ["from", "to", "origin"] {
            let mut v = vector(element, field, [0.0; 3])?;
            v[2] -= z_offset * 16.0;
            element[field] = json!(v);
        }
        let uv = element.get("uv_offset").and_then(Value::as_array);
        let x = uv
            .and_then(|a| a.first())
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let y = uv
            .and_then(|a| a.get(1))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        element["uv_offset"] = json!([x, y + uv_y as i64]);
    }
    fn rename(group: &mut Value, ids: &BTreeMap<String, String>) {
        if let Some(id) = group.as_str() {
            if let Some(new) = ids.get(id) {
                *group = new.clone().into();
            }
        } else {
            if let Some(id) = group.get("uuid").and_then(Value::as_str) {
                group["uuid"] = format!(
                    "group-{}-{}",
                    ids.values().next().map(String::as_str).unwrap_or("empty"),
                    id
                )
                .into();
            }
            if let Some(children) = group.get_mut("children").and_then(Value::as_array_mut) {
                for child in children {
                    rename(child, ids);
                }
            }
        }
    }
    for root in model
        .get_mut("outliner")
        .and_then(Value::as_array_mut)
        .ok_or("Missing BBMODEL outliner.")?
    {
        rename(root, &ids);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write_model(model: &Value) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("bbmodel-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("model.bbmodel");
        fs::write(&path, serde_json::to_vec(model).unwrap()).unwrap();
        (root, path)
    }
    fn cube_model() -> Value {
        json!({"resolution":{"width":64,"height":64},"elements":[{"name":"Cube","type":"cube","uuid":"cube","from":[0,0,0],"to":[16,16,32],"uv_offset":[2,3]}],"outliner":[{"name":"Body","uuid":"body","children":["cube"]}]})
    }
    fn bounds(document: &ModelDocument) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for point in document.parts.iter().flat_map(|p| &p.positions) {
            for i in 0..3 {
                min[i] = min[i].min(point[i]);
                max[i] = max[i].max(point[i]);
            }
        }
        (min, max)
    }
    #[test]
    fn cuboids_use_mtr_units_axes_box_uvs_and_element_rotation() {
        let (root, path) = write_model(&cube_model());
        let document = parse(&path, &BTreeMap::new()).unwrap();
        assert_eq!(bounds(&document), ([0.0, 0.0, -2.0], [1.0, 1.0, 0.0]));
        assert_eq!(document.parts[0].indices.len(), 36);
        assert_eq!(document.parts[0].id, "body");
        assert!(document.uv_origin_top_left);
        assert_eq!(
            document.parts[0].texcoords[0],
            [(2.0 + 32.0 + 16.0) / 64.0, 3.0 / 64.0]
        );
        let mut model = cube_model();
        model["elements"][0]["rotation"] = json!([0, 90, 0]);
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        let document = parse(&path, &BTreeMap::new()).unwrap();
        let (min, max) = bounds(&document);
        for (a, b) in min
            .into_iter()
            .zip([0.0, 0.0, 0.0])
            .chain(max.into_iter().zip([2.0, 1.0, 1.0]))
        {
            assert!((a - b).abs() < 1e-5);
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_mtr3_rejects_uvs_that_would_sample_another_atlas_tile() {
        let mut model = cube_model();
        let (root, path) = write_model(&model);
        let document = parse(&path, &BTreeMap::new()).unwrap();
        let layer: ModelLayer =
            serde_json::from_value(json!({"id":"layer","name":"Body","assetId":"asset"})).unwrap();
        assert!(native_model(&fs::read(&path).unwrap(), &document.parts, &layer, false).is_ok());
        assert!(
            native_model(&fs::read(&path).unwrap(), &document.parts, &layer, true)
                .unwrap_err()
                .contains("repeating UVs")
        );
        model["resolution"]["width"] = json!(128);
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        assert!(native_model(&fs::read(&path).unwrap(), &document.parts, &layer, true).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_mtr_bogie_has_expected_scale_and_texture_dependency() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/mtr4-4.0.5/bogie_1.bbmodel");
        let document = parse(&path, &BTreeMap::new()).unwrap();
        assert_eq!(document.parts.len(), 1);
        assert_eq!(document.parts[0].name, "bogie");
        assert_eq!(
            bounds(&document),
            ([0.0, -0.96875, -2.0], [0.875, -0.0625, 2.0])
        );
        let (references, embedded) = dependencies(&path, &BTreeMap::new()).unwrap();
        assert!(embedded.is_empty());
        assert_eq!(references.len(), 1);
        assert!(references[0].is_file());
    }
    #[test]
    fn embedded_textures_and_renamed_external_dependencies_are_resolved() {
        let mut model = cube_model();
        let (root, path) = write_model(&model);
        let mut image = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        model["textures"] = json!([{"name":"paint.png","source":format!("data:image/png;base64,{}",STANDARD.encode(image.get_ref()))}]);
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        let (refs, embedded) = dependencies(&path, &BTreeMap::new()).unwrap();
        assert!(refs.is_empty());
        assert_eq!(embedded[0].1, *image.get_ref());
        assert_eq!(
            parse(&path, &BTreeMap::new()).unwrap().materials[0]
                .texture
                .as_deref(),
            Some("paint.png")
        );
        model["textures"] = json!([{"name":"missing.png","relative_path":"old/path/missing.png"}]);
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        let chosen = root.join("replacement.png");
        fs::write(&chosen, image.into_inner()).unwrap();
        let overrides =
            BTreeMap::from([("missing.png".into(), chosen.to_string_lossy().into_owned())]);
        assert_eq!(dependencies(&path, &overrides).unwrap().0, vec![chosen]);
        assert_eq!(
            parse(&path, &overrides).unwrap().materials[0]
                .texture
                .as_deref(),
            Some("replacement.png")
        );
        model["textures"][0]["source"] = json!("data:image/png;base64,broken");
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        assert!(dependencies(&path, &BTreeMap::new()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn blockbench_five_groups_are_resolved_and_invalid_meshes_are_rejected() {
        let mut model = cube_model();
        model["groups"] =
            json!([{"name":"Body","uuid":"body","origin":[16,0,0],"rotation":[0,25,0]}]);
        model["outliner"] = json!([{"uuid":"body","children":["cube"]}]);
        let (root, path) = write_model(&model);
        let document = parse(&path, &BTreeMap::new()).unwrap();
        assert_eq!(document.parts[0].name, "Body");
        assert_eq!(bounds(&document), ([0.0, 0.0, -2.0], [1.0, 1.0, 0.0]));
        assert!(!document.warnings.is_empty());
        model["elements"][0]["type"] = json!("mesh");
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        assert!(parse(&path, &BTreeMap::new())
            .unwrap_err()
            .contains("cuboid"));
        model["elements"][0]["type"] = json!("cube");
        model["outliner"][0]["children"] = json!(["missing"]);
        fs::write(&path, serde_json::to_vec(&model).unwrap()).unwrap();
        assert!(parse(&path, &BTreeMap::new()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
