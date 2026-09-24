use crate::material::{AlphaMode, MaterialProperties, TextureChannel};
use crate::domain::{slugify, ModelFormat, ModelPartSummary};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::{BufRead, BufReader}, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDocument {
    pub parts: Vec<ModelPart>,
    pub materials: Vec<ModelMaterial>,
    pub warnings: Vec<String>,
    #[serde(default)] pub uv_origin_top_left: bool,
}

impl ModelDocument {
    pub fn default_flip_v(&self) -> bool { !self.uv_origin_top_left }
    pub fn flip_v(&self, layer_flip_v: bool) -> bool { self.default_flip_v() ^ layer_flip_v }
}

pub fn is_metasequoia_obj(bytes: &[u8]) -> bool {
    bytes.starts_with(b"# Created by Metasequoia")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPart {
    pub id: String,
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMaterial {
    pub id: String,
    pub name: String,
    pub color: [f32; 4],
    pub texture: Option<String>,
    #[serde(default)] pub properties: crate::material::MaterialProperties,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAnalysis {
    pub format: ModelFormat,
    pub missing_dependencies: Vec<String>,
    pub parts: Vec<ModelPartSummary>,
    pub warnings: Vec<String>,
}

pub fn model_format(path: &Path) -> Result<ModelFormat, String> {
    match path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "obj" => Ok(ModelFormat::Obj), "fbx" => Ok(ModelFormat::Fbx), "mqo" => Ok(ModelFormat::Mqo),
        _ => Err("Choose an OBJ, FBX, or MQO model file.".into()),
    }
}

pub fn analyze(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ImportAnalysis, String> {
    let format = model_format(path)?;
    let missing_dependencies: Vec<String> = referenced_files(path, overrides)?.into_iter().filter(|item| !item.exists()).map(|item| item.to_string_lossy().into_owned()).collect();
    // The import dialog only needs missing files. Geometry is parsed once during import.
    Ok(ImportAnalysis { format, missing_dependencies, parts: Vec::new(), warnings: Vec::new() })
}

pub fn parse(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let document = match model_format(path)? {
        ModelFormat::Obj => parse_obj(path, overrides)?,
        ModelFormat::Fbx => parse_fbx(path, overrides)?,
        ModelFormat::Mqo => parse_mqo(path, overrides)?,
    };
    finish_document(document)
}

pub fn parse_with_dependencies(path: &Path, overrides: &BTreeMap<String, String>) -> Result<(ModelDocument, Vec<PathBuf>, Vec<(String, Vec<u8>)>), String> {
    if model_format(path)? != ModelFormat::Fbx {
        return Ok((parse(path, overrides)?, referenced_files(path, overrides)?, Vec::new()));
    }
    let scene = load_fbx(path)?;
    let document = finish_document(parse_fbx_scene(&scene, overrides)?)?;
    let references = referenced_fbx(path, overrides, &scene);
    let embedded = embedded_fbx(&scene);
    Ok((document, references, embedded))
}

fn finish_document(mut document: ModelDocument) -> Result<ModelDocument, String> {
    let mut ids = BTreeSet::new();
    for (index, part) in document.parts.iter_mut().enumerate() {
        if !ids.insert(part.id.clone()) { part.id = format!("{}-{index}", part.id); ids.insert(part.id.clone()); }
    }
    if document.parts.iter().any(|part| part.material.is_none()) {
        let index = document.materials.len();
        document.materials.push(ModelMaterial { id: format!("material-{index}"), name: "Default".into(), color: [1.0;4], texture: None, properties: Default::default() });
        for part in &mut document.parts { if part.material.is_none() { part.material = Some(index); } }
    }
    validate_document(&document)?;
    Ok(document)
}

pub fn summaries(document: &ModelDocument) -> Vec<ModelPartSummary> {
    document.parts.iter().map(|part| ModelPartSummary { id: part.id.clone(), name: part.name.clone(), triangle_count: part.indices.len() / 3 }).collect()
}

pub fn referenced_files(path: &Path, overrides: &BTreeMap<String, String>) -> Result<Vec<PathBuf>, String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut files = BTreeSet::new();
    match model_format(path)? {
        ModelFormat::Obj => {
            let source = BufReader::new(fs::File::open(path).map_err(|e| format!("Unable to read OBJ: {e}"))?);
            for line in source.lines() {
                let line = line.map_err(|e| format!("Unable to read OBJ: {e}"))?;
                let line = line.trim();
                if let Some(name) = line.strip_prefix("mtllib ") {
                    let requested = name.trim(); let key = Path::new(requested).file_name().and_then(|value|value.to_str()).unwrap_or(requested); let mtl = overrides.get(key).map(PathBuf::from).unwrap_or_else(||parent.join(requested)); files.insert(mtl.clone());
                    if let Ok(mtl_text) = fs::read_to_string(&mtl) {
                        for mtl_line in mtl_text.lines().map(str::trim) {
                            if let Some(name) = ["map_Kd ", "norm ", "map_Pm ", "map_Pr ", "map_Ke ", "map_AO "].iter().find_map(|prefix| mtl_line.strip_prefix(prefix)) { let requested=name.split_whitespace().last().unwrap_or_default();let key=Path::new(requested).file_name().and_then(|value|value.to_str()).unwrap_or(requested);files.insert(overrides.get(key).map(PathBuf::from).unwrap_or_else(||mtl.parent().unwrap_or(parent).join(requested))); }
                        }
                    }
                }
            }
        }
        ModelFormat::Mqo => {
            let text = fs::read_to_string(path).map_err(|e| format!("Unable to read MQO: {e}"))?;
            for capture in quoted_values_after(&text, "tex(") { let key=Path::new(&capture).file_name().and_then(|value|value.to_str()).unwrap_or(&capture);files.insert(overrides.get(key).map(PathBuf::from).unwrap_or_else(||parent.join(capture))); }
        }
        ModelFormat::Fbx => {
            let scene = load_fbx(path)?;
            return Ok(referenced_fbx(path, overrides, &scene));
        }
    }
    Ok(files.into_iter().collect())
}

fn referenced_fbx(path: &Path, overrides: &BTreeMap<String, String>, scene: &ufbx::SceneRoot) -> Vec<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    scene.textures.iter().filter(|texture| texture.content.is_empty()).filter_map(|texture| {
        let requested = if texture.relative_filename.is_empty() { texture.filename.to_string() } else { texture.relative_filename.to_string() };
        if requested.is_empty() { return None; }
        let key = Path::new(&requested).file_name().and_then(|value| value.to_str()).unwrap_or(&requested);
        Some(overrides.get(key).map(PathBuf::from).unwrap_or_else(|| parent.join(requested)))
    }).collect::<BTreeSet<_>>().into_iter().collect()
}

