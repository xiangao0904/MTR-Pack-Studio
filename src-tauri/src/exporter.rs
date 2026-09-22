use crate::{
    container::Container,
    domain::{AssetDefinition, ModelLayer, ModelTransform, TrainDefinition},
    model::{ModelDocument, ModelMaterial, ModelPart},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::Path,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub target: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub model_format: String,
    #[serde(default)]
    pub only_visible: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub severity: String,
    pub message: String,
    pub train_id: Option<String>,
    pub carriage_id: Option<String>,
    pub layer_id: Option<String>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub path: String,
    pub file_count: usize,
    pub warnings: Vec<ValidationIssue>,
}

pub fn load_trains(container: &mut Container) -> Result<Vec<TrainDefinition>, String> {
    let entries: Vec<_> = container
        .index
        .content
        .iter()
        .filter(|entry| entry.kind == "train")
        .cloned()
        .collect();
    let mut trains = Vec::new();
    for entry in entries {
        let hash = entry
            .resources
            .first()
            .ok_or_else(|| format!("Train {} has no document.", entry.name))?;
        let bytes = container.read_blob(hash)?;
        let train: TrainDefinition = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Invalid train {}: {e}", entry.name))?;
        trains.push(train);
    }
    Ok(trains)
}

fn load_asset(
    container: &mut Container,
    id: &str,
) -> Result<(AssetDefinition, ModelDocument), String> {
    let asset_hash = container
        .index
        .assets
        .get(id)
        .cloned()
        .ok_or_else(|| format!("Model asset {id} is missing."))?;
    let asset: AssetDefinition =
        serde_json::from_slice(&container.read_blob(&asset_hash)?).map_err(|e| e.to_string())?;
    let document: ModelDocument =
        rmp_serde::from_slice(&container.read_blob(&asset.document_hash)?)
            .map_err(|e| e.to_string())?;
    Ok((asset, document))
}

pub fn validate(
    container: &mut Container,
    options: &ExportOptions,
) -> Result<Vec<ValidationIssue>, String> {
    let trains = load_trains(container)?;
    let mut issues = Vec::new();
    let mut ids = BTreeSet::new();
    if !valid_id(&container.index.namespace) {
        issue(
            &mut issues,
            "error",
            "Project namespace contains unsupported characters.",
            None,
            None,
            Some("namespace"),
        );
    }
    if !matches!(options.target.as_str(), "mtr4" | "mtr3_nte") {
        issue(
            &mut issues,
            "error",
            "Choose an export target.",
            None,
            None,
            Some("target"),
        );
    }
    if pack_format(&options.target, &options.minecraft_version).is_none() {
        issue(
            &mut issues,
            "error",
            "The selected Minecraft version is not supported for this target.",
            None,
            None,
            Some("minecraftVersion"),
        );
    }
    if options.target == "mtr4" && !matches!(options.model_format.as_str(), "obj" | "mqo") {
        issue(
            &mut issues,
            "error",
            "MTR 4 model format must be OBJ or MQO.",
            None,
            None,
            Some("modelFormat"),
        );
    }
    if trains.is_empty() {
        issue(
            &mut issues,
            "error",
            "Create at least one train before exporting.",
            None,
            None,
            None,
        );
    }
    for train in &trains {
        if train.color.len() != 6 || !train.color.chars().all(|c| c.is_ascii_hexdigit()) {
            issue(
                &mut issues,
                "error",
                "Train color must contain six hexadecimal digits.",
                Some(&train.id),
                None,
                Some("color"),
            );
        }
        if !valid_id(&train.export_id) {
            issue(
                &mut issues,
                "error",
                "Train export ID is invalid.",
                Some(&train.id),
                None,
                Some("exportId"),
            );
        }
        if !ids.insert(train.export_id.clone()) {
            issue(
                &mut issues,
                "error",
                "Train export IDs must be unique.",
                Some(&train.id),
                None,
                Some("exportId"),
            );
        }
        if train.carriages.is_empty() {
            issue(
                &mut issues,
                "error",
                "Train has no carriages.",
                Some(&train.id),
                None,
                Some("carriages"),
            );
        }
        let mut carriage_ids = BTreeSet::new();
        let first_length = train.carriages.first().map(|value| value.length);
        let first_width = train.carriages.first().map(|value| value.width);
        if options.target == "mtr3_nte" && train.mtr3_base_train_type.trim().is_empty() {
            issue(
                &mut issues,
                "error",
                "Choose an MTR 3 base train type.",
                Some(&train.id),
                None,
                Some("mtr3BaseTrainType"),
            );
        }
        for carriage in &train.carriages {
            if !carriage_ids.insert(carriage.export_id.clone()) {
                issue(
                    &mut issues,
                    "error",
                    "Carriage export IDs must be unique within a train.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("exportId"),
                );
            }
            for (field, value, positive) in [
                ("length", carriage.length, true),
                ("width", carriage.width, true),
                ("couplingPadding1", carriage.coupling_padding_1, false),
                ("couplingPadding2", carriage.coupling_padding_2, false),
            ] {
                if !value.is_finite() || value < 0.0 || (positive && value == 0.0) {
                    issue(
                        &mut issues,
                        "error",
                        "Dimensions must be finite and positive; coupling padding may be zero.",
                        Some(&train.id),
                        Some(&carriage.id),
                        Some(field),
                    );
                }
            }
            if !carriage.bogie_1_position.is_finite() || !carriage.bogie_2_position.is_finite() {
                issue(
                    &mut issues,
                    "error",
                    "Bogie positions must be finite.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("bogie1Position"),
                );
            }
            if options.target == "mtr3_nte"
                && (carriage.end_1.gangway
                    || carriage.end_2.gangway
                    || carriage.end_1.barrier
                    || carriage.end_2.barrier
                    || carriage.coupling_padding_1 != 0.0
                    || carriage.coupling_padding_2 != 0.0)
            {
                issue(&mut issues,"error","MTR 3 cannot represent per-carriage gangways, barriers or coupling padding. Model these explicitly or export to MTR 4.",Some(&train.id),Some(&carriage.id),Some("mtr3Compatibility"));
            }
            if !valid_id(&carriage.export_id) {
                issue(
                    &mut issues,
                    "error",
                    "Carriage export ID is invalid.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("exportId"),
                );
            }
            let has_visible_body = carriage
                .body_models
                .iter()
                .filter(|layer| !options.only_visible || layer.visible)
                .any(|layer| {
                    load_asset(container, &layer.asset_id).is_ok_and(|(_, document)| {
                        document.parts.iter().any(|part| {
                            !options.only_visible || !layer.hidden_parts.contains(&part.id)
                        })
                    })
                });
            if !has_visible_body {
                issue(
                    &mut issues,
                    "error",
                    "Include at least one body model part for this carriage before exporting.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("bodyModels"),
                );
            }
            if options.target == "mtr3_nte"
                && first_length.is_some_and(|length| (length - carriage.length).abs() > 0.001)
            {
                issue(
                    &mut issues,
                    "error",
                    "MTR 3 requires every carriage in a train to use the same length.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("length"),
                );
            }
            if options.target == "mtr3_nte"
                && first_width.is_some_and(|width| (width - carriage.width).abs() > 0.001)
            {
                issue(
                    &mut issues,
                    "error",
                    "MTR 3 requires every carriage in a train to use the same width.",
                    Some(&train.id),
                    Some(&carriage.id),
                    Some("width"),
                );
            }
            if options.target == "mtr3_nte" {
                if let Err(error) = carriage.placement.expressions() {
                    issue(
                        &mut issues,
                        "error",
                        &error,
                        Some(&train.id),
                        Some(&carriage.id),
                        Some("placement"),
                    );
                }
            }
            for layer in carriage
                .body_models
                .iter()
                .chain(&carriage.bogie_1_models)
                .chain(&carriage.bogie_2_models)
            {
                if options.only_visible && !layer.visible {
                    continue;
                }
                let mut errors: Vec<(String, String)> = Vec::new();
                if let Err(error) = layer.transform.validate() {
                    errors.push(("transform".into(), error));
                }
                for (part_id, transform) in &layer.part_transforms {
                    if let Err(error) = transform.validate() {
                        errors.push((format!("partTransforms.{part_id}"), error));
                    }
                }
                match load_asset(container, &layer.asset_id) {
                    Err(error) => errors.push(("models".into(), error)),
                    Ok((asset, document)) => {
                        if document.parts.is_empty() {
                            errors.push((
                                "models".into(),
                                "Model contains no renderable geometry.".into(),
                            ));
                        }
                        let mut effective = document.clone();
                        if let Err(error) =
                            apply_layer_edits(&mut effective, layer, options.only_visible)
                        {
                            errors.push(("transform".into(), error));
                        }
                        for part in &effective.parts {
                            if part.positions.is_empty()
                                || part.indices.is_empty()
                                || part.indices.len() % 3 != 0
                                || (!part.normals.is_empty()
                                    && part.normals.len() != part.positions.len())
                                || (!part.texcoords.is_empty()
                                    && part.texcoords.len() != part.positions.len())
                                || part
                                    .indices
                                    .iter()
                                    .any(|i| *i as usize >= part.positions.len())
                                || part
                                    .positions
                                    .iter()
                                    .flatten()
                                    .chain(part.normals.iter().flatten())
                                    .chain(part.texcoords.iter().flatten())
                                    .any(|v| !v.is_finite())
                            {
                                errors.push((
                                    "models".into(),
                                    format!(
                                        "Part {} contains invalid or empty mesh data.",
                                        part.name
                                    ),
                                ));
                            }
                            if part
                                .material
                                .is_some_and(|index| index >= document.materials.len())
                            {
                                errors.push((
                                    "materialBindings".into(),
                                    format!("Part {} references a missing material.", part.name),
                                ));
                            }
                        }
                        for binding in &layer.material_bindings {
                            if !document
                                .materials
                                .iter()
                                .any(|m| m.id == binding.material_id)
                            {
                                errors.push((
                                    "materialBindings".into(),
                                    format!("Unknown material binding: {}", binding.material_id),
                                ));
                            }
                        }
                        for material in &effective.materials {
                            if material.color.iter().any(|v| !v.is_finite()) {
                                errors.push((
                                    "materialBindings".into(),
                                    format!("Material {} has an invalid color.", material.name),
                                ));
                            }
                            if let Err(error) =
                                texture_bytes(container, &asset, Some(material), layer)
                            {
                                errors.push((
                                    "materialBindings".into(),
                                    format!("{}: {error}", material.name),
                                ));
                            }
                        }
                        if options.target == "mtr3_nte" {
                            for (part_id, rule) in &layer.part_rules {
                                if options.only_visible && layer.hidden_parts.contains(part_id) {
                                    continue;
                                }
                                if !document.parts.iter().any(|p| &p.id == part_id) {
                                    errors.push((
                                        format!("partRules.{part_id}"),
                                        format!(
                                            "Placement rule references missing part {part_id}."
                                        ),
                                    ));
                                }
                                if let Err(error) = rule.expressions() {
                                    errors.push((format!("partRules.{part_id}"), error));
                                }
                            }
                        }
                    }
                }
                for (field, message) in errors {
                    issues.push(ValidationIssue {
                        severity: "error".into(),
                        message,
                        train_id: Some(train.id.clone()),
                        carriage_id: Some(carriage.id.clone()),
                        layer_id: Some(layer.id.clone()),
                        field: Some(field),
                    });
                }
            }
        }
    }
    Ok(issues)
}

