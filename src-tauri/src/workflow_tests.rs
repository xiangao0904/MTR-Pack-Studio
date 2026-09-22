use super::*;
use std::collections::BTreeMap;

fn glb_json(bytes: &[u8]) -> serde_json::Value {
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&bytes[20..20 + length]).unwrap()
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
    let bindings = vec![MaterialBinding { material_id:"material-0".into(), texture_asset_id:Some(hash.clone()) }];
    assert_ne!(asset_preview(&mut container,&asset,&bindings).unwrap(),preview);
    assert_eq!(asset_preview(&mut container,&asset,&[]).unwrap(),preview);
    let mut train = imported.train; train.carriages[0].body_models[0].material_bindings = bindings;
    container.index.cover_hash = Some(hash.clone()); train.carriages[0].thumbnail_hash = Some(hash.clone());
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
        let document = model::parse(&fixtures.join(name),&BTreeMap::new()).unwrap();
        assert!(!document.parts.is_empty());
        assert!(document.parts.iter().all(|part| !part.indices.is_empty() && part.normals.len()==part.positions.len()));
        model::validate_document(&document).unwrap();
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
