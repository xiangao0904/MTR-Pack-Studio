use super::*;
use std::collections::BTreeMap;

fn glb_json(bytes: &[u8]) -> serde_json::Value {
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[20..20 + length]).unwrap()
}

#[test]
fn valid_png_bytes_are_reused_without_reencoding() {
    let mut output=std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2,2,image::Rgba([12,34,56,78])).write_to(&mut output,image::ImageFormat::Png).unwrap();
    let bytes=output.into_inner();
    assert_eq!(normalize_png(&bytes).unwrap(),bytes);
    assert!(normalize_png(&bytes[..bytes.len()/2]).is_err());
}

#[test]
fn asset_catalog_recovers_model_textures_and_reuses_identical_imports() {
    let root=std::env::temp_dir().join(format!("mtr-assets-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
    let mut png=std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2,3,image::Rgba([20,30,40,255])).write_to(&mut png,image::ImageFormat::Png).unwrap();
    fs::write(root.join("paint.png"),png.into_inner()).unwrap();
    fs::write(root.join("body.mtl"),"newmtl paint\nmap_Kd paint.png\n").unwrap();
    let source=root.join("body.obj");
    fs::write(&source,"mtllib body.mtl\no shell\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl paint\nf 1/1 2/2 3/3\n").unwrap();
    let path=root.join("assets.mtrpack");let mut container=Container::create(&path,"Assets").unwrap();
    let first=import_model_asset_into(&mut container,&source,&BTreeMap::new()).unwrap();
    let second=import_model_asset_into(&mut container,&source,&BTreeMap::new()).unwrap();
    assert_eq!(first.id,second.id);assert_eq!(container.index.assets.len(),1);
    let mut thumbnail=std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(240,160,image::Rgba([40,50,60,255])).write_to(&mut thumbnail,image::ImageFormat::Png).unwrap();
    let thumbnail_hash=container.put_blob(&thumbnail.into_inner(),"image/png").unwrap();
    container.index.asset_thumbnail_hashes.insert(first.id.clone(),thumbnail_hash.clone());
    let mut train=TrainDefinition::new("Train","train");train.carriages[0].body_models.push(new_model_layer(&first));
    container.index.content.push(ContentEntry{id:train.id.clone(),kind:"train".into(),name:train.name.clone(),file:"train.json".into(),updated_at:0,resources:vec![]});
    write_train_document(&mut container,0,&train).unwrap();container.commit().unwrap();
    container.index.texture_names.clear(); // Earlier projects had no explicit texture catalog.
    let catalog=asset_catalog(&mut container).unwrap();
    assert_eq!(catalog.models.len(),1);assert_eq!(catalog.models[0].references.len(),1);
    assert_eq!(catalog.models[0].thumbnail_hash.as_deref(),Some(thumbnail_hash.as_str()));
    assert_eq!(catalog.textures.len(),1);assert_eq!((catalog.textures[0].width,catalog.textures[0].height),(2,3));
    container.commit().unwrap();drop(container);
    let mut reopened=Container::open(&path).unwrap();let catalog=asset_catalog(&mut reopened).unwrap();assert_eq!(catalog.textures.len(),1);assert_eq!(catalog.models[0].thumbnail_hash.as_deref(),Some(thumbnail_hash.as_str()));
    drop(reopened);fs::remove_dir_all(root).unwrap();
}

#[test]
fn metasequoia_obj_uvs_keep_their_top_left_origin_in_preview() {
    let root=std::env::temp_dir().join(format!("mtr-uv-origin-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
    let path=root.join("model.obj");
    let geometry="v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0.25 0.1\nvt 0.75 0.1\nvt 0.25 0.9\nf 1/1 2/2 3/3\n";
    let preview_v=|document:&model::ModelDocument| {
        let glb=model::to_glb(document).unwrap();let json=glb_json(&glb);
        let accessor=json["meshes"][0]["primitives"][0]["attributes"]["TEXCOORD_0"].as_u64().unwrap() as usize;
        let view=json["accessors"][accessor]["bufferView"].as_u64().unwrap() as usize;
        let offset=json["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;
        let start=28+u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize+offset+4;
        f32::from_le_bytes(glb[start..start+4].try_into().unwrap())
    };
    fs::write(&path,format!("# Created by Metasequoia\n{geometry}")).unwrap();
    let top_left=model::parse(&path,&BTreeMap::new()).unwrap();
    assert!(top_left.uv_origin_top_left);
    assert!((preview_v(&top_left)-0.1).abs()<1e-6);
    assert!(!top_left.flip_v(false));assert!(top_left.flip_v(true));
    fs::write(&path,geometry).unwrap();
    let conventional=model::parse(&path,&BTreeMap::new()).unwrap();
    assert!(!conventional.uv_origin_top_left);
    assert!((preview_v(&conventional)-0.9).abs()<1e-6);
    assert!(conventional.flip_v(false));assert!(!conventional.flip_v(true));
    let mut legacy=serde_json::to_value(&conventional).unwrap();legacy.as_object_mut().unwrap().remove("uvOriginTopLeft");
    assert!(!serde_json::from_value::<model::ModelDocument>(legacy).unwrap().uv_origin_top_left);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn existing_metasequoia_assets_get_uv_correction_without_reimport() {
    let root=std::env::temp_dir().join(format!("mtr-legacy-uv-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
    let mut container=Container::create(&root.join("legacy.mtrpack"),"Legacy").unwrap();
    let source_hash=container.put_blob(b"# Created by Metasequoia\nv 0 0 0\n","model/obj").unwrap();
    let document=model::ModelDocument {parts:vec![],materials:vec![],warnings:vec![],uv_origin_top_left:false};
    let document_hash=container.put_blob(&rmp_serde::to_vec_named(&document).unwrap(),"application/vnd.mtrpack.model+msgpack").unwrap();
    let asset=AssetDefinition{id:"legacy".into(),name:"Legacy".into(),source_format:domain::ModelFormat::Obj,source_hash,document_hash,preview_hash:String::new(),dependencies:vec![],parts:vec![],warnings:vec![],legacy_uv_correction:false};
    assert!(legacy_uv_correction(&mut container,&asset).unwrap());
    let mut current=document;current.uv_origin_top_left=true;
    let mut current_asset=asset.clone();current_asset.document_hash=container.put_blob(&rmp_serde::to_vec_named(&current).unwrap(),"application/vnd.mtrpack.model+msgpack").unwrap();
    assert!(!legacy_uv_correction(&mut container,&current_asset).unwrap());
    drop(container);fs::remove_dir_all(root).unwrap();
}

#[test]
fn pbr_import_bindings_and_images_survive_reopen_and_glb_conversion() {
    use crate::material::{AlphaMode, MaterialProperties, TextureChannel};
    let root=std::env::temp_dir().join(format!("mtr-pbr-{}",Uuid::new_v4()));fs::create_dir_all(&root).unwrap();
    let png=|pixel| {let mut out=std::io::Cursor::new(Vec::new());image::RgbaImage::from_pixel(2,2,image::Rgba(pixel)).write_to(&mut out,image::ImageFormat::Png).unwrap();out.into_inner()};
    for (name,pixel) in [("base",[255,255,255,128]),("normal",[128,128,255,255]),("metal",[64,0,0,255]),("rough",[192,0,0,255]),("emission",[255,64,0,255]),("ao",[128,128,128,255])] {fs::write(root.join(format!("{name}.png")),png(pixel)).unwrap();}
    fs::write(root.join("body.mtl"),"newmtl paint\nKd 1 1 1\nd 0.6\nPm 0.8\nPr 0.3\nKe 0.2 0.1 0\nmap_Kd base.png\nnorm normal.png\nmap_Pm metal.png\nmap_Pr rough.png\nmap_Ke emission.png\nmap_AO ao.png\n").unwrap();
    let source=root.join("body.obj");fs::write(&source,"mtllib body.mtl\no shell\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl paint\nf 1/1 2/2 3/3\n").unwrap();
    let path=root.join("pbr.mtrpack");let mut container=Container::create(&path,"PBR").unwrap();let train=TrainDefinition::new("Train","train");
    container.index.content.push(ContentEntry{id:train.id.clone(),kind:"train".into(),name:train.name.clone(),file:"train.json".into(),updated_at:0,resources:vec![]});write_train_document(&mut container,0,&train).unwrap();container.commit().unwrap();
    let imported=import_model_into(&mut container,&train.id,&train.carriages[0].id,"body",source.to_str().unwrap(),&BTreeMap::new(),Some(train.revision)).unwrap();
    let original=asset_preview(&mut container,&imported.asset,&[]).unwrap();let json=glb_json(&original);let material=&json["materials"][0];
    assert_eq!(material["alphaMode"],"BLEND");assert!((material["pbrMetallicRoughness"]["roughnessFactor"].as_f64().unwrap()-0.3).abs()<1e-6);
    for key in ["normalTexture","emissiveTexture","occlusionTexture"] {assert!(material[key]["index"].is_number());}
    let texture=material["pbrMetallicRoughness"]["metallicRoughnessTexture"]["index"].as_u64().unwrap() as usize;
    let image=json["textures"][texture]["source"].as_u64().unwrap() as usize;let view=json["images"][image]["bufferView"].as_u64().unwrap() as usize;
    let offset=json["bufferViews"][view]["byteOffset"].as_u64().unwrap() as usize;let length=json["bufferViews"][view]["byteLength"].as_u64().unwrap() as usize;
    let binary=28+u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    let packed=image::load_from_memory(&original[binary+offset..binary+offset+length]).unwrap().to_rgba8();assert_eq!(packed.get_pixel(0,0).0,[255,192,64,255]);
    assert_eq!(json["samplers"][0]["minFilter"],9987);
    let replacement=container.put_blob(&png([0,128,255,255]),"image/png").unwrap();
    let binding=MaterialBinding{material_id:"material-0".into(),texture_asset_id:None,properties:MaterialProperties{metalness:Some(1.0),roughness:Some(0.12),opacity:Some(0.4),alpha_mode:Some(AlphaMode::Blend),normal_scale:Some(0.5),maps:BTreeMap::from([(TextureChannel::Normal,replacement.clone())]),..Default::default()}};
    let mut train=imported.train;train.carriages[0].body_models[0].material_bindings=vec![binding.clone()];write_train_document(&mut container,0,&train).unwrap();container.commit().unwrap();drop(container);
    let mut reopened=Container::open(&path).unwrap();let restored=read_train_document(&mut reopened,0).unwrap();assert_eq!(restored,train);assert!(reopened.read_blob(&replacement).is_ok());
    let preview=asset_preview(&mut reopened,&imported.asset,&[binding]).unwrap();let edited=glb_json(&preview);assert_eq!(edited["materials"][0]["normalTexture"]["scale"],0.5);assert_eq!(edited["materials"][0]["pbrMetallicRoughness"]["metallicFactor"],1.0);
    assert_eq!(asset_preview(&mut reopened,&imported.asset,&[]).unwrap(),original);
    drop(reopened);fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_materials_have_defaults_and_invalid_edits_are_rejected() {
    let material:model::ModelMaterial=serde_json::from_value(serde_json::json!({"id":"m","name":"Paint","color":[1,1,1,1],"texture":null})).unwrap();
    assert_eq!(material.properties,crate::material::MaterialProperties::default());
    let binding:MaterialBinding=serde_json::from_value(serde_json::json!({"materialId":"m","textureAssetId":"hash"})).unwrap();assert!(binding.properties.maps.is_empty());
    assert!(crate::material::MaterialProperties{roughness:Some(f32::NAN),..Default::default()}.validate().is_err());
    assert!(crate::material::MaterialProperties{opacity:Some(2.0),..Default::default()}.validate().is_err());
}

#[test]
fn textured_import_reopens_and_replacement_is_independent() {
    let root = std::env::temp_dir().join(format!("mtr-workflow-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("workflow.mtrpack");
    let mut container = Container::create(&path, "Workflow").unwrap();
    let train = TrainDefinition::new("Train", "train");
    container.index.content.push(ContentEntry { id: train.id.clone(), kind: "train".into(), name: train.name.clone(), file: "train.json".into(), updated_at: 0, resources: vec![] });
    write_train_document(&mut container, 0, &train).unwrap(); container.commit().unwrap();
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255])).write_to(&mut image_bytes, image::ImageFormat::Png).unwrap();
    fs::write(root.join("paint.png"), image_bytes.into_inner()).unwrap();
    fs::write(root.join("body.mtl"), "newmtl paint\nKd 1 1 1\nmap_Kd paint.png\n").unwrap();
    let model = root.join("body.obj");
    fs::write(&model, "mtllib body.mtl\no shell\nv 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nusemtl paint\nf 1/1 2/2 3/3\n").unwrap();
    let imported = import_model_into(&mut container, &train.id, &train.carriages[0].id, "body", model.to_str().unwrap(), &BTreeMap::new(), Some(train.revision)).unwrap();
    let asset = imported.asset;
    let preview = container.read_blob(&asset.preview_hash).unwrap();
    let glb = glb_json(&preview);
    assert_eq!(glb["images"][0]["mimeType"], "image/png");
    assert_eq!(glb["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"]["index"], 0);
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(2,2,image::Rgb([0,255,0])).write_to(&mut image_bytes,image::ImageFormat::Jpeg).unwrap();
    let png = normalize_png(&image_bytes.into_inner()).unwrap(); assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let hash = container.put_blob(&png,"image/png").unwrap();
    let bindings = vec![MaterialBinding { material_id:"material-0".into(), texture_asset_id:Some(hash.clone()), properties: Default::default() }];
    assert_ne!(asset_preview(&mut container,&asset,&bindings).unwrap(),preview);
    assert_eq!(asset_preview(&mut container,&asset,&[]).unwrap(),preview);
    let mut train = imported.train; train.carriages[0].body_models[0].material_bindings = bindings;
    container.index.cover_hash = Some(hash.clone()); train.carriages[0].thumbnail_hash = Some(hash.clone());
    train.carriages[0].thumbnail_model_signature = Some("studio-thumbnail-v2".into());
    write_train_document(&mut container,0,&train).unwrap(); container.commit().unwrap(); drop(container);
    let mut reopened = Container::open(&path).unwrap();
    let restored = read_train_document(&mut reopened,0).unwrap(); assert_eq!(restored,train);
    assert_eq!(reopened.index.cover_hash.as_deref(),Some(hash.as_str()));
    assert_eq!(reopened.read_blob(&hash).unwrap(),png);
    // Missing dependencies and invalid slots must not mutate the active index.
    fs::remove_file(root.join("paint.png")).unwrap();
    let before = serde_json::to_value(&reopened.index).unwrap();
    assert!(import_model_into(&mut reopened,&train.id,&train.carriages[0].id,"body",model.to_str().unwrap(),&BTreeMap::new(),Some(train.revision)).is_err());
    assert_eq!(serde_json::to_value(&reopened.index).unwrap(),before);
    drop(reopened); fs::remove_dir_all(root).unwrap();
}

#[test]
fn binary_and_ascii_fbx_have_valid_static_geometry() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ufbx");
    for name in ["blender_279_default_6100_ascii.fbx", "blender_279_default_7400_binary.fbx"] {
        let path = fixtures.join(name);
        let (document, references, embedded) = model::parse_with_dependencies(&path,&BTreeMap::new()).unwrap();
        assert!(!document.parts.is_empty());
        assert!(document.parts.iter().all(|part| !part.indices.is_empty() && part.normals.len()==part.positions.len()));
        model::validate_document(&document).unwrap();
        assert_eq!(references,model::referenced_files(&path,&BTreeMap::new()).unwrap());
        assert!(embedded.iter().all(|(name,bytes)| !name.is_empty() && !bytes.is_empty()));
    }
}

#[test]
fn mqo_preserves_face_materials_and_uv_seams_and_rejects_bad_indices() {
    let root=std::env::temp_dir().join(format!("mtr-mqo-{}",Uuid::new_v4())); fs::create_dir_all(&root).unwrap();
    let path=root.join("faces.mqo");
    let source="Metasequoia Document\nFormat Text Ver 1.0\nMaterial 2 {\n\"red\" col(1 0 0 1)\n\"green\" col(0 1 0 1)\n}\nObject \"shell\" {\nvertex 4 {\n0 0 0\n100 0 0\n100 100 0\n0 100 0\n}\nface 2 {\n3 V(0 1 2) M(0) UV(0 0 1 0 1 1)\n3 V(0 2 3) M(1) UV(0.5 0.5 1 1 0 1)\n}\n}\nEof\n";
    fs::write(&path,source).unwrap(); let doc=model::parse(&path,&BTreeMap::new()).unwrap();
    assert_eq!(doc.parts.len(),2); assert_eq!(doc.parts[0].positions.len(),3); assert_eq!(doc.parts[1].texcoords[0],[0.5,0.5]);
    assert_eq!(doc.parts[0].positions[2],[-1.0,0.0,0.0]);
    fs::write(&path,source.replace("V(0 2 3)","V(0 2 99)")).unwrap(); assert!(model::parse(&path,&BTreeMap::new()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn authored_train_round_trips_and_exports_all_three_formats() {
    let manifest=Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures=manifest.join("tests/fixtures/studio-train");
    let directory=std::env::temp_dir().join(format!("mtr-acceptance-{}",Uuid::new_v4()));
    fs::create_dir_all(&directory).unwrap();
    let source=directory.join("studio-train.mtrpack");
    let mut container=Container::create(&source,"Studio Acceptance").unwrap();
    let mut train=TrainDefinition::new("Studio Metro","studio_metro");train.mtr3_base_train_type="sp1900".into();
    container.index.content.push(ContentEntry{id:train.id.clone(),kind:"train".into(),name:train.name.clone(),file:"train.json".into(),updated_at:0,resources:vec![]});
    write_train_document(&mut container,0,&train).unwrap();container.commit().unwrap();
    for (slot,file) in [("body","body.obj"),("bogie1","bogie.obj"),("bogie2","bogie.obj")] {
        train=import_model_into(&mut container,&train.id,&train.carriages[0].id,slot,fixtures.join(file).to_str().unwrap(),&BTreeMap::new(),Some(train.revision)).unwrap().train;
    }
    let bogie_one=read_asset(&mut container,&train.carriages[0].bogie_1_models[0].asset_id).unwrap();
    let bogie_two=read_asset(&mut container,&train.carriages[0].bogie_2_models[0].asset_id).unwrap();
    assert_eq!(bogie_one.source_hash,bogie_two.source_hash);
    assert_eq!(bogie_one.document_hash,bogie_two.document_hash);
    let car_id=train.carriages[0].id.clone();
    train.preview_consist=vec![domain::PreviewCarriage{carriage_id:car_id.clone(),reversed:false},domain::PreviewCarriage{carriage_id:car_id.clone(),reversed:false},domain::PreviewCarriage{carriage_id:car_id,reversed:true}];
    write_train_document(&mut container,0,&train).unwrap();container.commit().unwrap();drop(container);
    let moved=directory.join("moved.mtrpack");fs::rename(&source,&moved).unwrap();
    let mut container=Container::open(&moved).unwrap();assert_eq!(read_train_document(&mut container,0).unwrap(),train);
    let body=read_asset(&mut container,&train.carriages[0].body_models[0].asset_id).unwrap();
    let preview=asset_preview(&mut container,&body,&[]).unwrap();assert_eq!(glb_json(&preview)["images"].as_array().unwrap().len(),2);
    for (name,target,version,format) in [("mtr4-obj","mtr4","1.20.4","obj"),("mtr4-mqo","mtr4","1.20.4","mqo"),("mtr3-nte","mtr3_nte","1.20.1","obj")] {
        let report=exporter::export(&mut container,&directory.join(format!("{name}.zip")),&ExportOptions{target:target.into(),minecraft_version:version.into(),model_format:format.into(),only_visible:false}).unwrap();
        assert!(report.file_count>8);
    }
    // Keep reviewable native artifacts outside the source tree for desktop/game acceptance.
    let output=manifest.join("target/acceptance");fs::create_dir_all(&output).unwrap();
    drop(container);
    fs::copy(&moved,output.join("Studio Acceptance.mtrpack")).unwrap();
    for name in ["mtr4-obj","mtr4-mqo","mtr3-nte"] {fs::copy(directory.join(format!("{name}.zip")),output.join(format!("{name}.zip"))).unwrap();}
    fs::remove_dir_all(directory).unwrap();
}