pub fn export(
    container: &mut Container,
    path: &Path,
    options: &ExportOptions,
) -> Result<ExportReport, String> {
    let issues = validate(container, options)?;
    if issues.iter().any(|item| item.severity == "error") {
        return Err("Resolve the export validation errors before creating the pack.".into());
    }
    let trains = load_trains(container)?;
    let mut files = BTreeMap::new();
    files.insert("pack.mcmeta".into(), pretty(&json!({"pack":{"pack_format":pack_format(&options.target,&options.minecraft_version).unwrap(),"description":format!("{} — exported by MTR Pack Studio",container.index.name)}}))?);
    if let Some(hash) = container.index.cover_hash.clone() {
        files.insert(
            "pack.png".into(),
            crate::normalize_png(&container.read_blob(&hash)?)?,
        );
    }
    if options.target == "mtr4" {
        build_mtr4(
            container,
            &trains,
            &container.index.namespace.clone(),
            &options.model_format,
            options.only_visible,
            &mut files,
        )?;
    } else {
        build_mtr3(
            container,
            &trains,
            &container.index.namespace.clone(),
            options.only_visible,
            &mut files,
        )?;
    }
    write_zip(path, &files)?;
    Ok(ExportReport {
        path: path.to_string_lossy().into_owned(),
        file_count: files.len(),
        warnings: issues
            .into_iter()
            .filter(|item| item.severity == "warning")
            .collect(),
    })
}

fn build_mtr4(
    container: &mut Container,
    trains: &[TrainDefinition],
    namespace: &str,
    format: &str,
    only_visible: bool,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let mut vehicles = Vec::new();
    for train in trains {
        for carriage in &train.carriages {
            let base = export_base(&train.export_id, &carriage.export_id);
            let models = write_mtr4_layers(
                container,
                namespace,
                &base,
                "body",
                &carriage.body_models,
                format,
                only_visible,
                files,
            )?;
            let bogie1 = write_mtr4_layers(
                container,
                namespace,
                &base,
                "bogie1",
                &carriage.bogie_1_models,
                format,
                only_visible,
                files,
            )?;
            let bogie2 = write_mtr4_layers(
                container,
                namespace,
                &base,
                "bogie2",
                &carriage.bogie_2_models,
                format,
                only_visible,
                files,
            )?;
            let mut tags = train.tags.clone();
            tags.push(format!("family:{}", train.export_id));
            tags.push(format!("type:{}", carriage.export_id));
            tags.sort();
            tags.dedup();
            vehicles.push(json!({"id":format!("{}:{}",namespace,base),"name":format!("{} — {}",train.name,carriage.name),"color":train.color,"transportMode":"TRAIN","length":carriage.length,"width":carriage.width,"bogie1Position":carriage.bogie_1_position,"bogie2Position":carriage.bogie_2_position,"couplingPadding1":carriage.coupling_padding_1,"couplingPadding2":carriage.coupling_padding_2,"description":train.description,"wikipediaArticle":"","tags":tags,"models":models,"bogie1Models":bogie1,"bogie2Models":bogie2,"hasGangway1":carriage.end_1.gangway,"hasGangway2":carriage.end_2.gangway,"hasBarrier1":carriage.end_1.barrier,"hasBarrier2":carriage.end_2.barrier,"legacyRiderOffset":0,"bveSoundBaseResource":"","legacySpeedSoundBaseResource":"","legacySpeedSoundCount":1,"legacyUseAccelerationSoundsWhenCoasting":false,"legacyConstantPlaybackSpeed":false,"legacyDoorSoundBaseResource":"","legacyDoorCloseSoundTime":0}));
        }
    }
    files.insert(
        format!("assets/{namespace}/mtr_custom_resources.json"),
        pretty(&json!({"vehicles":vehicles}))?,
    );
    Ok(())
}

