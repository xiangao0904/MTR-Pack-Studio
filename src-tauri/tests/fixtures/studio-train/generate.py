"""Regenerate the authored static import fixtures and browser GLBs.

Run from any directory with Python 3 and `pip install blake3`.
All geometry and textures are original procedural test artwork, CC0.
"""
from pathlib import Path
import json
import struct
import zlib
from blake3 import blake3

HERE = Path(__file__).resolve().parent
PUBLIC = HERE.parents[3] / "public" / "fixtures"
PUBLIC.mkdir(parents=True, exist_ok=True)


def png(name, pixel):
    def chunk(kind, payload):
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))
    raw = b"".join(b"\0" + bytes(c for x in range(64) for c in pixel(x, y)) for y in range(64))
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 64, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    (HERE / name).write_bytes(data)
    (PUBLIC / name).write_bytes(data)
    return data


paint = png("paint.png", lambda x, y: (33, 142, 184, 255) if 41 <= y < 49 else (190, 204, 214, 255) if y >= 57 else (226, 232, 236, 255))
checker = png("checker.png", lambda x, y: (20, 44, 64, 255) if (x // 8 + y // 8) % 2 else (29, 59, 80, 255))


def box(name, center, size, material):
    x, y, z = center
    w, h, d = [v / 2 for v in size]
    faces = [
        ((0, 0, 1), [(-w, -h, d), (w, -h, d), (w, h, d), (-w, h, d)]),
        ((0, 0, -1), [(w, -h, -d), (-w, -h, -d), (-w, h, -d), (w, h, -d)]),
        ((1, 0, 0), [(w, -h, d), (w, -h, -d), (w, h, -d), (w, h, d)]),
        ((-1, 0, 0), [(-w, -h, -d), (-w, -h, d), (-w, h, d), (-w, h, -d)]),
        ((0, 1, 0), [(-w, h, d), (w, h, d), (w, h, -d), (-w, h, -d)]),
        ((0, -1, 0), [(-w, -h, -d), (w, -h, -d), (w, -h, d), (-w, -h, d)]),
    ]
    part = dict(id=f"part-{name.lower()}", name=name, positions=[], normals=[], texcoords=[], indices=[], material=material)
    for normal, corners in faces:
        offset = len(part["positions"])
        part["positions"].extend([[a + x, b + y, c + z] for a, b, c in corners])
        part["normals"].extend([list(normal)] * 4)
        part["texcoords"].extend([[0, 1], [1, 1], [1, 0], [0, 0]])
        part["indices"].extend(offset + i for i in (0, 1, 2, 0, 2, 3))
    return part


body = [box("Body_Shell", (0, 2.15, 0), (2.96, 2.9, 19.96), 0)]
for side, x in (("Left", 1.49), ("Right", -1.49)):
    for index, z in enumerate((-8, -4, 0, 4, 8), 1):
        body.append(box(f"Window_{side}_{index}", (x, 2.75, z), (.02, .85, 1.65), 1))
    for index, z in enumerate((-6, -2, 2, 6), 1):
        body.append(box(f"Door_{side}_{index}", (x, 1.88, z), (.02, 2.2, 1.12), 1))
        body.append(box(f"Door_Seam_{side}_{index}", (1.494 if x > 0 else -1.494, 1.88, z), (.012, 2.2, .04), 0))
body += [
    box("Front_Windscreen", (0, 2.78, 9.99), (2.35, 1.05, .02), 1),
    box("Rear_Windscreen", (0, 2.78, -9.99), (1.75, .9, .02), 1),
    box("Roof_Equipment", (0, 3.58, -1), (1.9, .2, 11), 1),
    box("Headlight_Left", (.95, 1.25, 9.99), (.28, .19, .02), 0),
    box("Headlight_Right", (-.95, 1.25, 9.99), (.28, .19, .02), 0),
]
bogie = [box("Bogie_Frame", (0, .62, 0), (2.1, .42, 2.9), 0)]
for end, z in (("Front", 1), ("Rear", -1)):
    bogie.append(box(f"Axle_{end}", (0, .34, z), (2.45, .22, .22), 0))
    for side, x in (("Left", 1.13), ("Right", -1.13)):
        bogie.append(box(f"Wheel_{end}_{side}", (x, .36, z), (.32, .65, .65), 1))


def glb(document):
    binary = bytearray()
    views, accessors, meshes, nodes, images, textures = [], [], [], [], [], []
    def view(data, target=None):
        binary.extend(b"\0" * (-len(binary) % 4))
        result = len(views)
        entry = dict(buffer=0, byteOffset=len(binary), byteLength=len(data))
        if target: entry["target"] = target
        views.append(entry); binary.extend(data)
        return result
    def accessor(values, components, kind, integer=False):
        flat = values if components == 1 else [v for row in values for v in row]
        vi = view(struct.pack("<" + ("I" if integer else "f") * len(flat), *flat), 34963 if integer else 34962)
        entry = dict(bufferView=vi, componentType=5125 if integer else 5126, count=len(values), type=kind)
        if components == 3:
            entry["min"] = [min(v[i] for v in values) for i in range(3)]
            entry["max"] = [max(v[i] for v in values) for i in range(3)]
        result = len(accessors); accessors.append(entry); return result
    for part in document["parts"]:
        attributes = {"POSITION": accessor(part["positions"], 3, "VEC3"), "NORMAL": accessor(part["normals"], 3, "VEC3"), "TEXCOORD_0": accessor(part["texcoords"], 2, "VEC2")}
        primitive = dict(attributes=attributes, indices=accessor(part["indices"], 1, "SCALAR", True), material=part["material"])
        mesh = len(meshes); meshes.append(dict(name=part["name"], primitives=[primitive]))
        nodes.append(dict(name=part["name"], mesh=mesh, extras=dict(partId=part["id"])))
    materials = []
    for material in document["materials"]:
        definition = dict(name=material["name"], pbrMetallicRoughness=dict(baseColorFactor=material["color"], metallicFactor=0, roughnessFactor=.8), doubleSided=True, extras=dict(materialId=material["id"]))
        if material.get("texture"):
            texture = len(textures)
            images.append(dict(bufferView=view((HERE / material["texture"]).read_bytes()), mimeType="image/png"))
            textures.append(dict(source=texture, sampler=0))
            definition["pbrMetallicRoughness"]["baseColorTexture"] = dict(index=texture)
        materials.append(definition)
    scene = dict(asset=dict(version="2.0", generator="MTR Pack Studio authored acceptance fixture"), scene=0, scenes=[dict(nodes=list(range(len(nodes))))], nodes=nodes, meshes=meshes, materials=materials, images=images, textures=textures, samplers=[dict(magFilter=9728, minFilter=9728, wrapS=10497, wrapT=10497)], buffers=[dict(byteLength=len(binary))], bufferViews=views, accessors=accessors)
    js = json.dumps(scene, separators=(",", ":")).encode(); js += b" " * (-len(js) % 4)
    binary.extend(b"\0" * (-len(binary) % 4))
    return struct.pack("<III", 0x46546c67, 2, 28 + len(js) + len(binary)) + struct.pack("<II", len(js), 0x4e4f534a) + js + struct.pack("<II", len(binary), 0x004e4942) + binary


def write_model(stem, browser_name, parts, materials):
    lines = ["# Original MTR Pack Studio fixture; metres; +X left, +Y up, +Z forward", f"mtllib {stem}.mtl"]
    offset = 1
    for part in parts:
        lines += [f"o {part['name']}", f"usemtl {materials[part['material']]['name']}"]
        lines += ["v " + " ".join(f"{v:.6f}" for v in row) for row in part["positions"]]
        lines += ["vt " + " ".join(f"{v:.6f}" for v in row) for row in part["texcoords"]]
        lines += ["vn " + " ".join(f"{v:.6f}" for v in row) for row in part["normals"]]
        for at in range(0, len(part["indices"]), 3):
            lines.append("f " + " ".join(f"{i + offset}/{i + offset}/{i + offset}" for i in part["indices"][at:at+3]))
        offset += len(part["positions"])
    source = ("\n".join(lines) + "\n").encode(); (HERE / f"{stem}.obj").write_bytes(source)
    mtl = ""
    for material in materials:
        mtl += f"newmtl {material['name']}\nKd {' '.join(str(x) for x in material['color'][:3])}\nd 1\n"
        if material.get("texture"): mtl += f"map_Kd {material['texture']}\n"
        mtl += "\n"
    (HERE / f"{stem}.mtl").write_text(mtl, encoding="utf-8")
    document = dict(parts=parts, materials=materials, warnings=[])
    normalized = json.dumps(document, separators=(",", ":")).encode()
    (PUBLIC / f"{browser_name}.document.json").write_bytes(normalized)
    preview = glb(document); (PUBLIC / f"{browser_name}.glb").write_bytes(preview)
    dependencies = [dict(name=f"{stem}.mtl", hash=blake3(mtl.encode()).hexdigest(), mediaType="text/plain")]
    for name in sorted({m["texture"] for m in materials if m.get("texture")}):
        dependencies.append(dict(name=name, hash=blake3((HERE / name).read_bytes()).hexdigest(), mediaType="image/png"))
    metadata = dict(id=f"fixture-{browser_name}", name=f"{stem}.obj", sourceFormat="obj", sourceHash=blake3(source).hexdigest(), documentHash=blake3(normalized).hexdigest(), previewHash=blake3(preview).hexdigest(), materials=materials, dependencies=dependencies, parts=[dict(id=p["id"], name=p["name"], triangleCount=len(p["indices"]) // 3) for p in parts], warnings=[])
    (PUBLIC / f"{browser_name}.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


write_model("body", "studio-train", body, [dict(id="material-0", name="Paint", color=[1, 1, 1, 1], texture="paint.png"), dict(id="material-1", name="Glass", color=[1, 1, 1, 1], texture="checker.png")])
write_model("bogie", "studio-bogie", bogie, [dict(id="material-0", name="Frame", color=[.24, .28, .33, 1], texture=None), dict(id="material-1", name="Wheels", color=[.1, .12, .14, 1], texture=None)])
print(f"Generated {len(body)} body parts and {len(bogie)} bogie parts in {HERE} and {PUBLIC}")