fn embedded_fbx(scene: &ufbx::SceneRoot) -> Vec<(String, Vec<u8>)> {
    scene.textures.iter().filter_map(|texture| {
        if texture.content.is_empty() { return None; }
        let source = if texture.relative_filename.is_empty() { texture.filename.to_string() } else { texture.relative_filename.to_string() };
        let name = Path::new(&source).file_name()?.to_str()?.to_string();
        Some((name, texture.content.to_vec()))
    }).collect()
}

fn parse_obj(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let options = tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut source = BufReader::new(fs::File::open(path).map_err(|e| format!("Unable to read OBJ: {e}"))?);
    let uv_origin_top_left = is_metasequoia_obj(source.fill_buf().map_err(|e| format!("Unable to read OBJ: {e}"))?);
    let (models, materials) = tobj::load_obj_buf(&mut source, &options, |material_path| {
        let name = material_path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        let resolved = overrides.get(name).map(PathBuf::from).unwrap_or_else(|| parent.join(material_path));
        let file = fs::File::open(resolved).map_err(|_| tobj::LoadError::OpenFileFailed)?;
        tobj::load_mtl_buf(&mut BufReader::new(file))
    }).map_err(|e| format!("Unable to parse OBJ: {e}"))?;
    let materials = materials.unwrap_or_default().into_iter().enumerate().map(|(index, material)| {
        let mut properties = MaterialProperties::default();
        properties.metalness = material.unknown_param.get("Pm").and_then(|v| v.parse::<f32>().ok()).map(|v|v.clamp(0.0,1.0));
        properties.roughness = material.unknown_param.get("Pr").and_then(|v|v.parse::<f32>().ok()).or_else(||material.shininess.map(|v|(2.0/(v.max(0.0)+2.0)).sqrt())).map(|v|v.clamp(0.0,1.0));
        properties.emissive = material.unknown_param.get("Ke").and_then(|value| { let values:Vec<f32>=value.split_whitespace().filter_map(|v|v.parse().ok()).collect(); (values.len()==3).then(||[values[0].clamp(0.0,1.0),values[1].clamp(0.0,1.0),values[2].clamp(0.0,1.0)]) });
        for (channel, name) in [(TextureChannel::Normal, material.unknown_param.get("norm")), (TextureChannel::Metalness, material.unknown_param.get("map_Pm")), (TextureChannel::Roughness, material.unknown_param.get("map_Pr")), (TextureChannel::Emissive, material.unknown_param.get("map_Ke")), (TextureChannel::Occlusion, material.unknown_param.get("map_AO"))] {
            if let Some(name) = name { let name=name.split_whitespace().last().unwrap_or(name); properties.maps.insert(channel,name.into()); }
        }
        if properties.maps.contains_key(&TextureChannel::Metalness) && properties.metalness.is_none() { properties.metalness=Some(1.0); }
        if properties.maps.contains_key(&TextureChannel::Roughness) && properties.roughness.is_none() { properties.roughness=Some(1.0); }
        if properties.maps.contains_key(&TextureChannel::Emissive) && properties.emissive.is_none() { properties.emissive=Some([1.0;3]); }
        let texture = material.diffuse_texture.map(|name| { let key = name.rsplit(['/', '\\']).next().unwrap_or(&name); overrides.get(key).cloned().unwrap_or(name) });
        let diffuse = material.diffuse.unwrap_or([0.8, 0.8, 0.8]);
        for name in properties.maps.values_mut() { let key=name.rsplit(['/', '\\']).next().unwrap_or(name);if let Some(replacement)=overrides.get(key) { *name=replacement.clone(); } }
        ModelMaterial { id: format!("material-{index}"), name: material.name, color: [diffuse[0], diffuse[1], diffuse[2], material.dissolve.unwrap_or(1.0)], texture, properties }
    }).collect();
    let parts = models.into_iter().enumerate().map(|(part_index, model)| {
        let mesh = model.mesh;
        ModelPart {
            id: format!("part-{}", slugify(&model.name, &part_index.to_string())), name: if model.name.is_empty() { format!("Part {}", part_index + 1) } else { model.name },
            positions: mesh.positions.chunks_exact(3).map(|v| [-v[0], v[1], v[2]]).collect(),
            normals: mesh.normals.chunks_exact(3).map(|v| [-v[0], v[1], v[2]]).collect(),
            texcoords: mesh.texcoords.chunks_exact(2).map(|v| [v[0], v[1]]).collect(),
            indices: mesh.indices.chunks_exact(3).flat_map(|v| [v[0], v[2], v[1]]).collect(), material: mesh.material_id,
        }
    }).collect();
    Ok(ModelDocument { parts, materials, warnings: Vec::new(), uv_origin_top_left })
}