fn write_mtr4_layers(
    container: &mut Container,
    namespace: &str,
    base: &str,
    slot: &str,
    layers: &[ModelLayer],
    format: &str,
    only_visible: bool,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<Vec<Value>, String> {
    let mut output = Vec::new();
    for (layer_index, layer) in layers
        .iter()
        .enumerate()
        .filter(|(_, layer)| !only_visible || layer.visible)
    {
        let (asset, mut document) = load_asset(container, &layer.asset_id)?;
        apply_layer_edits(&mut document, layer, only_visible)?;
        name_parts(&mut document, &layer.id);
        let groups = material_groups(&document);
        for (group_index, mut parts) in groups {
            let group_name = group_index
                .map(|value| value.to_string())
                .unwrap_or_else(|| "default".into());
            let suffix = format!("{base}_{slot}_{layer_index}_{group_name}");
            for part in &mut parts {
                part.material = Some(0);
            }
            let filtered = ModelDocument {
                parts,
                materials: vec![ModelMaterial {
                    id: "material".into(),
                    name: "Material".into(),
                    color: [1.0; 4],
                    texture: None,
                    properties: Default::default(),
                }],
                warnings: Vec::new(),
            };
            let extension = if format == "mqo" { "mqo" } else { "obj" };
            let model_path = format!("assets/{namespace}/models/vehicle/{suffix}.{extension}");
            if format == "mqo" {
                files.insert(model_path, write_mqo(&filtered).into_bytes());
            } else {
                files.insert(
                    model_path,
                    write_obj(&filtered, &format!("{suffix}.mtl")).into_bytes(),
                );
                files.insert(
                    format!("assets/{namespace}/models/vehicle/{suffix}.mtl"),
                    write_mtl_single(&filtered, "default.png").into_bytes(),
                );
            }
            let material = group_index.and_then(|index| document.materials.get(index));
            let texture_resource = write_texture(
                container, &asset, material, namespace, &suffix, layer, files,
            )?;
            let properties = format!("assets/{namespace}/properties/vehicle/{suffix}.json");
            files.insert(properties, pretty(&mtr4_properties(&filtered))?);
            let positions = format!("assets/{namespace}/properties/definition/{suffix}.json");
            files.insert(positions,pretty(&json!({"positionDefinitions":[{"name":"origin","positions":[{"x":0,"y":0,"z":0}],"positionsFlipped":[]}]}))?);
            output.push(json!({"modelResource":format!("{namespace}:models/vehicle/{suffix}.{extension}"),"textureResource":texture_resource,"modelPropertiesResource":format!("{namespace}:properties/vehicle/{suffix}.json"),"positionDefinitionsResource":format!("{namespace}:properties/definition/{suffix}.json"),"flipTextureV":!layer.flip_texture_v}));
        }
    }
    Ok(output)
}

fn build_mtr3(
    container: &mut Container,
    trains: &[TrainDefinition],
    namespace: &str,
    only_visible: bool,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let mut custom = serde_json::Map::new();
    for train in trains {
        let mut segments = Vec::new();
        let mut property_parts = Vec::new();
        for carriage in &train.carriages {
            let (whitelist, blacklist) = carriage.placement.expressions()?;
            let base = export_base(&train.export_id, &carriage.export_id);
            let mut combined_parts = Vec::new();
            let mut combined_materials = Vec::new();
            let mut material_textures = Vec::new();
            for (layers, z_offset) in [
                (&carriage.body_models, 0.0),
                (&carriage.bogie_1_models, carriage.bogie_1_position),
                (&carriage.bogie_2_models, carriage.bogie_2_position),
            ] {
                for layer in layers.iter().filter(|layer| !only_visible || layer.visible) {
                    let (asset, mut document) = load_asset(container, &layer.asset_id)?;
                    apply_layer_edits(&mut document, layer, only_visible)?;
                    name_parts(&mut document, &layer.id);
                    // NTE splits OBJ by material groups, including otherwise untextured geometry.
                    if document.parts.iter().any(|part| part.material.is_none()) {
                        let index = document.materials.len();
                        document.materials.push(ModelMaterial {
                            id: "__default".into(),
                            name: "Default".into(),
                            color: [1.0; 4],
                            texture: None,
                            properties: Default::default(),
                        });
                        for part in &mut document.parts {
                            if part.material.is_none() {
                                part.material = Some(index);
                            }
                        }
                    }
                    let offset = combined_materials.len();
                    for material in &document.materials {
                        material_textures.push(write_texture(
                            container,
                            &asset,
                            Some(material),
                            namespace,
                            &base,
                            layer,
                            files,
                        )?);
                    }
                    for part in &mut document.parts {
                        part.material = part.material.map(|index| index + offset);
                        for position in &mut part.positions {
                            position[2] += z_offset;
                        }
                        if !layer.flip_texture_v {
                            for uv in &mut part.texcoords {
                                uv[1] = 1.0 - uv[1];
                            }
                        }
                        // Qualified names replace inheritance rather than drawing an extra copy.
                        let (white, black) = layer
                            .part_rules
                            .get(&part.id)
                            .unwrap_or(&carriage.placement)
                            .expressions()?;
                        property_parts.push(mtr3_part(
                            format!("{base}.obj/{}", part.name),
                            &white,
                            &black,
                        ));
                    }
                    combined_parts.extend(document.parts);
                    combined_materials.extend(document.materials);
                }
            }
            let document = ModelDocument {
                parts: combined_parts,
                materials: combined_materials,
                warnings: Vec::new(),
            };
            files.insert(
                format!("assets/{namespace}/models/vehicle/{base}.obj"),
                write_obj(&document, &format!("{base}.mtl")).into_bytes(),
            );
            files.insert(
                format!("assets/{namespace}/models/vehicle/{base}.mtl"),
                write_mtl(&document, &material_textures).into_bytes(),
            );
            // Paired syntax is also used for one carriage. Preview instances never affect exports.
            segments.push(format!(
                "{namespace}:models/vehicle/{base}.obj|{whitelist};{blacklist};"
            ));
        }
        let properties_name = format!("{}_properties.json", train.export_id);
        files.insert(format!("assets/{namespace}/models/vehicle/{properties_name}"), pretty(&json!({"transport_mode":"train","length":train.carriages[0].length,"width":train.carriages[0].width,"door_max":0,"parts":property_parts}))?);
        custom.insert(train.export_id.clone(),json!({"name":train.name,"description":train.description,"color":train.color,"base_train_type":train.mtr3_base_train_type,"model":segments.join("|"),"model_properties":format!("{namespace}:models/vehicle/{properties_name}"),"texture_id":format!("{namespace}:textures/white.png"),"flipV":false}));
    }
    files.insert(
        format!("assets/{namespace}/textures/white.png"),
        solid_png([1.0; 4])?,
    );
    files.insert(
        format!("assets/{namespace}/mtr_custom_resources.json"),
        pretty(&json!({"custom_trains":custom}))?,
    );
    Ok(())
}

fn export_base(train: &str, carriage: &str) -> String {
    let hash = blake3::hash(format!("{train}\0{carriage}").as_bytes()).to_hex();
    format!("{train}_{carriage}_{}", &hash[..12])
}

// Three.js Euler XYZ uses Rx * Ry * Rz; vectors are therefore rotated Z, Y, X.
fn rotate_vector(mut vector: [f32; 3], rotation: [f32; 3]) -> [f32; 3] {
    let (s, c) = rotation[2].sin_cos();
    vector = [
        c * vector[0] - s * vector[1],
        s * vector[0] + c * vector[1],
        vector[2],
    ];
    let (s, c) = rotation[1].sin_cos();
    vector = [
        c * vector[0] + s * vector[2],
        vector[1],
        -s * vector[0] + c * vector[2],
    ];
    let (s, c) = rotation[0].sin_cos();
    [
        vector[0],
        c * vector[1] - s * vector[2],
        s * vector[1] + c * vector[2],
    ]
}

fn transform_part(part: &mut ModelPart, transform: &ModelTransform) -> Result<(), String> {
    transform.validate()?;
    for position in &mut part.positions {
        let scaled = std::array::from_fn(|axis| position[axis] * transform.scale[axis]);
        let rotated = rotate_vector(scaled, transform.rotation);
        *position = std::array::from_fn(|axis| rotated[axis] + transform.translation[axis]);
        if position.iter().any(|value| !value.is_finite()) {
            return Err("Model transform exceeds supported coordinate range.".into());
        }
    }
    // Inverse transpose of R*S is R*inverse(S). Positive scales preserve winding.
    for normal in &mut part.normals {
        let inverse_scaled = std::array::from_fn(|axis| normal[axis] / transform.scale[axis]);
        let rotated = rotate_vector(inverse_scaled, transform.rotation);
        let length = rotated
            .iter()
            .map(|value| (*value as f64).powi(2))
            .sum::<f64>()
            .sqrt();
        if !length.is_finite() {
            return Err("Model transform exceeds supported normal range.".into());
        }
        *normal = if length > 0.0 {
            rotated.map(|value| (value as f64 / length) as f32)
        } else {
            [0.0; 3]
        };
    }
    Ok(())
}

fn apply_layer_edits(
    document: &mut ModelDocument,
    layer: &ModelLayer,
    only_visible: bool,
) -> Result<(), String> {
    for material in &mut document.materials {
        if let Some(binding) = layer
            .material_bindings
            .iter()
            .find(|b| b.material_id == material.id)
        {
            binding.properties.validate()?;
            material.properties.overlay(&binding.properties);
            if let Some(opacity) = binding.properties.opacity {
                material.color[3] = opacity;
            }
        }
    }
    document
        .parts
        .retain(|part| !only_visible || !layer.hidden_parts.contains(&part.id));
    for part in &mut document.parts {
        if let Some(transform) = layer.part_transforms.get(&part.id) {
            transform_part(part, transform)?;
        }
        transform_part(part, &layer.transform)?;
    }
    // Remove unused materials so hidden parts do not require unavailable textures.
    let used: BTreeSet<usize> = document
        .parts
        .iter()
        .filter_map(|part| part.material)
        .collect();
    let mut indices = BTreeMap::new();
    let mut materials = Vec::new();
    for (index, material) in document.materials.iter().enumerate() {
        if used.contains(&index) {
            indices.insert(index, materials.len());
            materials.push(material.clone());
        }
    }
    for part in &mut document.parts {
        if let Some(index) = part.material {
            part.material = Some(
                *indices
                    .get(&index)
                    .ok_or_else(|| "Part references a missing material.".to_string())?,
            );
        }
    }
    document.materials = materials;
    Ok(())
}

fn name_parts(document: &mut ModelDocument, layer_id: &str) {
    for (index, part) in document.parts.iter_mut().enumerate() {
        let hash = blake3::hash(format!("{layer_id}\0{}\0{index}", part.id).as_bytes()).to_hex();
        part.name = format!("{}_{}", safe_name(&part.name), &hash[..12]);
    }
}

fn material_groups(document: &ModelDocument) -> Vec<(Option<usize>, Vec<ModelPart>)> {
    let mut map: BTreeMap<Option<usize>, Vec<ModelPart>> = BTreeMap::new();
    for part in &document.parts {
        map.entry(part.material).or_default().push(part.clone());
    }
    map.into_iter().collect()
}
fn mtr4_properties(document: &ModelDocument) -> Value {
    let parts:Vec<_>=document.parts.iter().map(|part|json!({"names":[safe_name(&part.name)],"positionDefinitions":["origin"],"condition":"NORMAL","renderStage":"EXTERIOR","type":"NORMAL","displayXPadding":0,"displayYPadding":0,"displayColorCjk":"FFFFFF","displayColor":"FFFFFF","displayMaxLineHeight":0,"displayCjkSizeRatio":1,"displayPadZeros":0,"displayType":"DESTINATION","displayDefaultText":"","doorXMultiplier":0,"doorZMultiplier":0,"doorAnimationType":"STANDARD","renderFromOpeningDoorTime":0,"renderUntilOpeningDoorTime":0,"renderFromClosingDoorTime":0,"renderUntilClosingDoorTime":0,"flashOffTime":0,"flashOnTime":0})).collect();
    json!({"parts":parts,"modelYOffset":0,"gangwayInnerSideResource":"","gangwayInnerTopResource":"","gangwayInnerBottomResource":"","gangwayOuterSideResource":"","gangwayOuterTopResource":"","gangwayOuterBottomResource":"","gangwayWidth":0,"gangwayHeight":0,"gangwayYOffset":0,"gangwayZOffset":0,"barrierInnerSideResource":"","barrierInnerTopResource":"","barrierInnerBottomResource":"","barrierOuterSideResource":"","barrierOuterTopResource":"","barrierOuterBottomResource":"","barrierWidth":0,"barrierHeight":0,"barrierYOffset":0,"barrierZOffset":0})
}
fn mtr3_part(name: String, whitelist: &str, blacklist: &str) -> Value {
    json!({"name":name,"stage":"exterior","mirror":false,"skip_rendering_if_too_far":false,"door_offset":"none","render_condition":"all","positions":[[0,0]],"whitelisted_cars":whitelist,"blacklisted_cars":blacklist})
}

fn write_obj(document: &ModelDocument, mtl: &str) -> String {
    let mut out = format!("# Exported by MTR Pack Studio\nmtllib {mtl}\n");
    let (mut vertex_offset, mut uv_offset, mut normal_offset) = (1u32, 1u32, 1u32);
    for part in &document.parts {
        out += &format!("o {}\ng {}\n", safe_name(&part.name), safe_name(&part.name));
        if let Some(index) = part.material {
            out += &format!("usemtl material_{index}\n");
        }
        for p in &part.positions {
            out += &format!("v {} {} {}\n", -p[0], p[1], p[2]);
        }
        for uv in &part.texcoords {
            out += &format!("vt {} {}\n", uv[0], uv[1]);
        }
        for n in &part.normals {
            out += &format!("vn {} {} {}\n", -n[0], n[1], n[2]);
        }
        let has_uv = part.texcoords.len() == part.positions.len();
        let has_normal = part.normals.len() == part.positions.len();
        for tri in part.indices.chunks_exact(3) {
            out.push('f');
            for index in [tri[0], tri[2], tri[1]] {
                out += &format!(" {}", vertex_offset + index);
                if has_uv || has_normal {
                    out.push('/');
                    if has_uv {
                        out += &(uv_offset + index).to_string();
                    }
                }
                if has_normal {
                    out += &format!("/{}", normal_offset + index);
                }
            }
            out.push('\n');
        }
        vertex_offset += part.positions.len() as u32;
        uv_offset += part.texcoords.len() as u32;
        normal_offset += part.normals.len() as u32;
    }
    out
}

fn write_mtl(document: &ModelDocument, textures: &[String]) -> String {
    let mut out = String::from("# Exported by MTR Pack Studio\n");
    for index in 0..document.materials.len() {
        out += &format!(
            "newmtl material_{index}\nKd 1 1 1\nd 1\nmap_Kd {}\n\n",
            textures[index]
        );
    }
    out
}
fn write_mtl_single(document: &ModelDocument, texture: &str) -> String {
    let mut out = String::from("# Exported by MTR Pack Studio\n");
    for index in 0..document.materials.len() {
        out += &format!(
            "newmtl material_{index}\nKd {} {} {}\nd {}\nmap_Kd {texture}\n\n",
            1.0, 1.0, 1.0, 1.0
        );
    }
    out
}
fn write_mqo(document: &ModelDocument) -> String {
    let mut out = String::from("Metasequoia Document\nFormat Text Ver 1.0\n\n");
    out += &format!("Material {} {{\n", document.materials.len());
    for material in &document.materials {
        out += &format!(
            " \"{}\" col(1 1 1 1) dif(1) tex(\"default.png\")\n",
            safe_name(&material.name)
        );
    }
    out += "}\n";
    for part in &document.parts {
        out += &format!(
            "Object \"{}\" {{\n vertex {} {{\n",
            safe_name(&part.name),
            part.positions.len()
        );
        // MTR 4.0.5 MQO conversion uses centimetres, and reverses each face.
        for p in &part.positions {
            out += &format!("  {} {} {}\n", -p[0] * 100.0, p[1] * 100.0, p[2] * 100.0);
        }
        out += " }\n";
        out += &format!(" face {} {{\n", part.indices.len() / 3);
        for tri in part.indices.chunks_exact(3) {
            out += &format!("  3 V({} {} {})", tri[0], tri[1], tri[2]);
            if let Some(m) = part.material {
                out += &format!(" M({m})");
            }
            if part.texcoords.len() == part.positions.len() {
                out += " UV(";
                for (i, index) in tri.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    let uv = part.texcoords[*index as usize];
                    out += &format!("{} {}", uv[0], uv[1]);
                }
                out.push(')');
            }
            out.push('\n');
        }
        out += " }\n}\n";
    }
    out += "Eof\n";
    out
}

