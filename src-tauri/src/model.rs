use crate::domain::{slugify, ModelFormat, ModelPartSummary};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, fs, io::BufReader, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDocument {
    pub parts: Vec<ModelPart>,
    pub materials: Vec<ModelMaterial>,
    pub warnings: Vec<String>,
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
    if !missing_dependencies.is_empty() { return Ok(ImportAnalysis { format, missing_dependencies, parts: Vec::new(), warnings: Vec::new() }); }
    let document = parse(path, overrides)?;
    Ok(ImportAnalysis { format, missing_dependencies, parts: summaries(&document), warnings: document.warnings })
}

pub fn parse(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    match model_format(path)? {
        ModelFormat::Obj => parse_obj(path, overrides),
        ModelFormat::Fbx => parse_fbx(path, overrides),
        ModelFormat::Mqo => parse_mqo(path, overrides),
    }
}

pub fn summaries(document: &ModelDocument) -> Vec<ModelPartSummary> {
    document.parts.iter().map(|part| ModelPartSummary { id: part.id.clone(), name: part.name.clone(), triangle_count: part.indices.len() / 3 }).collect()
}

pub fn referenced_files(path: &Path, overrides: &BTreeMap<String, String>) -> Result<Vec<PathBuf>, String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut files = BTreeSet::new();
    match model_format(path)? {
        ModelFormat::Obj => {
            for line in text.lines().map(str::trim) {
                if let Some(name) = line.strip_prefix("mtllib ") {
                    let requested = name.trim(); let key = Path::new(requested).file_name().and_then(|value|value.to_str()).unwrap_or(requested); let mtl = overrides.get(key).map(PathBuf::from).unwrap_or_else(||parent.join(requested)); files.insert(mtl.clone());
                    if let Ok(mtl_text) = fs::read_to_string(&mtl) {
                        for mtl_line in mtl_text.lines().map(str::trim) {
                            if let Some(name) = mtl_line.strip_prefix("map_Kd ") { let requested=name.split_whitespace().last().unwrap_or_default();let key=Path::new(requested).file_name().and_then(|value|value.to_str()).unwrap_or(requested);files.insert(overrides.get(key).map(PathBuf::from).unwrap_or_else(||mtl.parent().unwrap_or(parent).join(requested))); }
                        }
                    }
                }
            }
        }
        ModelFormat::Mqo => {
            for capture in quoted_values_after(&text, "tex(") { let key=Path::new(&capture).file_name().and_then(|value|value.to_str()).unwrap_or(&capture);files.insert(overrides.get(key).map(PathBuf::from).unwrap_or_else(||parent.join(capture))); }
        }
        ModelFormat::Fbx => {
            let scene = load_fbx(path)?;
            for texture in scene.textures.iter() {
                if !texture.content.is_empty() { continue; }
                let requested = if texture.relative_filename.is_empty() { texture.filename.to_string() } else { texture.relative_filename.to_string() };
                if requested.is_empty() { continue; }
                let key = Path::new(&requested).file_name().and_then(|value| value.to_str()).unwrap_or(&requested);
                files.insert(overrides.get(key).map(PathBuf::from).unwrap_or_else(|| parent.join(requested)));
            }
        }
    }
    Ok(files.into_iter().collect())
}

pub fn embedded_dependencies(path: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    if model_format(path)? != ModelFormat::Fbx { return Ok(Vec::new()); }
    let scene = load_fbx(path)?;
    Ok(scene.textures.iter().filter_map(|texture| {
        if texture.content.is_empty() { return None; }
        let source = if texture.relative_filename.is_empty() { texture.filename.to_string() } else { texture.relative_filename.to_string() };
        let name = Path::new(&source).file_name()?.to_str()?.to_string();
        Some((name, texture.content.to_vec()))
    }).collect())
}