fn load_fbx(path: &Path) -> Result<ufbx::SceneRoot, String> {
    let options = ufbx::LoadOpts { generate_missing_normals: true, ignore_missing_external_files: true, target_axes: ufbx::CoordinateAxes::right_handed_y_up(), target_unit_meters: 1.0, ..Default::default() };
    ufbx::load_file(&path.to_string_lossy(), options).map_err(|e| format!("Unable to parse FBX: {e:?}"))
}

fn parse_fbx(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let scene = load_fbx(path)?;
    parse_fbx_scene(&scene, overrides)
}

fn parse_fbx_scene(scene: &ufbx::SceneRoot, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let mut warnings = Vec::new();
    if !scene.skin_deformers.is_empty() { warnings.push("Skinning was detected. The imported preview uses the static mesh pose.".into()); }
    if !scene.blend_deformers.is_empty() { warnings.push("Morph targets were detected and skipped.".into()); }
    if scene.anim_stacks.len() > 1 || !scene.anim_curves.is_empty() { warnings.push("Animation timelines were detected and skipped.".into()); }
    let materials = scene.materials.iter().enumerate().map(|(index, material)| {
        let map = if material.pbr.base_color.has_value || material.pbr.base_color.texture.is_some() { &material.pbr.base_color } else { &material.fbx.diffuse_color };
        let value = map.value_vec4;
        let mut color = if map.has_value { [value.x as f32, value.y as f32, value.z as f32, if material.pbr.opacity.has_value { material.pbr.opacity.value_vec4.x as f32 } else { 1.0 }] } else { [1.0;4] };
        color[3]=if material.pbr.opacity.has_value { (material.pbr.opacity.value_vec4.x as f32).clamp(0.0,1.0) } else if material.fbx.transparency_factor.has_value { (1.0-material.fbx.transparency_factor.value_vec4.x as f32).clamp(0.0,1.0) } else {1.0};
        let texture = map.texture.as_ref().and_then(|texture| {
            let name = if texture.relative_filename.is_empty() { texture.filename.to_string() } else { texture.relative_filename.to_string() };
            if name.is_empty() { None } else { let key = name.rsplit(['/', '\\']).next().unwrap_or(&name); Some(overrides.get(key).cloned().unwrap_or(name)) }
        });
        let scalar = |map: &ufbx::MaterialMap| map.has_value.then_some((map.value_vec4.x as f32).clamp(0.0,1.0));
        let mut properties = MaterialProperties { metalness: scalar(&material.pbr.metalness), roughness: scalar(&material.pbr.roughness), ..Default::default() };
        if properties.roughness.is_none() { properties.roughness=scalar(&material.pbr.glossiness).map(|v|1.0-v).or_else(||material.fbx.specular_exponent.has_value.then(||(2.0/(material.fbx.specular_exponent.value_vec4.x.max(0.0) as f32+2.0)).sqrt())); }
        let emission=if material.pbr.emission_color.has_value || material.pbr.emission_color.texture.is_some() { &material.pbr.emission_color } else { &material.fbx.emission_color };
        let factor=if material.pbr.emission_factor.has_value {material.pbr.emission_factor.value_vec4.x as f32} else if material.fbx.emission_factor.has_value {material.fbx.emission_factor.value_vec4.x as f32} else {1.0};
        if emission.has_value { let v=emission.value_vec4;properties.emissive=Some([v.x as f32,v.y as f32,v.z as f32].map(|v|(v*factor).clamp(0.0,1.0))); }
        let normal=if material.pbr.normal_map.texture.is_some() { &material.pbr.normal_map } else { &material.fbx.normal_map };
        for (channel,map) in [(TextureChannel::Normal,normal),(TextureChannel::Metalness,&material.pbr.metalness),(TextureChannel::Roughness,&material.pbr.roughness),(TextureChannel::Emissive,emission),(TextureChannel::Occlusion,&material.pbr.ambient_occlusion)] {
            if let Some(texture)=map.texture.as_ref() { let name=if texture.relative_filename.is_empty(){texture.filename.to_string()}else{texture.relative_filename.to_string()};if !name.is_empty(){properties.maps.insert(channel,name);} }
        }
        if properties.maps.contains_key(&TextureChannel::Metalness) && properties.metalness.is_none() { properties.metalness=Some(1.0); }
        if properties.maps.contains_key(&TextureChannel::Roughness) && properties.roughness.is_none() { properties.roughness=Some(1.0); }
        if properties.maps.contains_key(&TextureChannel::Emissive) && properties.emissive.is_none() { properties.emissive=Some([1.0;3]); }
        for name in properties.maps.values_mut() { let key=name.rsplit(['/', '\\']).next().unwrap_or(name);if let Some(replacement)=overrides.get(key) { *name=replacement.clone(); } }
        ModelMaterial { id: format!("material-{index}"), name: material.element.name.to_string(), color, texture, properties }
    }).collect();
    let mut parts = Vec::new();
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let name = if node.element.name.is_empty() { format!("Part {}", node_index + 1) } else { node.element.name.to_string() };
        let mut groups: BTreeMap<Option<usize>, ModelPart> = BTreeMap::new();
        let normal_matrix = node.get_compatible_matrix_for_normals();
        let reverse_winding = ufbx::matrix_determinant(&node.geometry_to_world) >= 0.0;
        for (face_index, face) in mesh.faces.iter().enumerate() {
            let material = mesh.face_material.get(face_index).and_then(|index| node.materials.get(*index as usize).or_else(|| mesh.materials.get(*index as usize))).map(|material| material.element.typed_id as usize);
            let part = groups.entry(material).or_insert_with(|| ModelPart { id: format!("part-{node_index}-{}", material.map_or_else(|| "default".into(), |v| v.to_string())), name: name.clone(), positions:Vec::new(), normals:Vec::new(), texcoords:Vec::new(), indices:Vec::new(), material });
            let mut corners = vec![0u32; mesh.max_face_triangles.max(1) * 3];
            let triangle_count = mesh.triangulate_face(&mut corners, *face) as usize;
            for triangle in corners[..triangle_count * 3].chunks_exact(3) {
                let order = if reverse_winding { [triangle[0],triangle[2],triangle[1]] } else { [triangle[0],triangle[1],triangle[2]] };
                for corner in order {
                    let corner = corner as usize;
                    let position = ufbx::transform_position(&node.geometry_to_world, mesh.vertex_position[corner]);
                    part.positions.push([-(position.x as f32),position.y as f32,position.z as f32]);
                    if mesh.vertex_normal.exists { let normal = ufbx::transform_direction(&normal_matrix, mesh.vertex_normal[corner]); let length = (normal.x*normal.x+normal.y*normal.y+normal.z*normal.z).sqrt().max(1e-12); part.normals.push([(-normal.x/length) as f32,(normal.y/length) as f32,(normal.z/length) as f32]); }
                    if mesh.vertex_uv.exists { let uv=mesh.vertex_uv[corner]; part.texcoords.push([uv.x as f32,uv.y as f32]); }
                    part.indices.push((part.positions.len()-1) as u32);
                }
            }
        }
        parts.extend(groups.into_values().filter(|part| !part.indices.is_empty()));
    }
    if parts.is_empty() { return Err("The FBX file contains no static mesh geometry.".into()); }
    Ok(ModelDocument { parts, materials, warnings, uv_origin_top_left: false })
}

