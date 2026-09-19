use crate::{container::Container, domain::{AssetDefinition, ModelLayer, TrainDefinition}, model::{ModelDocument, ModelMaterial, ModelPart}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, fs::{self, File}, io::Write, path::Path};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions { pub target: String, pub minecraft_version: String, #[serde(default)] pub model_format: String }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue { pub severity: String, pub message: String, pub train_id: Option<String>, pub carriage_id: Option<String>, pub field: Option<String> }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport { pub path: String, pub file_count: usize, pub warnings: Vec<ValidationIssue> }

pub fn load_trains(container: &mut Container) -> Result<Vec<TrainDefinition>, String> {
    let entries: Vec<_> = container.index.content.iter().filter(|entry| entry.kind == "train").cloned().collect();
    let mut trains = Vec::new();
    for entry in entries {
        let hash = entry.resources.first().ok_or_else(|| format!("Train {} has no document.", entry.name))?;
        let bytes = container.read_blob(hash)?;
        let train: TrainDefinition = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid train {}: {e}", entry.name))?;
        trains.push(train);
    }
    Ok(trains)
}

fn load_asset(container: &mut Container, id: &str) -> Result<(AssetDefinition, ModelDocument), String> {
    let asset_hash = container.index.assets.get(id).cloned().ok_or_else(|| format!("Model asset {id} is missing."))?;
    let asset: AssetDefinition = serde_json::from_slice(&container.read_blob(&asset_hash)?).map_err(|e| e.to_string())?;
    let document: ModelDocument = rmp_serde::from_slice(&container.read_blob(&asset.document_hash)?).map_err(|e| e.to_string())?;
    Ok((asset, document))
}

pub fn validate(container: &mut Container, options: &ExportOptions) -> Result<Vec<ValidationIssue>, String> {
    let trains = load_trains(container)?; let mut issues = Vec::new(); let mut ids = BTreeSet::new();
    if !valid_id(&container.index.namespace) { issue(&mut issues,"error","Project namespace contains unsupported characters.",None,None,Some("namespace")); }
    if !matches!(options.target.as_str(), "mtr4" | "mtr3_nte") { issue(&mut issues,"error","Choose an export target.",None,None,Some("target")); }
    if pack_format(&options.target, &options.minecraft_version).is_none() { issue(&mut issues,"error","The selected Minecraft version is not supported for this target.",None,None,Some("minecraftVersion")); }
    if options.target == "mtr4" && !matches!(options.model_format.as_str(), "obj" | "mqo") { issue(&mut issues,"error","MTR 4 model format must be OBJ or MQO.",None,None,Some("modelFormat")); }
    if trains.is_empty() { issue(&mut issues,"error","Create at least one train before exporting.",None,None,None); }
    for train in &trains {
        if !valid_id(&train.export_id) { issue(&mut issues,"error","Train export ID is invalid.",Some(&train.id),None,Some("exportId")); }
        if !ids.insert(train.export_id.clone()) { issue(&mut issues,"error","Train export IDs must be unique.",Some(&train.id),None,Some("exportId")); }
        if train.carriages.is_empty() { issue(&mut issues,"error","Train has no carriages.",Some(&train.id),None,Some("carriages")); }
        let first_length = train.carriages.first().map(|value| value.length);
        if options.target == "mtr3_nte" && train.mtr3_base_train_type.trim().is_empty() { issue(&mut issues,"error","Choose an MTR 3 base train type.",Some(&train.id),None,Some("mtr3BaseTrainType")); }
        for carriage in &train.carriages {
            if !valid_id(&carriage.export_id) { issue(&mut issues,"error","Carriage export ID is invalid.",Some(&train.id),Some(&carriage.id),Some("exportId")); }
            if carriage.body_models.is_empty() { issue(&mut issues,"error","Import at least one body model for this carriage.",Some(&train.id),Some(&carriage.id),Some("bodyModels")); }
            if options.target == "mtr3_nte" && first_length.is_some_and(|length| (length-carriage.length).abs() > 0.001) { issue(&mut issues,"error","MTR 3 requires every carriage in a train to use the same length.",Some(&train.id),Some(&carriage.id),Some("length")); }
            if let Err(error)=carriage.placement.expressions(){issue(&mut issues,"error",&error,Some(&train.id),Some(&carriage.id),Some("placement"));}
            for layer in carriage.body_models.iter().chain(&carriage.bogie_1_models).chain(&carriage.bogie_2_models) {
                if !container.index.assets.contains_key(&layer.asset_id) { issue(&mut issues,"error","A referenced model asset is missing.",Some(&train.id),Some(&carriage.id),Some("models")); }
                for rule in layer.part_rules.values(){if let Err(error)=rule.expressions(){issue(&mut issues,"error",&error,Some(&train.id),Some(&carriage.id),Some("partRules"));}}
            }
        }
    }
    Ok(issues)
}

pub fn export(container: &mut Container, path: &Path, options: &ExportOptions) -> Result<ExportReport, String> {
    let issues = validate(container, options)?;
    if issues.iter().any(|item| item.severity == "error") { return Err("Resolve the export validation errors before creating the pack.".into()); }
    let trains = load_trains(container)?; let mut files = BTreeMap::new();
    files.insert("pack.mcmeta".into(), pretty(&json!({"pack":{"pack_format":pack_format(&options.target,&options.minecraft_version).unwrap(),"description":format!("{} — exported by MTR Pack Studio",container.index.name)}}))?);
    if options.target == "mtr4" { build_mtr4(container,&trains,&container.index.namespace.clone(),&options.model_format,&mut files)?; }
    else { return Err("The MTR 3 + NTE exporter is not available yet.".into()); }
    write_zip(path,&files)?;
    Ok(ExportReport { path: path.to_string_lossy().into_owned(), file_count: files.len(), warnings: issues.into_iter().filter(|item|item.severity=="warning").collect() })
}

fn build_mtr4(container:&mut Container,trains:&[TrainDefinition],namespace:&str,format:&str,files:&mut BTreeMap<String,Vec<u8>>)->Result<(),String>{
    let mut vehicles=Vec::new();
    for train in trains { for carriage in &train.carriages {
        let base=format!("{}_{}",train.export_id,carriage.export_id); let models=write_mtr4_layers(container,namespace,&base,"body",&carriage.body_models,format,files)?; let bogie1=write_mtr4_layers(container,namespace,&base,"bogie1",&carriage.bogie_1_models,format,files)?; let bogie2=write_mtr4_layers(container,namespace,&base,"bogie2",&carriage.bogie_2_models,format,files)?;
        let mut tags=train.tags.clone();tags.push(format!("family:{}",train.export_id));tags.push(format!("type:{}",carriage.export_id));tags.sort();tags.dedup();
        vehicles.push(json!({"id":format!("{}:{}",namespace,base),"name":format!("{} — {}",train.name,carriage.name),"color":train.color,"transportMode":"TRAIN","length":carriage.length,"width":carriage.width,"bogie1Position":carriage.bogie_1_position,"bogie2Position":carriage.bogie_2_position,"couplingPadding1":carriage.coupling_padding_1,"couplingPadding2":carriage.coupling_padding_2,"description":train.description,"wikipediaArticle":"","tags":tags,"models":models,"bogie1Models":bogie1,"bogie2Models":bogie2,"hasGangway1":carriage.end_1.gangway,"hasGangway2":carriage.end_2.gangway,"hasBarrier1":carriage.end_1.barrier,"hasBarrier2":carriage.end_2.barrier,"legacyRiderOffset":0,"bveSoundBaseResource":"","legacySpeedSoundBaseResource":"","legacySpeedSoundCount":1,"legacyUseAccelerationSoundsWhenCoasting":false,"legacyConstantPlaybackSpeed":false,"legacyDoorSoundBaseResource":"","legacyDoorCloseSoundTime":0}));
    }}
    files.insert(format!("assets/{namespace}/mtr_custom_resources.json"),pretty(&json!({"vehicles":vehicles}))?);Ok(())
}

fn write_mtr4_layers(container:&mut Container,namespace:&str,base:&str,slot:&str,layers:&[ModelLayer],format:&str,files:&mut BTreeMap<String,Vec<u8>>)->Result<Vec<Value>,String>{
    let mut output=Vec::new();
    for (layer_index,layer) in layers.iter().enumerate(){let(asset,document)=load_asset(container,&layer.asset_id)?;let groups=material_groups(&document);
        for(group_index,parts)in groups{let group_name=group_index.map(|value|value.to_string()).unwrap_or_else(||"default".into());let suffix=format!("{base}_{slot}_{layer_index}_{group_name}");let filtered=ModelDocument{parts,materials:document.materials.clone(),warnings:Vec::new()};let extension=if format=="mqo"{"mqo"}else{"obj"};let model_path=format!("assets/{namespace}/models/vehicle/{suffix}.{extension}");
            if format=="mqo"{files.insert(model_path,write_mqo(&filtered).into_bytes());}else{files.insert(model_path,write_obj(&filtered,&format!("{suffix}.mtl")).into_bytes());files.insert(format!("assets/{namespace}/models/vehicle/{suffix}.mtl"),write_mtl_single(&filtered,&format!("{suffix}.png")).into_bytes());}
            let material=group_index.and_then(|index|document.materials.get(index));let texture_resource=write_texture(container,&asset,material,namespace,&suffix,files)?;
            let properties=format!("assets/{namespace}/properties/vehicle/{suffix}.json");files.insert(properties,pretty(&mtr4_properties(&filtered))?);let positions=format!("assets/{namespace}/properties/definition/{suffix}.json");files.insert(positions,pretty(&json!({"positionDefinitions":[]}))?);
            output.push(json!({"modelResource":format!("{namespace}:models/vehicle/{suffix}.{extension}"),"textureResource":texture_resource,"modelPropertiesResource":format!("{namespace}:properties/vehicle/{suffix}.json"),"positionDefinitionsResource":format!("{namespace}:properties/definition/{suffix}.json"),"flipTextureV":layer.flip_texture_v}));
        }
    }Ok(output)
}


fn material_groups(document:&ModelDocument)->Vec<(Option<usize>,Vec<ModelPart>)>{let mut map:BTreeMap<Option<usize>,Vec<ModelPart>>=BTreeMap::new();for part in &document.parts{map.entry(part.material).or_default().push(part.clone());}map.into_iter().collect()}
fn mtr4_properties(document:&ModelDocument)->Value{let parts:Vec<_>=document.parts.iter().map(|part|json!({"names":[part.name],"positionDefinitions":[],"condition":"NORMAL","renderStage":"EXTERIOR","type":"NORMAL","displayXPadding":0,"displayYPadding":0,"displayColorCjk":"FFFFFF","displayColor":"FFFFFF","displayMaxLineHeight":0,"displayCjkSizeRatio":1,"displayPadZeros":0,"displayType":"DESTINATION","displayDefaultText":"","doorXMultiplier":0,"doorZMultiplier":0,"doorAnimationType":"STANDARD","renderFromOpeningDoorTime":0,"renderUntilOpeningDoorTime":0,"renderFromClosingDoorTime":0,"renderUntilClosingDoorTime":0,"flashOffTime":0,"flashOnTime":0})).collect();json!({"parts":parts,"modelYOffset":0,"gangwayInnerSideResource":"","gangwayInnerTopResource":"","gangwayInnerBottomResource":"","gangwayOuterSideResource":"","gangwayOuterTopResource":"","gangwayOuterBottomResource":"","gangwayWidth":0,"gangwayHeight":0,"gangwayYOffset":0,"gangwayZOffset":0,"barrierInnerSideResource":"","barrierInnerTopResource":"","barrierInnerBottomResource":"","barrierOuterSideResource":"","barrierOuterTopResource":"","barrierOuterBottomResource":"","barrierWidth":0,"barrierHeight":0,"barrierYOffset":0,"barrierZOffset":0})}

fn write_obj(document:&ModelDocument,mtl:&str)->String{let mut out=format!("# Exported by MTR Pack Studio\nmtllib {mtl}\n");let mut offset=1u32;for part in &document.parts{out+=&format!("o {}\ng {}\n",safe_name(&part.name),safe_name(&part.name));if let Some(index)=part.material{out+=&format!("usemtl material_{index}\n");}for p in &part.positions{out+=&format!("v {} {} {}\n",p[0],p[1],p[2]);}for uv in &part.texcoords{out+=&format!("vt {} {}\n",uv[0],uv[1]);}for n in &part.normals{out+=&format!("vn {} {} {}\n",n[0],n[1],n[2]);}for tri in part.indices.chunks_exact(3){out+="f";for index in tri{let value=offset+*index;if part.texcoords.len()==part.positions.len()&&part.normals.len()==part.positions.len(){out+=&format!(" {value}/{value}/{value}");}else if part.texcoords.len()==part.positions.len(){out+=&format!(" {value}/{value}");}else{out+=&format!(" {value}");}}out.push('\n');}offset+=part.positions.len()as u32;}out}
fn write_mtl(document:&ModelDocument,base:&str)->String{let mut out=String::from("# Exported by MTR Pack Studio\n");for(index,material)in document.materials.iter().enumerate(){out+=&format!("newmtl material_{index}\nKd {} {} {}\nd {}\nmap_Kd {base}_{index}.png\n\n",material.color[0],material.color[1],material.color[2],material.color[3]);}out}
fn write_mtl_single(document:&ModelDocument,texture:&str)->String{let mut out=String::from("# Exported by MTR Pack Studio\n");for(index,material)in document.materials.iter().enumerate(){out+=&format!("newmtl material_{index}\nKd {} {} {}\nd {}\nmap_Kd {texture}\n\n",material.color[0],material.color[1],material.color[2],material.color[3]);}out}
fn write_mqo(document:&ModelDocument)->String{let mut out=String::from("Metasequoia Document\nFormat Text Ver 1.0\n\n");out+=&format!("Material {} {{\n",document.materials.len());for material in &document.materials{out+=&format!(" \"{}\" col({} {} {} {})\n",safe_name(&material.name),material.color[0],material.color[1],material.color[2],material.color[3]);}out+="}\n";for part in &document.parts{out+=&format!("Object \"{}\" {{\n vertex {} {{\n",safe_name(&part.name),part.positions.len());for p in &part.positions{out+=&format!("  {} {} {}\n",-p[0],p[1],p[2]);}out+=" }\n";out+=&format!(" face {} {{\n",part.indices.len()/3);for tri in part.indices.chunks_exact(3){out+=&format!("  3 V({} {} {})",tri[0],tri[2],tri[1]);if let Some(m)=part.material{out+=&format!(" M({m})");}out.push('\n');}out+=" }\n}\n";}out+="Eof\n";out}

fn write_texture(container:&mut Container,asset:&AssetDefinition,material:Option<&ModelMaterial>,namespace:&str,suffix:&str,files:&mut BTreeMap<String,Vec<u8>>)->Result<String,String>{let path=format!("textures/vehicle/{suffix}.png");let bytes=if let Some(texture)=material.and_then(|value|value.texture.as_ref()){let name=Path::new(texture).file_name().and_then(|v|v.to_str()).unwrap_or(texture);if let Some(dep)=asset.dependencies.iter().find(|dep|dep.name.eq_ignore_ascii_case(name)){container.read_blob(&dep.hash)?}else{solid_png(material.unwrap().color)?}}else{solid_png(material.map(|value|value.color).unwrap_or([1.0;4]))?};files.insert(format!("assets/{namespace}/{path}"),bytes);Ok(format!("{namespace}:{path}"))}
fn solid_png(color:[f32;4])->Result<Vec<u8>,String>{let mut bytes=Vec::new();{let mut encoder=png::Encoder::new(&mut bytes,1,1);encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);let mut writer=encoder.write_header().map_err(|e|e.to_string())?;let pixel=color.map(|v|(v.clamp(0.0,1.0)*255.0)as u8);writer.write_image_data(&pixel).map_err(|e|e.to_string())?;}Ok(bytes)}
fn write_zip(path:&Path,files:&BTreeMap<String,Vec<u8>>)->Result<(),String>{if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}let temp=path.with_extension("zip.tmp");let file=File::create(&temp).map_err(|e|e.to_string())?;let mut zip=zip::ZipWriter::new(file);let options=zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).unix_permissions(0o644);for(name,bytes)in files{zip.start_file(name.replace('\\',"/"),options).map_err(|e|e.to_string())?;zip.write_all(bytes).map_err(|e|e.to_string())?;}let file=zip.finish().map_err(|e|e.to_string())?;file.sync_all().map_err(|e|e.to_string())?;drop(file);if path.exists(){fs::remove_file(path).map_err(|e|e.to_string())?;}fs::rename(&temp,path).map_err(|e|e.to_string())}
fn issue(issues:&mut Vec<ValidationIssue>,severity:&str,message:&str,train_id:Option<&str>,carriage_id:Option<&str>,field:Option<&str>){issues.push(ValidationIssue{severity:severity.into(),message:message.into(),train_id:train_id.map(Into::into),carriage_id:carriage_id.map(Into::into),field:field.map(Into::into)})}
fn valid_id(value:&str)->bool{!value.is_empty()&&value.chars().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||matches!(c,'_'|'-'|'.'))}
fn safe_name(value:&str)->String{let value:value::String=value.chars().map(|c|if c.is_ascii_alphanumeric()||matches!(c,'_'|'-'){c}else{'_'}).collect();if value.is_empty(){"part".into()}else{value}}
mod value{pub type String=std::string::String;}
fn pack_format(target:&str,version:&str)->Option<u32>{match(target,version){("mtr4","1.20.4")=>Some(22),("mtr3_nte","1.16.5")=>Some(6),("mtr3_nte","1.17.1")=>Some(7),("mtr3_nte","1.18.2")=>Some(8),("mtr3_nte","1.19.2")=>Some(9),("mtr3_nte","1.19.3")=>Some(12),("mtr3_nte","1.19.4")=>Some(13),("mtr3_nte","1.20.1")=>Some(15),_=>None}}
fn pretty(value:&Value)->Result<Vec<u8>,String>{serde_json::to_vec_pretty(value).map_err(|e|e.to_string())}

#[cfg(test)]
mod tests{use super::*;#[test]fn pack_versions_are_pinned(){assert_eq!(pack_format("mtr4","1.20.4"),Some(22));assert_eq!(pack_format("mtr3_nte","1.16.5"),Some(6));assert_eq!(pack_format("mtr3_nte","1.21"),None);}#[test]fn obj_writer_keeps_groups(){let doc=ModelDocument{parts:vec![ModelPart{id:"p".into(),name:"body shell".into(),positions:vec![[0.0,0.0,0.0],[1.0,0.0,0.0],[0.0,1.0,0.0]],normals:vec![],texcoords:vec![],indices:vec![0,1,2],material:None}],materials:vec![],warnings:vec![]};let obj=write_obj(&doc,"test.mtl");assert!(obj.contains("g body_shell"));assert!(obj.contains("f 1 2 3"));}}