fn parse_obj(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let options = tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut source = BufReader::new(fs::File::open(path).map_err(|e| format!("Unable to read OBJ: {e}"))?);
    let (models, materials) = tobj::load_obj_buf(&mut source, &options, |material_path| {
        let name = material_path.file_name().and_then(|value| value.to_str()).unwrap_or_default();
        let resolved = overrides.get(name).map(PathBuf::from).unwrap_or_else(|| parent.join(material_path));
        let file = fs::File::open(resolved).map_err(|_| tobj::LoadError::OpenFileFailed)?;
        tobj::load_mtl_buf(&mut BufReader::new(file))
    }).map_err(|e| format!("Unable to parse OBJ: {e}"))?;
    let materials = materials.unwrap_or_default().into_iter().enumerate().map(|(index, material)| {
        let texture = material.diffuse_texture.map(|name| overrides.get(&name).cloned().unwrap_or(name));
        let diffuse = material.diffuse.unwrap_or([0.8, 0.8, 0.8]);
        ModelMaterial { id: format!("material-{index}"), name: material.name, color: [diffuse[0], diffuse[1], diffuse[2], material.dissolve.unwrap_or(1.0)], texture }
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
    Ok(ModelDocument { parts, materials, warnings: Vec::new() })
}

fn load_fbx(path: &Path) -> Result<ufbx::SceneRoot, String> {
    let options = ufbx::LoadOpts { generate_missing_normals: true, ignore_missing_external_files: true, target_axes: ufbx::CoordinateAxes::right_handed_y_up(), target_unit_meters: 1.0, ..Default::default() };
    ufbx::load_file(&path.to_string_lossy(), options).map_err(|e| format!("Unable to parse FBX: {e:?}"))
}

fn parse_fbx(path: &Path, overrides: &BTreeMap<String, String>) -> Result<ModelDocument, String> {
    let scene = load_fbx(path)?;
    let mut warnings = Vec::new();
    if !scene.skin_deformers.is_empty() { warnings.push("Skinning was detected. The imported preview uses the static mesh pose.".into()); }
    if !scene.blend_deformers.is_empty() { warnings.push("Morph targets were detected and skipped.".into()); }
    if scene.anim_stacks.len() > 1 || !scene.anim_curves.is_empty() { warnings.push("Animation timelines were detected and skipped.".into()); }
    let materials = scene.materials.iter().enumerate().map(|(index, material)| ModelMaterial { id: format!("material-{index}"), name: material.element.name.to_string(), color: [0.8, 0.8, 0.8, 1.0], texture: material.textures.first().and_then(|entry| { let name = if entry.texture.relative_filename.is_empty() { entry.texture.filename.to_string() } else { entry.texture.relative_filename.to_string() }; if name.is_empty() { None } else { let key = Path::new(&name).file_name().and_then(|value| value.to_str()).unwrap_or(&name); Some(overrides.get(key).cloned().unwrap_or(name)) } }) }).collect();
    let mut parts = Vec::new();
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let Some(mesh) = node.mesh.as_ref() else { continue };
        let mut positions = Vec::new(); let mut normals = Vec::new(); let mut texcoords = Vec::new(); let mut indices = Vec::new();
        let normal_matrix = node.get_compatible_matrix_for_normals();
        for face in mesh.faces.iter() {
            let mut triangle_indices = vec![0u32; mesh.max_face_triangles.max(1) * 3];
            let triangle_count = mesh.triangulate_face(&mut triangle_indices, *face) as usize;
            for &corner in triangle_indices[..triangle_count * 3].iter() {
                let corner = corner as usize; let position = ufbx::transform_position(&node.geometry_to_world, mesh.vertex_position[corner]);
                positions.push([-(position.x as f32), position.y as f32, position.z as f32]);
                if mesh.vertex_normal.exists { let normal = ufbx::transform_direction(&normal_matrix, mesh.vertex_normal[corner]); normals.push([-(normal.x as f32), normal.y as f32, normal.z as f32]); }
                if mesh.vertex_uv.exists { let uv = mesh.vertex_uv[corner]; texcoords.push([uv.x as f32, uv.y as f32]); }
                indices.push((positions.len() - 1) as u32);
            }
        }
        for triangle in indices.chunks_exact_mut(3) { triangle.swap(1, 2); }
        let name = if node.element.name.is_empty() { format!("Part {}", node_index + 1) } else { node.element.name.to_string() };
        parts.push(ModelPart { id: format!("part-{}", slugify(&name, &node_index.to_string())), name, positions, normals, texcoords, indices, material: mesh.materials.first().map(|material| material.element.typed_id as usize) });
    }
    if parts.is_empty() { return Err("The FBX file contains no static mesh geometry.".into()); }
    Ok(ModelDocument { parts, materials, warnings })
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
                materials.push(ModelMaterial { id: format!("material-{index}"), name, color, texture });
            }
        }}
    }
    let mut parts = Vec::new(); let mut cursor = 0;
    while let Some(relative) = text[cursor..].find("Object ") {
        let start = cursor + relative; let Some(open) = text[start..].find('{') else { break }; let body_start = start + open + 1;
        let Some(close) = find_matching_brace(&text, start + open) else { break }; let header = &text[start..start + open]; let body = &text[body_start..close];
        let name = quoted(header).unwrap_or_else(|| format!("Part {}", parts.len() + 1)); let mut vertices = Vec::new(); let mut indices = Vec::new(); let mut texcoords = Vec::new(); let mut material = None;
        if let Some(vertex_at) = body.find("vertex ") { if let Some(v_open) = body[vertex_at..].find('{') { if let Some(v_close) = body[vertex_at + v_open..].find('}') {
            for line in body[vertex_at + v_open + 1..vertex_at + v_open + v_close].lines() { let nums: Vec<f32> = line.split_whitespace().filter_map(|v| v.parse().ok()).collect(); if nums.len() >= 3 { vertices.push([-nums[0], nums[1], nums[2]]); } }
        }}}
        if let Some(face_at) = body.find("face ") { if let Some(f_open) = body[face_at..].find('{') { if let Some(f_close) = body[face_at + f_open..].find('}') {
            for line in body[face_at + f_open + 1..face_at + f_open + f_close].lines().map(str::trim) {
                let vertex_ids: Vec<u32> = tuple_values(line, "V(").unwrap_or_default().into_iter().map(|v| v as u32).collect(); if vertex_ids.len() < 3 { continue; }
                let uv = tuple_values(line, "UV(").unwrap_or_default(); material = tuple_values(line, "M(").and_then(|v| v.first().copied()).map(|v| v as usize);
                for triangle in 1..vertex_ids.len() - 1 { indices.extend([vertex_ids[0], vertex_ids[triangle + 1], vertex_ids[triangle]]); }
                if !uv.is_empty() && texcoords.is_empty() { texcoords.resize(vertices.len(), [0.0, 0.0]); for (i, vertex) in vertex_ids.iter().enumerate() { if i * 2 + 1 < uv.len() { texcoords[*vertex as usize] = [uv[i * 2], uv[i * 2 + 1]]; } } }
            }
        }}}
        parts.push(ModelPart { id: format!("part-{}", slugify(&name, &parts.len().to_string())), name, positions: vertices, normals: Vec::new(), texcoords, indices, material }); cursor = close + 1;
    }
    if parts.is_empty() { return Err("The MQO file contains no model objects.".into()); }
    Ok(ModelDocument { parts, materials, warnings: Vec::new() })
}