fn parse_mqo(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("Unable to read MQO: {e}"))?;
    let mut materials = Vec::new();
    if let Some(start) = text.find("Material ") {
        if let Some(open) = text[start..].find('{') { if let Some(close) = text[start + open..].find('}') {
            for (index, line) in text[start + open + 1..start + open + close].lines().map(str::trim).filter(|line| !line.is_empty()).enumerate() {
                let name = quoted(line).unwrap_or_else(|| format!("Material {index}")); let values = tuple_values(line, "col(").unwrap_or_default();
                let color = [*values.first().unwrap_or(&0.8), *values.get(1).unwrap_or(&0.8), *values.get(2).unwrap_or(&0.8), *values.get(3).unwrap_or(&1.0)];
                let texture = quoted_after(line, "tex(").map(|value| { let key = Path::new(&value).file_name().and_then(|item| item.to_str()).unwrap_or(&value); overrides.get(key).cloned().unwrap_or(value) });
                let power=tuple_values(line,"power(").and_then(|v|v.first().copied());
                let emission=tuple_values(line,"emi(").and_then(|v|v.first().copied());
                let properties=MaterialProperties { roughness:power.map(|v|(2.0/(v.max(0.0)+2.0)).sqrt()), emissive:emission.map(|v|[color[0],color[1],color[2]].map(|c|(c*v).clamp(0.0,1.0))), ..Default::default() };
                materials.push(ModelMaterial { id: format!("material-{index}"), name, color, texture, properties });
            }
        }}
    }
    let mut parts = Vec::new(); let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("Object ") {
        let start = cursor + relative; let Some(open) = text[start..].find('{') else { break }; let body_start = start + open + 1;
        let Some(close) = find_matching_brace(&text, start + open) else { break }; let header = &text[start..start + open]; let body = &text[body_start..close];
        let name = quoted(header).unwrap_or_else(|| format!("Part {}", parts.len() + 1));
        let mut vertices = Vec::new();
        let mut groups: BTreeMap<Option<usize>, ModelPart> = BTreeMap::new();
        if let Some(vertex_at) = body.find("vertex ") { if let Some(v_open) = body[vertex_at..].find('{') { if let Some(v_close) = body[vertex_at + v_open..].find('}') {
            for line in body[vertex_at + v_open + 1..vertex_at + v_open + v_close].lines() { let nums: Vec<f32> = line.split_whitespace().filter_map(|v| v.parse().ok()).collect(); if nums.len() >= 3 { vertices.push([-nums[0]*0.01,nums[1]*0.01,nums[2]*0.01]); } }
        }}}
        if let Some(face_at) = body.find("face ") { if let Some(f_open) = body[face_at..].find('{') { if let Some(f_close) = body[face_at + f_open..].find('}') {
            for line in body[face_at + f_open + 1..face_at + f_open + f_close].lines().map(str::trim) {
                let ids=tuple_values(line,"V(").unwrap_or_default(); if ids.len()<3 { continue; }
                if ids.iter().any(|v| !v.is_finite() || *v < 0.0 || v.fract()!=0.0 || *v as usize >= vertices.len()) { return Err(format!("Invalid vertex index in MQO object {name}.")); }
                let uv=tuple_values(line,"UV(").unwrap_or_default(); if !uv.is_empty() && uv.len()!=ids.len()*2 { return Err(format!("Invalid UV data in MQO object {name}.")); }
                let material=tuple_values(line,"M(").and_then(|v|v.first().copied()).filter(|v| *v>=0.0).map(|v|v as usize);
                let part=groups.entry(material).or_insert_with(|| ModelPart { id:format!("part-{}-{}",parts.len(),material.map_or_else(||"default".into(),|v|v.to_string())),name:name.clone(),positions:Vec::new(),normals:Vec::new(),texcoords:Vec::new(),indices:Vec::new(),material });
                for triangle in 1..ids.len()-1 { for corner in [0,triangle+1,triangle] {
                    part.positions.push(vertices[ids[corner] as usize]); part.indices.push((part.positions.len()-1) as u32);
                    part.texcoords.push(if uv.is_empty(){[0.0,0.0]}else{[uv[corner*2],uv[corner*2+1]]});
                }}
            }
        }}}
        parts.extend(groups.into_values()); cursor=close+1;
    }
    if parts.is_empty() { return Err("The MQO file contains no model objects.".into()); }
    Ok(ModelDocument { parts, materials, warnings: Vec::new(), uv_origin_top_left: false })
}