fn texture_bytes(
    container: &mut Container,
    asset: &AssetDefinition,
    material: Option<&ModelMaterial>,
    layer: &ModelLayer,
) -> Result<Vec<u8>, String> {
    let color = material.map(|value| value.color).unwrap_or([1.0; 4]);
    let binding = material
        .and_then(|m| {
            layer
                .material_bindings
                .iter()
                .find(|b| b.material_id == m.id)
        })
        .and_then(|b| b.texture_asset_id.as_ref());
    let data = if let Some(hash) = binding {
        Some(
            container
                .read_blob(hash)
                .map_err(|e| format!("Replacement texture is missing: {e}"))?,
        )
    } else if let Some(texture) = material.and_then(|m| m.texture.as_ref()) {
        let normalized = texture.replace('\\', "/");
        let name = normalized.rsplit('/').next().unwrap_or(&normalized);
        let exact = asset.dependencies.iter().find(|dep| {
            dep.name
                .replace('\\', "/")
                .eq_ignore_ascii_case(&normalized)
        });
        let matches: Vec<_> = asset
            .dependencies
            .iter()
            .filter(|dep| {
                dep.name
                    .replace('\\', "/")
                    .rsplit('/')
                    .next()
                    .is_some_and(|v| v.eq_ignore_ascii_case(name))
            })
            .collect();
        let dependency = exact
            .or_else(|| {
                if matches.len() == 1 {
                    Some(matches[0])
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                format!("Texture {texture} is missing or ambiguous; assign a replacement texture.")
            })?;
        Some(container.read_blob(&dependency.hash)?)
    } else {
        None
    };
    if let Some(bytes) = data {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        let mut image = reader
            .decode()
            .map_err(|e| format!("Cannot decode texture: {e}"))?
            .to_rgba8();
        for pixel in image.pixels_mut() {
            for channel in 0..4 {
                pixel[channel] =
                    (pixel[channel] as f32 * color[channel].clamp(0.0, 1.0)).round() as u8;
            }
        }
        let mut output = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(output.into_inner())
    } else {
        solid_png(color)
    }
}
fn write_texture(
    container: &mut Container,
    asset: &AssetDefinition,
    material: Option<&ModelMaterial>,
    namespace: &str,
    _suffix: &str,
    layer: &ModelLayer,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<String, String> {
    let bytes = texture_bytes(container, asset, material, layer)?;
    let hash = blake3::hash(&bytes).to_hex();
    let path = format!("textures/vehicle/{hash}.png");
    files.insert(format!("assets/{namespace}/{path}"), bytes);
    Ok(format!("{namespace}:{path}"))
}

fn solid_png(color: [f32; 4]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        let pixel = color.map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8);
        writer.write_image_data(&pixel).map_err(|e| e.to_string())?;
    }
    Ok(bytes)
}
fn write_zip(path: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for name in files.keys() {
        if name.contains('\\')
            || name.starts_with('/')
            || name
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || name.contains(':')
        {
            return Err(format!("Unsafe archive path: {name}"));
        }
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".mtr-export-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let file = File::create(&temporary).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (name, bytes) in files {
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        let file = zip.finish().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        crate::container::atomic_replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn issue(
    issues: &mut Vec<ValidationIssue>,
    severity: &str,
    message: &str,
    train_id: Option<&str>,
    carriage_id: Option<&str>,
    field: Option<&str>,
) {
    issues.push(ValidationIssue {
        severity: severity.into(),
        message: message.into(),
        train_id: train_id.map(Into::into),
        carriage_id: carriage_id.map(Into::into),
        layer_id: None,
        field: field.map(Into::into),
    })
}
fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
}
fn safe_name(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if value.is_empty() {
        "part".into()
    } else {
        value
    }
}
fn pack_format(target: &str, version: &str) -> Option<u32> {
    match (target, version) {
        ("mtr4", "1.20.4") => Some(22),
        ("mtr3_nte", "1.16.5") => Some(6),
        ("mtr3_nte", "1.17.1") => Some(7),
        ("mtr3_nte", "1.18.2") => Some(8),
        ("mtr3_nte", "1.19.2") => Some(9),
        ("mtr3_nte", "1.19.3") => Some(12),
        ("mtr3_nte", "1.19.4") => Some(13),
        ("mtr3_nte", "1.20.1") => Some(15),
        _ => None,
    }
}
fn pretty(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        container::ContentEntry,
        domain::{ModelFormat, ModelLayer, TrainDefinition},
    };
    use std::io::Read;
    fn document() -> ModelDocument {
        ModelDocument {
            parts: vec![ModelPart {
                id: "p".into(),
                name: "body shell".into(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![],
                texcoords: vec![],
                indices: vec![0, 1, 2],
                material: None,
            }],
            materials: vec![],
            warnings: vec![],
        }
    }
    #[test]
    fn transforms_match_xyz_euler_and_inverse_transpose_normals() {
        let old_layer: ModelLayer =
            serde_json::from_value(json!({"id":"old", "name":"Old", "assetId":"asset"})).unwrap();
        assert_eq!(old_layer.transform, ModelTransform::default());
        assert!(old_layer.hidden_parts.is_empty());
        let mut document = document();
        document.parts[0].normals = vec![[1.0, 1.0, 0.0]; 3];
        let mut layer = old_layer;
        layer.transform.translation = [10.0, 20.0, 30.0];
        layer.transform.rotation = [0.0, 0.0, std::f32::consts::FRAC_PI_2];
        layer.transform.scale = [2.0, 1.0, 1.0];
        layer.part_transforms.insert(
            "p".into(),
            ModelTransform {
                translation: [1.0, 0.0, 0.0],
                ..Default::default()
            },
        );
        apply_layer_edits(&mut document, &layer, false).unwrap();
        let part = &document.parts[0];
        assert_eq!(part.positions[0], [10.0, 22.0, 30.0]);
        assert!((part.normals[0][0] + 2.0 / 5.0f32.sqrt()).abs() < 0.00001);
        assert!((part.normals[0][1] - 1.0 / 5.0f32.sqrt()).abs() < 0.00001);
        assert_eq!(part.indices, [0, 1, 2]);
        let rotation = rotate_vector([1.0, 2.0, 3.0], [std::f32::consts::FRAC_PI_2; 3]);
        for (actual, expected) in rotation.into_iter().zip([3.0, -2.0, 1.0]) {
            assert!((actual - expected).abs() < 0.00001);
        }
        layer.transform.scale[0] = 0.0;
        assert!(layer.transform.validate().is_err());
        layer.transform.scale[0] = -1.0;
        assert!(layer.transform.validate().is_err());
        layer.transform.scale[0] = f32::NAN;
        assert!(layer.transform.validate().is_err());
    }

    #[test]
    fn both_exporters_apply_visibility_and_layer_and_part_transforms() {
        let mut fixture = Fixture::new();
        let body = &mut fixture.train.carriages[0].body_models[0];
        body.hidden_parts = vec!["p2".into(), "stale-part".into()];
        body.transform.translation = [10.0, 20.0, 30.0];
        body.transform.scale = [2.0, 3.0, 4.0];
        body.part_transforms.insert(
            "p".into(),
            ModelTransform {
                translation: [1.0, 0.0, 0.0],
                ..Default::default()
            },
        );
        let bogie = &mut fixture.train.carriages[0].bogie_1_models[0];
        bogie.visible = false;
        bogie.asset_id = "missing-but-hidden".into();
        fixture.save();
        for option in [
            options("mtr4", "obj"),
            options("mtr4", "mqo"),
            options("mtr3_nte", "obj"),
        ] {
            let output = fixture.root.join("visibility.zip");
            let option = ExportOptions {
                only_visible: true,
                ..option
            };
            export(&mut fixture.container, &output, &option).unwrap();
            let files = unzip(&output);
            let models: Vec<_> = files
                .iter()
                .filter(|(path, _)| path.ends_with(".obj") || path.ends_with(".mqo"))
                .collect();
            assert_eq!(models.len(), 1);
            let model = std::str::from_utf8(models[0].1).unwrap();
            if option.model_format == "mqo" {
                assert!(model.contains("1200 2000 3000"), "{model}");
                assert_eq!(model.matches("Object ").count(), 1);
            } else {
                // Export coordinate conversion negates canonical X.
                assert!(model.contains("v -12 20 30"), "{model}");
                assert_eq!(
                    model.lines().filter(|line| line.starts_with("v ")).count(),
                    3
                );
                assert_eq!(
                    model.lines().filter(|line| line.starts_with("g ")).count(),
                    1
                );
            }
        }
        let body = &mut fixture.train.carriages[0].body_models[0];
        body.hidden_parts.push("p".into());
        fixture.save();
        assert!(validate(
            &mut fixture.container,
            &ExportOptions {
                only_visible: true,
                ..options("mtr4", "obj")
            }
        )
        .unwrap()
        .iter()
        .any(|issue| issue.field.as_deref() == Some("bodyModels")));
        fixture.finish();
    }

    #[test]
    fn export_includes_hidden_geometry_by_default() {
        let legacy: ExportOptions = serde_json::from_value(
            json!({"target":"mtr4", "minecraftVersion":"1.20.4", "modelFormat":"obj"}),
        )
        .unwrap();
        assert!(!legacy.only_visible);
        let mut fixture = Fixture::new();
        fixture.train.carriages[0].body_models[0]
            .transform
            .translation = [3.0, 4.0, 5.0];
        fixture.save();
        for option in [
            options("mtr4", "obj"),
            options("mtr4", "mqo"),
            options("mtr3_nte", "obj"),
        ] {
            let output = fixture.root.join("default-visibility.zip");
            export(&mut fixture.container, &output, &option).unwrap();
            let original = unzip(&output);
            fixture.train.carriages[0].body_models[0].hidden_parts = vec!["p".into(), "p2".into()];
            fixture.train.carriages[0].body_models[0].visible = false;
            fixture.train.carriages[0].bogie_1_models[0].visible = false;
            fixture.save();
            export(&mut fixture.container, &output, &option).unwrap();
            assert_eq!(original, unzip(&output));
            let visible_only = ExportOptions {
                only_visible: true,
                ..option
            };
            assert!(validate(&mut fixture.container, &visible_only)
                .unwrap()
                .iter()
                .any(|issue| issue.field.as_deref() == Some("bodyModels")));
            fixture.train.carriages[0].body_models[0]
                .hidden_parts
                .clear();
            fixture.train.carriages[0].body_models[0].visible = true;
            fixture.train.carriages[0].bogie_1_models[0].visible = true;
            fixture.save();
        }
        fixture.finish();
    }

    #[test]
    fn pack_versions_are_pinned() {
        assert_eq!(pack_format("mtr4", "1.20.4"), Some(22));
        assert_eq!(pack_format("mtr3_nte", "1.16.5"), Some(6));
        assert_eq!(pack_format("mtr3_nte", "1.21"), None);
    }
    #[test]
    fn obj_writer_keeps_groups() {
        let obj = write_obj(&document(), "test.mtl");
        assert!(obj.contains("g body_shell"));
        assert!(obj.contains("f 1 3 2"));
    }
    #[test]
    fn both_exporters_create_complete_archives() {
        let root = std::env::temp_dir().join(format!("mtr-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let project = root.join("test.mtrpack");
        let mut container = Container::create(&project, "Export Test").unwrap();
        let doc = document();
        let document_hash = container
            .put_blob(
                &rmp_serde::to_vec_named(&doc).unwrap(),
                "application/vnd.mtrpack.model+msgpack",
            )
            .unwrap();
        let source_hash = container.put_blob(b"model", "model/obj").unwrap();
        let preview_hash = container.put_blob(b"glTF", "model/gltf-binary").unwrap();
        let asset = AssetDefinition {
            id: "asset".into(),
            name: "Model".into(),
            source_format: ModelFormat::Obj,
            source_hash,
            document_hash,
            preview_hash,
            dependencies: vec![],
            parts: vec![],
            warnings: vec![],
        };
        let asset_hash = container
            .put_blob(
                &serde_json::to_vec(&asset).unwrap(),
                "application/vnd.mtrpack.asset+json",
            )
            .unwrap();
        container.index.assets.insert(asset.id.clone(), asset_hash);
        let mut train = TrainDefinition::new("Train", "train");
        train.mtr3_base_train_type = "sp1900".into();
        train.carriages[0].body_models.push(ModelLayer {
            id: "layer".into(),
            name: "Body".into(),
            asset_id: "asset".into(),
            flip_texture_v: false,
            visible: true,
            material_bindings: vec![],
            part_rules: BTreeMap::new(),
            hidden_parts: vec![],
            transform: Default::default(),
            part_transforms: BTreeMap::new(),
        });
        let train_hash = container
            .put_blob(
                &serde_json::to_vec(&train).unwrap(),
                "application/vnd.mtrpack.train+json",
            )
            .unwrap();
        container.index.content.push(ContentEntry {
            id: train.id.clone(),
            kind: "train".into(),
            name: train.name.clone(),
            file: "content/train.json".into(),
            updated_at: 0,
            resources: vec![train_hash],
        });
        container.commit().unwrap();
        for options in [
            ExportOptions {
                target: "mtr4".into(),
                minecraft_version: "1.20.4".into(),
                model_format: "obj".into(),
                only_visible: false,
            },
            ExportOptions {
                target: "mtr3_nte".into(),
                minecraft_version: "1.20.1".into(),
                model_format: "obj".into(),
                only_visible: false,
            },
        ] {
            let output = root.join(format!("{}.zip", options.target));
            export(&mut container, &output, &options).unwrap();
            let mut archive = zip::ZipArchive::new(File::open(&output).unwrap()).unwrap();
            let mut mcmeta = String::new();
            archive
                .by_name("pack.mcmeta")
                .unwrap()
                .read_to_string(&mut mcmeta)
                .unwrap();
            assert!(mcmeta.contains("Export Test"));
            assert!(archive
                .by_name("assets/export_test/mtr_custom_resources.json")
                .is_ok());
        }
        drop(container);
        fs::remove_dir_all(root).unwrap();
    }
    struct Fixture {
        root: std::path::PathBuf,
        container: Container,
        train: TrainDefinition,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("mtr-export-contract-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let mut container =
                Container::create(&root.join("project.mtrpack"), "Export Test").unwrap();
            let mut doc = document();
            doc.parts[0].material = Some(0);
            doc.parts[0].texcoords = vec![[0.1, 0.2], [0.8, 0.3], [0.4, 0.9]];
            doc.parts[0].normals = vec![[0.0, 0.0, 1.0]; 3];
            doc.materials.push(ModelMaterial {
                id: "material".into(),
                name: "Paint".into(),
                color: [0.5, 1.0, 1.0, 1.0],
                texture: Some("paint.jpg".into()),
                properties: Default::default(),
            });
            let mut second = doc.parts[0].clone();
            second.id = "p2".into();
            second.material = Some(1);
            doc.parts.push(second);
            doc.materials.push(ModelMaterial {
                id: "solid".into(),
                name: "Solid".into(),
                color: [0.2, 0.3, 0.4, 1.0],
                texture: None,
                properties: Default::default(),
            });
            let mut jpeg = std::io::Cursor::new(Vec::new());
            image::RgbImage::from_pixel(2, 2, image::Rgb([200, 100, 50]))
                .write_to(&mut jpeg, image::ImageFormat::Jpeg)
                .unwrap();
            let texture = container
                .put_blob(&jpeg.into_inner(), "image/jpeg")
                .unwrap();
            let document_hash = container
                .put_blob(
                    &rmp_serde::to_vec_named(&doc).unwrap(),
                    "application/vnd.mtrpack.model+msgpack",
                )
                .unwrap();
            let source_hash = container.put_blob(b"source", "model/obj").unwrap();
            let preview_hash = container.put_blob(b"preview", "model/gltf-binary").unwrap();
            let asset = AssetDefinition {
                id: "asset".into(),
                name: "Asset".into(),
                source_format: ModelFormat::Obj,
                source_hash,
                document_hash,
                preview_hash,
                dependencies: vec![crate::domain::AssetDependency {
                    name: "paint.jpg".into(),
                    hash: texture,
                    media_type: "image/jpeg".into(),
                }],
                parts: vec![],
                warnings: vec![],
            };
            let hash = container
                .put_blob(
                    &serde_json::to_vec(&asset).unwrap(),
                    "application/vnd.mtrpack.asset+json",
                )
                .unwrap();
            container.index.assets.insert("asset".into(), hash);
            let mut train = TrainDefinition::new("Test Train", "train");
            train.mtr3_base_train_type = "sp1900".into();
            let layer = ModelLayer {
                id: "body-layer".into(),
                name: "Body".into(),
                asset_id: "asset".into(),
                flip_texture_v: false,
                visible: true,
                material_bindings: vec![],
                part_rules: BTreeMap::new(),
                hidden_parts: vec![],
                transform: Default::default(),
                part_transforms: BTreeMap::new(),
            };
            train.carriages[0].body_models.push(layer.clone());
            let mut bogie = layer;
            bogie.id = "bogie-layer".into();
            train.carriages[0].bogie_1_models.push(bogie);
            train.carriages[0].placement.preset = crate::domain::PlacementPreset::First;
            train.carriages[0].body_models[0].part_rules.insert(
                "p2".into(),
                crate::domain::CarPlacementRule {
                    preset: crate::domain::PlacementPreset::Last,
                    ..Default::default()
                },
            );
            let mut fixture = Self {
                root,
                container,
                train,
            };
            fixture.save();
            fixture
        }
        fn save(&mut self) {
            let hash = self
                .container
                .put_blob(
                    &serde_json::to_vec(&self.train).unwrap(),
                    "application/vnd.mtrpack.train+json",
                )
                .unwrap();
            self.container.index.content = vec![ContentEntry {
                id: self.train.id.clone(),
                kind: "train".into(),
                name: self.train.name.clone(),
                file: "train.json".into(),
                updated_at: 0,
                resources: vec![hash],
            }];
            self.container.commit().unwrap();
        }
        fn finish(self) {
            let Self {
                root, container, ..
            } = self;
            drop(container);
            fs::remove_dir_all(root).unwrap();
        }
    }
    fn options(target: &str, format: &str) -> ExportOptions {
        ExportOptions {
            target: target.into(),
            minecraft_version: if target == "mtr4" { "1.20.4" } else { "1.20.1" }.into(),
            model_format: format.into(),
            only_visible: false,
        }
    }
    fn unzip(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut files = BTreeMap::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).unwrap();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            files.insert(entry.name().into(), bytes);
        }
        files
    }
    fn resource_path(resource: &str) -> String {
        let (namespace, path) = resource.split_once(':').unwrap();
        format!("assets/{namespace}/{path}")
    }
    fn read_json(files: &BTreeMap<String, Vec<u8>>, path: &str) -> Value {
        serde_json::from_slice(&files[path]).unwrap()
    }
    // The pinned official schemas use primitive types, local references, required fields and enum annotations.
    fn check_schema(value: &Value, name: &str) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mtr4-4.0.5");
        let schema: Value = serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap();
        check_schema_node(value, &schema, &root);
    }
    fn check_schema_node(value: &Value, schema: &Value, root: &Path) {
        if let Some(reference) = schema["$ref"].as_str() {
            if reference.ends_with(".json") {
                let schema: Value =
                    serde_json::from_slice(&fs::read(root.join(reference)).unwrap()).unwrap();
                check_schema_node(value, &schema, root);
            } else if let Some(enumeration) = schema["typeScriptEnum"].as_str() {
                assert!(
                    enumeration.split('|').any(|v| Some(v) == value.as_str()),
                    "Invalid enum {value}"
                );
            }
        }
        if let Some(required) = schema["required"].as_array() {
            for field in required {
                assert!(
                    value.get(field.as_str().unwrap()).is_some(),
                    "Missing schema field {field}"
                );
            }
        }
        match schema["type"].as_str() {
            Some("object") => {
                assert!(value.is_object());
                if let Some(properties) = schema["properties"].as_object() {
                    for (key, field) in properties {
                        if let Some(data) = value.get(key) {
                            check_schema_node(data, field, root);
                        }
                    }
                }
            }
            Some("array") => {
                for item in value.as_array().expect("array") {
                    check_schema_node(item, &schema["items"], root);
                }
            }
            Some("string") => assert!(value.is_string()),
            Some("boolean") => assert!(value.is_boolean()),
            Some("number") | Some("integer") => {
                let n = value.as_f64().expect("number");
                if schema["type"] == "integer" {
                    assert!(n.fract() == 0.0);
                }
                if let Some(min) = schema["minimum"].as_f64() {
                    assert!(n >= min);
                }
            }
            _ => (),
        }
    }
    #[test]
    fn archives_match_upstream_contract_and_resolve_all_resources() {
        let mut fixture = Fixture::new();
        let cover = solid_png([0.1, 0.2, 0.3, 1.0]).unwrap();
        fixture.container.index.cover_hash =
            Some(fixture.container.put_blob(&cover, "image/png").unwrap());
        for option in [
            options("mtr4", "obj"),
            options("mtr4", "mqo"),
            options("mtr3_nte", "obj"),
        ] {
            let output = fixture.root.join("output.zip");
            export(&mut fixture.container, &output, &option).unwrap();
            let files = unzip(&output);
            assert!(files.contains_key("pack.png"));
            for (path, bytes) in &files {
                assert!(!path.contains(".."));
                if path.ends_with(".png") {
                    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
                    image::load_from_memory(bytes).unwrap();
                }
            }
            for (path, bytes) in &files {
                if path.ends_with(".mtl") {
                    for line in std::str::from_utf8(bytes)
                        .unwrap()
                        .lines()
                        .filter_map(|line| line.strip_prefix("map_Kd "))
                    {
                        if line != "default.png" {
                            assert!(files.contains_key(&resource_path(line)));
                        }
                    }
                }
            }
            let manifest = read_json(&files, "assets/export_test/mtr_custom_resources.json");
            if option.target == "mtr4" {
                check_schema(&manifest, "customResources.json");
                let vehicle = &manifest["vehicles"][0];
                assert_eq!(vehicle["bogie1Position"], 7.0);
                assert_eq!(vehicle["models"].as_array().unwrap().len(), 2);
                for model in vehicle["models"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .chain(vehicle["bogie1Models"].as_array().unwrap())
                {
                    for field in [
                        "modelResource",
                        "textureResource",
                        "modelPropertiesResource",
                        "positionDefinitionsResource",
                    ] {
                        assert!(files.contains_key(&resource_path(model[field].as_str().unwrap())));
                    }
                    assert_eq!(model["flipTextureV"], true);
                    let properties = read_json(
                        &files,
                        &resource_path(model["modelPropertiesResource"].as_str().unwrap()),
                    );
                    check_schema(&properties, "modelProperties.json");
                    let positions = read_json(
                        &files,
                        &resource_path(model["positionDefinitionsResource"].as_str().unwrap()),
                    );
                    check_schema(&positions, "positionDefinitions.json");
                    assert_eq!(
                        positions["positionDefinitions"][0]["positions"][0],
                        json!({"x":0,"y":0,"z":0})
                    );
                    let model_text = String::from_utf8(
                        files[&resource_path(model["modelResource"].as_str().unwrap())].clone(),
                    )
                    .unwrap();
                    for part in properties["parts"].as_array().unwrap() {
                        assert_eq!(part["positionDefinitions"], json!(["origin"]));
                        assert!(model_text.contains(part["names"][0].as_str().unwrap()));
                    }
                    if option.model_format == "mqo" {
                        assert!(model_text.contains("UV("));
                        assert!(model_text.contains("tex(\"default.png\")"));
                    }
                }
            } else {
                let train = &manifest["custom_trains"]["train"];
                let segments: Vec<_> = train["model"].as_str().unwrap().split('|').collect();
                assert_eq!(segments.len(), 2);
                assert_eq!(segments[1], "1;%1;");
                let obj = String::from_utf8(files[&resource_path(segments[0])].clone()).unwrap();
                assert!(obj.contains("v -0 0 7"));
                assert!(obj.contains("vt 0.1 0.8"));
                let properties = read_json(
                    &files,
                    &resource_path(train["model_properties"].as_str().unwrap()),
                );
                let mut names = BTreeSet::new();
                for part in properties["parts"].as_array().unwrap() {
                    let name = part["name"].as_str().unwrap();
                    assert!(names.insert(name));
                    assert_eq!(part["positions"], json!([[0, 0]]));
                    assert!(obj.contains(&format!("g {}\n", name.split('/').last().unwrap())));
                }
                assert_eq!(names.len(), 4);
                assert!(properties["parts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|part| part["whitelisted_cars"] == "-1"));
            }
            let first = fs::read(&output).unwrap();
            fixture.train.preview_consist[0].reversed = true;
            fixture.save();
            export(&mut fixture.container, &output, &option).unwrap();
            assert_eq!(
                first,
                fs::read(&output).unwrap(),
                "Preview changes must not change exported bytes"
            );
        }
        fixture.finish();
    }
    #[test]
    fn texture_bindings_normalize_images_and_apply_tint_once() {
        let mut fixture = Fixture::new();
        let (asset, document) = load_asset(&mut fixture.container, "asset").unwrap();
        let mut layer = fixture.train.carriages[0].body_models[0].clone();
        let original = texture_bytes(
            &mut fixture.container,
            &asset,
            Some(&document.materials[0]),
            &layer,
        )
        .unwrap();
        let image = image::load_from_memory(&original).unwrap().to_rgba8();
        assert!((image.get_pixel(0, 0)[0] as i32 - 100).abs() < 3);
        let replacement = solid_png([1.0, 0.0, 0.0, 1.0]).unwrap();
        let hash = fixture
            .container
            .put_blob(&replacement, "image/png")
            .unwrap();
        layer
            .material_bindings
            .push(crate::domain::MaterialBinding {
                material_id: "material".into(),
                texture_asset_id: Some(hash),
                properties: Default::default(),
            });
        let replaced = texture_bytes(
            &mut fixture.container,
            &asset,
            Some(&document.materials[0]),
            &layer,
        )
        .unwrap();
        assert_eq!(
            image::load_from_memory(&replaced)
                .unwrap()
                .to_rgba8()
                .get_pixel(0, 0)
                .0,
            [128, 0, 0, 255]
        );
        fixture.finish();
    }
    #[test]
    fn validation_reports_model_location_and_blocks_incompatible_data() {
        let mut fixture = Fixture::new();
        fixture.train.carriages[0].body_models[0]
            .material_bindings
            .push(crate::domain::MaterialBinding {
                material_id: "material".into(),
                texture_asset_id: Some("missing".into()),
                properties: Default::default(),
            });
        fixture.save();
        let issues = validate(&mut fixture.container, &options("mtr4", "obj")).unwrap();
        assert!(issues
            .iter()
            .any(|issue| issue.layer_id.as_deref() == Some("body-layer")
                && issue.field.as_deref() == Some("materialBindings")));
        assert!(export(
            &mut fixture.container,
            &fixture.root.join("invalid.zip"),
            &options("mtr4", "obj")
        )
        .is_err());
        fixture.train.carriages[0].body_models[0]
            .material_bindings
            .clear();
        let duplicate = fixture.train.carriages[0].clone();
        fixture.train.carriages.push(duplicate);
        fixture.save();
        assert!(validate(&mut fixture.container, &options("mtr4", "obj"))
            .unwrap()
            .iter()
            .any(|i| i.field.as_deref() == Some("exportId")));
        fixture.train.carriages.pop();
        fixture.train.carriages[0].end_1.gangway = true;
        fixture.save();
        assert!(
            validate(&mut fixture.container, &options("mtr3_nte", "obj"))
                .unwrap()
                .iter()
                .any(|i| i.field.as_deref() == Some("mtr3Compatibility"))
        );
        fixture.finish();
    }
    #[test]
    fn unsafe_zip_paths_never_replace_the_destination() {
        let root = std::env::temp_dir().join(format!("mtr-zip-safety-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("pack.zip");
        fs::write(&output, b"existing").unwrap();
        assert!(write_zip(&output, &BTreeMap::from([("../evil".into(), vec![1])])).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"existing");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn obj_channels_and_mqo_scale_uvs_round_trip() {
        let mut doc = document();
        let mut with_uv = doc.parts[0].clone();
        with_uv.id = "second".into();
        with_uv.name = "second".into();
        with_uv.texcoords = vec![[0.1, 0.2], [0.8, 0.3], [0.4, 0.9]];
        with_uv.normals = vec![[0.0, 0.0, 1.0]; 3];
        doc.parts.push(with_uv);
        let obj = write_obj(&doc, "test.mtl");
        assert!(obj.contains("f 4/1/1 6/3/3 5/2/2"));
        let mqo = write_mqo(&doc);
        assert!(mqo.contains("-100 0 0"));
        assert!(mqo.contains("UV(0.1 0.2 0.8 0.3 0.4 0.9)"));
    }
}