pub fn to_glb(document: &ModelDocument) -> Result<Vec<u8>, String> {
    let mut binary = Vec::new(); let mut views = Vec::new(); let mut accessors = Vec::new(); let mut meshes = Vec::new(); let mut nodes = Vec::new();
    for (index, part) in document.parts.iter().enumerate() {
        let position_view = append_f32_vec3(&mut binary, &part.positions, &mut views); let (min, max) = bounds(&part.positions);
        let position_accessor = accessors.len(); accessors.push(serde_json::json!({"bufferView":position_view,"componentType":5126,"count":part.positions.len(),"type":"VEC3","min":min,"max":max}));
        let normal_accessor = if part.normals.len() == part.positions.len() { let view = append_f32_vec3(&mut binary, &part.normals, &mut views); let value=accessors.len(); accessors.push(serde_json::json!({"bufferView":view,"componentType":5126,"count":part.normals.len(),"type":"VEC3"})); Some(value) } else { None };
        let uv_accessor = if part.texcoords.len() == part.positions.len() { let view=append_f32_vec2(&mut binary,&part.texcoords,&mut views); let value=accessors.len(); accessors.push(serde_json::json!({"bufferView":view,"componentType":5126,"count":part.texcoords.len(),"type":"VEC2"})); Some(value) } else { None };
        let index_view = append_u32(&mut binary, &part.indices, &mut views); let index_accessor=accessors.len(); accessors.push(serde_json::json!({"bufferView":index_view,"componentType":5125,"count":part.indices.len(),"type":"SCALAR"}));
        let mut attributes=serde_json::Map::new(); attributes.insert("POSITION".into(), position_accessor.into()); if let Some(v)=normal_accessor { attributes.insert("NORMAL".into(),v.into()); } if let Some(v)=uv_accessor { attributes.insert("TEXCOORD_0".into(),v.into()); }
        let mut primitive=serde_json::Map::new(); primitive.insert("attributes".into(),attributes.into()); primitive.insert("indices".into(),index_accessor.into()); if let Some(material)=part.material.filter(|v| *v<document.materials.len()){primitive.insert("material".into(),material.into());}
        meshes.push(serde_json::json!({"name":part.name,"primitives":[primitive]})); nodes.push(serde_json::json!({"name":part.name,"mesh":index,"extras":{"partId":part.id}}));
    }
    let materials: Vec<_> = document.materials.iter().map(|material| serde_json::json!({"name":material.name,"pbrMetallicRoughness":{"baseColorFactor":material.color,"metallicFactor":0,"roughnessFactor":0.8},"doubleSided":true})).collect();
    let json=serde_json::to_vec(&serde_json::json!({"asset":{"version":"2.0","generator":"MTR Pack Studio"},"scene":0,"scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>() }],"nodes":nodes,"meshes":meshes,"materials":materials,"buffers":[{"byteLength":binary.len()}],"bufferViews":views,"accessors":accessors})).map_err(|e|e.to_string())?;
    let mut json_chunk=json; while json_chunk.len()%4!=0 { json_chunk.push(b' '); } while binary.len()%4!=0 { binary.push(0); }
    let length=12+8+json_chunk.len()+8+binary.len(); let mut glb=Vec::with_capacity(length); glb.extend(0x46546C67u32.to_le_bytes()); glb.extend(2u32.to_le_bytes()); glb.extend((length as u32).to_le_bytes()); glb.extend((json_chunk.len() as u32).to_le_bytes()); glb.extend(0x4E4F534Au32.to_le_bytes()); glb.extend(json_chunk); glb.extend((binary.len() as u32).to_le_bytes()); glb.extend(0x004E4942u32.to_le_bytes()); glb.extend(binary); Ok(glb)
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
    #[test] fn glb_has_valid_header() { let doc=ModelDocument{parts:vec![ModelPart{id:"part".into(),name:"Part".into(),positions:vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]],normals:vec![],texcoords:vec![],indices:vec![0,1,2],material:None}],materials:vec![],warnings:vec![]};let glb=to_glb(&doc).unwrap();assert_eq!(&glb[..4],b"glTF");assert_eq!(u32::from_le_bytes(glb[8..12].try_into().unwrap())as usize,glb.len()); }
    #[test] fn parses_obj_and_mqo_triangles() {
        let root=std::env::temp_dir().join(format!("mtr-model-{}",uuid::Uuid::new_v4()));std::fs::create_dir_all(&root).unwrap();
        let obj=root.join("test.obj");std::fs::write(&obj,"o shell\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();let parsed=parse(&obj,&BTreeMap::new()).unwrap();assert_eq!(parsed.parts[0].indices.len(),3);
        let mqo=root.join("test.mqo");std::fs::write(&mqo,"Metasequoia Document\nFormat Text Ver 1.0\nObject \"shell\" {\n vertex 3 {\n0 0 0\n1 0 0\n0 1 0\n}\n face 1 {\n3 V(0 1 2)\n}\n}\nEof\n").unwrap();let parsed=parse(&mqo,&BTreeMap::new()).unwrap();assert_eq!(parsed.parts[0].indices.len(),3);std::fs::remove_dir_all(root).unwrap();
    }
}