#[cfg(test)]
pub fn to_glb(document: &ModelDocument) -> Result<Vec<u8>, String> { to_glb_with_textures(document, &BTreeMap::new()) }

pub fn to_glb_with_textures(document: &ModelDocument, image_bytes: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    validate_document(document)?;
    let mut binary = Vec::new(); let mut views = Vec::new(); let mut accessors = Vec::new(); let mut meshes = Vec::new(); let mut nodes = Vec::new();
    for (index, part) in document.parts.iter().enumerate() {
        let position_view = append_f32_vec3(&mut binary, &part.positions, &mut views); let (min, max) = bounds(&part.positions);
        let position_accessor = accessors.len(); accessors.push(serde_json::json!({"bufferView":position_view,"componentType":5126,"count":part.positions.len(),"type":"VEC3","min":min,"max":max}));
        let normal_accessor = if part.normals.len() == part.positions.len() { let view = append_f32_vec3(&mut binary, &part.normals, &mut views); let value=accessors.len(); accessors.push(serde_json::json!({"bufferView":view,"componentType":5126,"count":part.normals.len(),"type":"VEC3"})); Some(value) } else { None };
        let uv_accessor = if part.texcoords.len() == part.positions.len() { let preview_uv: Vec<_> = part.texcoords.iter().map(|uv| [uv[0], if document.default_flip_v() { 1.0 - uv[1] } else { uv[1] }]).collect(); let view=append_f32_vec2(&mut binary,&preview_uv,&mut views); let value=accessors.len(); accessors.push(serde_json::json!({"bufferView":view,"componentType":5126,"count":part.texcoords.len(),"type":"VEC2"})); Some(value) } else { None };
        let index_view = append_u32(&mut binary, &part.indices, &mut views); let index_accessor=accessors.len(); accessors.push(serde_json::json!({"bufferView":index_view,"componentType":5125,"count":part.indices.len(),"type":"SCALAR"}));
        let mut attributes=serde_json::Map::new(); attributes.insert("POSITION".into(), position_accessor.into()); if let Some(v)=normal_accessor { attributes.insert("NORMAL".into(),v.into()); } if let Some(v)=uv_accessor { attributes.insert("TEXCOORD_0".into(),v.into()); }
        let mut primitive=serde_json::Map::new(); primitive.insert("attributes".into(),attributes.into()); primitive.insert("indices".into(),index_accessor.into()); if let Some(material)=part.material.filter(|v| *v<document.materials.len()){primitive.insert("material".into(),material.into());}
        meshes.push(serde_json::json!({"name":part.name,"primitives":[primitive]})); nodes.push(serde_json::json!({"name":part.name,"mesh":index,"extras":{"partId":part.id}}));
    }
    let mut images = Vec::new(); let mut textures = Vec::new(); let mut image_ids = BTreeMap::new();
    let mut embed = |bytes: &[u8]| {
        let hash=blake3::hash(bytes).to_hex().to_string();
        if let Some(index)=image_ids.get(&hash) { return *index; }
        align4(&mut binary);let offset=binary.len();binary.extend(bytes);
        let view=views.len();views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":bytes.len()}));
        let index=images.len();images.push(serde_json::json!({"bufferView":view,"mimeType":"image/png"}));textures.push(serde_json::json!({"source":index,"sampler":0}));image_ids.insert(hash,index);index
    };
    let mut materials = Vec::new();
    for material in &document.materials {
        let p=&material.properties;p.validate()?;
        let mut color=material.color;if let Some(opacity)=p.opacity {color[3]=opacity;}
        let mut definition = serde_json::json!({"name": material.name, "pbrMetallicRoughness":{"baseColorFactor":color,"metallicFactor":p.metalness.unwrap_or(0.0),"roughnessFactor":p.roughness.unwrap_or(0.8)},"emissiveFactor":p.emissive.unwrap_or([0.0;3]),"doubleSided":p.double_sided.unwrap_or(true),"extras":{"materialId":material.id}});
        let base=image_bytes.get(&material.id);
        if let Some(bytes)=base { definition["pbrMetallicRoughness"]["baseColorTexture"]=serde_json::json!({"index":embed(bytes)}); }
        let mode=p.alpha_mode.unwrap_or(if color[3]<1.0 {AlphaMode::Blend} else if base.is_some() {AlphaMode::Mask} else {AlphaMode::Opaque});
        definition["alphaMode"]=serde_json::to_value(mode).map_err(|e|e.to_string())?;
        if mode==AlphaMode::Mask {definition["alphaCutoff"]=p.alpha_cutoff.unwrap_or(0.1).into();}
        for (channel,key) in [(TextureChannel::Normal,"normalTexture"),(TextureChannel::Emissive,"emissiveTexture"),(TextureChannel::Occlusion,"occlusionTexture")] {
            if let Some(bytes)=image_bytes.get(&format!("{}:{}",material.id,channel.key())) {definition[key]=serde_json::json!({"index":embed(bytes)});if channel==TextureChannel::Normal {definition[key]["scale"]=p.normal_scale.unwrap_or(1.0).into();}}
        }
        let metal=image_bytes.get(&format!("{}:metalness",material.id));let rough=image_bytes.get(&format!("{}:roughness",material.id));
        if let Some(bytes)=crate::material::pack_metallic_roughness(metal,rough)? {definition["pbrMetallicRoughness"]["metallicRoughnessTexture"]=serde_json::json!({"index":embed(&bytes)});}
        materials.push(definition);
    }
    let mut definition = serde_json::json!({"asset":{"version":"2.0","generator":"MTR Pack Studio"},"scene":0,"scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>() }],"nodes":nodes,"meshes":meshes,"materials":materials,"images":images,"textures":textures,"samplers":[{"magFilter":9729,"minFilter":9987,"wrapS":10497,"wrapT":10497}],"buffers":[{"byteLength":binary.len()}],"bufferViews":views,"accessors":accessors});
    for key in ["materials", "images", "textures"] { if definition[key].as_array().is_some_and(Vec::is_empty) { definition.as_object_mut().unwrap().remove(key); } }
    if !definition.as_object().unwrap().contains_key("textures") { definition.as_object_mut().unwrap().remove("samplers"); }
    let json = serde_json::to_vec(&definition).map_err(|e| e.to_string())?;
    let mut json_chunk=json; while json_chunk.len()%4!=0 { json_chunk.push(b' '); } while binary.len()%4!=0 { binary.push(0); }
    let length=12+8+json_chunk.len()+8+binary.len(); let mut glb=Vec::with_capacity(length); glb.extend(0x46546C67u32.to_le_bytes()); glb.extend(2u32.to_le_bytes()); glb.extend((length as u32).to_le_bytes()); glb.extend((json_chunk.len() as u32).to_le_bytes()); glb.extend(0x4E4F534Au32.to_le_bytes()); glb.extend(json_chunk); glb.extend((binary.len() as u32).to_le_bytes()); glb.extend(0x004E4942u32.to_le_bytes()); glb.extend(binary); Ok(glb)
}

pub fn validate_document(document: &ModelDocument) -> Result<(), String> {
    for material in &document.materials { material.properties.validate()?; }
    if document.parts.is_empty() { return Err("The model contains no geometry.".into()); }
    for part in &document.parts {
        if part.positions.is_empty() || part.indices.is_empty() || part.indices.len() % 3 != 0 || part.indices.iter().any(|index| *index as usize >= part.positions.len()) { return Err(format!("Invalid triangles in part {}.", part.name)); }
        if part.positions.iter().flatten().chain(part.normals.iter().flatten()).chain(part.texcoords.iter().flatten()).any(|v| !v.is_finite()) { return Err(format!("Non-finite coordinates in part {}.", part.name)); }
        if !part.texcoords.is_empty() && part.texcoords.len() != part.positions.len() { return Err(format!("Invalid UV coordinates in part {}.", part.name)); }
        if part.material.is_some_and(|index| index >= document.materials.len()) { return Err(format!("Invalid material in part {}.", part.name)); }
    }
    Ok(())
}

fn align4(data:&mut Vec<u8>){while data.len()%4!=0{data.push(0)}}
fn append_f32_vec3(data:&mut Vec<u8>,values:&[[f32;3]],views:&mut Vec<serde_json::Value>)->usize{align4(data);let offset=data.len();for v in values{for x in v{data.extend(x.to_le_bytes())}}let index=views.len();views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()-offset,"target":34962}));index}
fn append_f32_vec2(data:&mut Vec<u8>,values:&[[f32;2]],views:&mut Vec<serde_json::Value>)->usize{align4(data);let offset=data.len();for v in values{for x in v{data.extend(x.to_le_bytes())}}let index=views.len();views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()-offset,"target":34962}));index}
fn append_u32(data:&mut Vec<u8>,values:&[u32],views:&mut Vec<serde_json::Value>)->usize{align4(data);let offset=data.len();for v in values{data.extend(v.to_le_bytes())}let index=views.len();views.push(serde_json::json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()-offset,"target":34963}));index}
fn bounds(values:&[[f32;3]])->([f32;3],[f32;3]){let mut min=[f32::INFINITY;3];let mut max=[f32::NEG_INFINITY;3];for v in values{for i in 0..3{min[i]=min[i].min(v[i]);max[i]=max[i].max(v[i]);}}if values.is_empty(){([0.0;3],[0.0;3])}else{(min,max)}}
fn quoted(value:&str)->Option<String>{let start=value.find('"')?+1;let end=value[start..].find('"')?+start;Some(value[start..end].into())}
fn quoted_after(value:&str,needle:&str)->Option<String>{let start=value.find(needle)?+needle.len();quoted(&value[start..])}
fn quoted_values_after(value:&str,needle:&str)->Vec<String>{let mut result=Vec::new();let mut cursor=0;while let Some(at)=value[cursor..].find(needle){let start=cursor+at+needle.len();if let Some(item)=quoted(&value[start..]){result.push(item);}cursor=start+1;}result}
fn tuple_values(value:&str,needle:&str)->Option<Vec<f32>>{let start=value.find(needle)?+needle.len();let end=value[start..].find(')')?+start;Some(value[start..end].split_whitespace().filter_map(|v|v.parse().ok()).collect())}
fn find_matching_brace(value:&str,open:usize)->Option<usize>{let mut depth=0;for(index,c)in value[open..].char_indices(){if c=='{'{depth+=1}else if c=='}'{depth-=1;if depth==0{return Some(open+index)}}}None}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn glb_has_valid_header() { let doc=ModelDocument{parts:vec![ModelPart{id:"part".into(),name:"Part".into(),positions:vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]],normals:vec![],texcoords:vec![],indices:vec![0,1,2],material:None}],materials:vec![],warnings:vec![],uv_origin_top_left:false};let glb=to_glb(&doc).unwrap();assert_eq!(&glb[..4],b"glTF");assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap())as usize,glb.len()); }
    #[test] fn parses_obj_and_mqo_triangles() {
        let root=std::env::temp_dir().join(format!("mtr-model-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let obj=root.join("test.obj");std::fs::write(&obj,"o shell\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();let parsed=parse(&obj,&BTreeMap::new()).unwrap();assert_eq!(parsed.parts[0].indices.len(),3);
        let mqo=root.join("test.mqo");std::fs::write(&mqo,"Metasequoia Document\nFormat Text Ver 1.0\nObject \"shell\" {\n vertex 3 {\n0 0 0\n1 0 0\n0 1 0\n}\n face 1 {\n3 V(0 1 2)\n}\n}\nEof\n").unwrap();let parsed=parse(&mqo,&BTreeMap::new()).unwrap();assert_eq!(parsed.parts[0].indices.len(),3);std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn dependency_analysis_does_not_parse_geometry() {
        let root=std::env::temp_dir().join(format!("mtr-analysis-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let obj=root.join("broken.obj");std::fs::write(&obj,"mtllib missing.mtl\nf 1 2 3\n").unwrap();
        let analysis=analyze(&obj,&BTreeMap::new()).unwrap();
        assert_eq!(analysis.missing_dependencies,vec![root.join("missing.mtl").to_string_lossy().to_string()]);
        assert!(analysis.parts.is_empty());
        assert!(parse(&obj,&BTreeMap::new()).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
