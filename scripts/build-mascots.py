#!/usr/bin/env python3
"""Convert retained Blender/PMD/GLB sources to animated, self-contained characters.
Run with Blender 4.5: blender -b --python scripts/build-mascots.py -- CHARACTER.
Miku's PMD import requires the MMD Tools add-on.
"""

import bpy
import bmesh
import json
import math
from pathlib import Path
import sys
from mathutils import Matrix, Quaternion, Vector

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from character_motion import leg_angles, smooth, step

KIND = sys.argv[-1]
OUT = ROOT / "assets/characters" / KIND
SOURCE = ROOT / "assets/characters"
# All prepared characters face Blender +Y / glTF -Z.
AXES = {"x": Vector((1, 0, 0)), "y": Vector((0, 1, 0)), "z": Vector((0, 0, 1))}


def material(name, rgb, image=None):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()
    output = nodes.new("ShaderNodeOutputMaterial")
    shader = nodes.new("ShaderNodeBsdfPrincipled")
    shader.inputs["Base Color"].default_value = (*rgb, 1)
    shader.inputs["Roughness"].default_value = 0.9
    mat.node_tree.links.new(shader.outputs[0], output.inputs["Surface"])
    if image:
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = image
        mat.node_tree.links.new(tex.outputs["Color"], shader.inputs["Base Color"])
    mat.diffuse_color = (*rgb, 1)
    return mat


def flatten(objects, matrix):
    # Bake the source's object hierarchy into geometry; armature coordinates use
    # the same transform, so original skin weights remain valid.
    for obj in objects:
        transform = matrix @ obj.matrix_world
        obj.data.transform(transform)
        obj.parent = None
        obj.matrix_world = Matrix.Identity(4)
        obj.animation_data_clear()
        for modifier in list(obj.modifiers):
            obj.modifiers.remove(modifier)


def make_rig(spec, originals=None, transform=None):
    data = bpy.data.armatures.new(KIND + " skeleton")
    arm = bpy.data.objects.new(KIND + " rig", data)
    bpy.context.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    for name, (head, tail, parent) in spec.items():
        bone = data.edit_bones.new(name)
        bone.head = head
        bone.tail = tail
        if originals and name in originals:
            bone.matrix = transform @ originals[name]
            bone.length = max(0.025, (Vector(tail) - Vector(head)).length)
        if parent:
            bone.parent = data.edit_bones[parent]
    bpy.ops.object.mode_set(mode="OBJECT")
    return arm


def original_rig(objects, source_arm, matrix, roles, parent_overrides):
    chosen = {g.name for obj in objects for g in obj.vertex_groups}
    chosen |= set(roles.values())
    bones = source_arm.data.bones
    chosen &= set(bones.keys())
    # Retain deform bones and the ancestors needed by their weights, but remove
    # control rigs and physics constraints from the exported game skeleton.
    for name in list(chosen):
        bone = bones[name].parent
        while bone:
            if bone.use_deform:
                chosen.add(bone.name)
            bone = bone.parent
    spec, matrices = {}, {}
    pending = set(chosen)
    while pending:
        progressed = False
        for name in list(pending):
            bone = bones[name]
            parent = parent_overrides.get(name)
            if name not in parent_overrides:
                p = bone.parent
                while p and p.name not in chosen:
                    p = p.parent
                parent = p.name if p else None
            if parent and parent not in spec:
                continue
            head = matrix @ bone.head_local
            tail = matrix @ bone.tail_local
            spec[name] = (head, tail, parent)
            matrices[name] = bone.matrix_local.copy()
            pending.remove(name)
            progressed = True
        if not progressed:
            raise RuntimeError("Cyclic skeleton parents: " + str(pending))
    flatten(objects, matrix)
    return make_rig(spec, matrices, matrix)


def tube(start, end, radius, mat, name):
    direction = Vector(end) - Vector(start)
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=12,
        radius=radius,
        depth=direction.length,
        location=(Vector(start) + Vector(end)) / 2,
    )
    obj = bpy.context.object
    obj.name = name
    obj.rotation_euler = direction.to_track_quat("Z", "Y").to_euler()
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    obj.data.materials.append(mat)
    for face in obj.data.polygons:
        face.use_smooth = True
    return obj


def spatial_weights(objects, arm, roles, mascot):
    # Small mascots have a broad body and narrow limbs. Object identity keeps
    # eyes/ears rigid; arm and leg regions blend only across their own joints.
    for obj in objects:
        obj.vertex_groups.clear()
        groups = {
            name: obj.vertex_groups.new(name=name) for name in arm.data.bones.keys()
        }
        for v in obj.data.vertices:
            x, y, z = v.co
            label = obj.name.lower()
            if mascot == "gopher" and any(
                s in label for s in ["eye", "ear", "nose", "tooth"]
            ):
                weights = {roles["head"]: 1.0}
            elif mascot == "gopher" and ("hand" in label or "hnad" in label):
                weights = {roles["handR" if x > 0 else "handL"]: 1.0}
            elif mascot == "gopher" and "foot" in label:
                weights = {roles["footR" if x > 0 else "footL"]: 1.0}
            elif "wood hand" in label:
                weights = {roles["handR" if x > 0 else "handL"]: 1.0}
            elif "wood foot" in label:
                weights = {roles["footR" if x > 0 else "footL"]: 1.0}
            elif "wood leg" in label:
                side = "R" if x > 0 else "L"
                weights = {
                    (
                        roles["leg" + side]
                        if "upper" in label
                        else roles["shin" + side]
                    ): 1.0
                }
            elif "arm link" in label:
                side = "R" if x > 0 else "L"
                shoulder = arm.data.bones[roles["upper" + side]].head_local.z
                elbow = arm.data.bones[roles["lower" + side]].head_local.z
                wrist = arm.data.bones[roles["hand" + side]].head_local.z
                if z > elbow:
                    t = smooth((shoulder - z) / max(0.01, shoulder - elbow))
                    weights = {roles["upper" + side]: 1 - t, roles["lower" + side]: t}
                else:
                    t = smooth((elbow - z) / max(0.01, elbow - wrist))
                    weights = {roles["lower" + side]: 1 - t, roles["hand" + side]: t}
            elif z < (0.43 if mascot == "gopher" else 0.59):
                side = "R" if x > 0 else "L"
                knee = arm.data.bones[roles["shin" + side]].head_local.z
                t = smooth((knee + 0.07 - z) / 0.14)
                f = smooth((0.17 - z) / 0.12)
                weights = {
                    roles["leg" + side]: 1 - t,
                    roles["shin" + side]: t * (1 - f),
                    roles["foot" + side]: t * f,
                }
            else:
                t = smooth((z - 1.03) / 0.35)
                weights = {roles["chest"]: 1 - t, roles["head"]: t}
            for name, weight in weights.items():
                if weight > 0:
                    groups[name].add([v.index], weight, "REPLACE")


def animate(arm, roles):
    scene = bpy.context.scene
    scene.render.fps = 30
    arm.animation_data_create()
    rest = {b.name: b.matrix_local.to_quaternion() for b in arm.data.bones}

    def rotate(name, delta):
        bone = arm.pose.bones[name]
        bone.rotation_mode = "QUATERNION"
        bone.rotation_quaternion = rest[name].inverted() @ delta @ rest[name]

    def angle(axis, value):
        return Quaternion(AXES[axis], value)

    clips = []
    for clip, seconds in [
        ("Idle", 2),
        ("Walk", 0.85),
        ("Run", 0.62),
        ("Jump", 0.8),
        ("Dash", 0.4),
        ("Death", 0.85),
    ]:
        action = bpy.data.actions.new(clip)
        arm.animation_data.action = action
        count = round(seconds * 30)
        for frame in range(count + 1):
            t = frame / count
            phase = t * 2 * math.pi
            moving = clip in ["Walk", "Run"]
            running = clip == "Run"
            for bone in arm.pose.bones:
                bone.rotation_mode = "QUATERNION"
                bone.rotation_quaternion = Quaternion()
                bone.location = Vector()
            bob = (
                (-0.03 if running else -0.018)
                + (0.008 if running else 0.004) * math.cos(phase * 2)
                if moving
                else 0.003 * math.sin(phase)
            )
            hip = arm.pose.bones[roles["hips"]]
            hip.location = rest[roles["hips"]].inverted() @ Vector(
                (0.006 * math.sin(phase) if moving else 0, 0, bob)
            )
            rotate(
                roles["chest"],
                angle(
                    "x",
                    -0.08 if running else -0.02 if moving else 0.004 * math.sin(phase),
                ),
            )
            rotate(
                roles["head"],
                angle("z", 0.018 * math.sin(phase))
                @ angle("x", 0.035 if running else 0),
            )
            for side, offset, sign in [("R", 0, 1), ("L", 0.5, -1)]:
                upper, lower, hand = [
                    roles[key + side] for key in ["upper", "lower", "hand"]
                ]
                bone = arm.data.bones[upper]
                direction = (
                    arm.data.bones[lower].head_local - bone.head_local
                ).normalized()
                target = Vector((sign * 0.07, 0.06, -1)).normalized()
                drop = direction.rotation_difference(target)
                rotate(
                    upper,
                    angle(
                        "x",
                        (
                            math.cos(phase + offset * 2 * math.pi)
                            * (0.55 if running else 0.22)
                            if moving
                            else 0.025 * math.sin(phase)
                        ),
                    )
                    @ drop,
                )
                rotate(lower, angle("x", -0.65 if running else -0.16))
                leg, shin, foot = [roles[key + side] for key in ["leg", "shin", "foot"]]
                length1 = (
                    arm.data.bones[shin].head_local - arm.data.bones[leg].head_local
                ).length
                length2 = (
                    arm.data.bones[foot].head_local - arm.data.bones[shin].head_local
                ).length
                forward, lift, flight = step(
                    t + offset, 0.17 if running else 0.11, 0.09 if running else 0.04
                )
                a, b = (
                    leg_angles(
                        forward, length1 + length2 + bob - lift, length1, length2
                    )
                    if moving
                    else (0, 0)
                )
                rotate(leg, angle("x", a))
                rotate(shin, angle("x", b))
                rotate(foot, angle("x", -a - b - flight * 0.1))
                if clip in ["Jump", "Dash"]:
                    rotate(
                        leg,
                        angle(
                            "x",
                            0.4 if clip == "Jump" else 0.6 if side == "R" else -0.25,
                        ),
                    )
                    rotate(shin, angle("x", -0.7))
                    rotate(foot, angle("x", 0.3))
                if clip == "Death":
                    fall = smooth(t)
                    rotate(
                        roles["hips"], angle("x", 1.3 * fall) @ angle("y", 0.2 * fall)
                    )
                    hip.location = rest[roles["hips"]].inverted() @ Vector(
                        (0, -0.2 * fall, -0.55 * fall)
                    )
                    rotate(leg, angle("x", 0.35 * fall))
                    rotate(shin, angle("x", -0.6 * fall))
            for bone in arm.pose.bones:
                if any(s in bone.name for s in ["髪", "TAIL", "SCARF"]):
                    rotate(
                        bone.name,
                        angle(
                            "x", (0.035 if moving else 0.008) * math.sin(phase - 0.5)
                        ),
                    )
                bone.keyframe_insert(data_path="rotation_quaternion", frame=frame)
                if bone.name == roles["hips"]:
                    bone.keyframe_insert(data_path="location", frame=frame)
        track = arm.animation_data.nla_tracks.new()
        track.name = clip
        track.strips.new(clip, 1, action)
        track.mute = True
        clips.append(action)
    arm.animation_data.action = None
    for bone in arm.pose.bones:
        bone.rotation_quaternion = Quaternion()
        bone.location = Vector()
    scene.frame_set(0)


def run():
    if KIND in ["konqi", "gopher"]:
        bpy.ops.wm.open_mainfile(filepath=str(SOURCE / KIND / "source.blend"))
    elif KIND in ["miku", "sahur"]:
        bpy.ops.object.select_all(action="SELECT")
        bpy.ops.object.delete(use_global=False)
        bpy.ops.import_scene.gltf(filepath=str(SOURCE / KIND / "source.glb"))
    else:
        raise ValueError(KIND)
    roles = {
        key: key
        for key in [
            "hips",
            "chest",
            "head",
            "upperR",
            "lowerR",
            "handR",
            "upperL",
            "lowerL",
            "handL",
            "legR",
            "shinR",
            "footR",
            "legL",
            "shinL",
            "footL",
        ]
    }
    if KIND == "konqi":
        objects = [
            o
            for o in bpy.data.objects
            if o.type == "MESH"
            and (
                o.name == "Konqi"
                or o.name.startswith(("Konqi_eye", "Konqi_scarf", "Konqi_teeth"))
            )
        ]
        source_arm = bpy.data.objects["Konqi_rig"]
        roles.update(hips="DEF-HIPS", chest="DEF-SPINE", head="DEF-HEAD")
        for side in ["L", "R"]:
            for key, label in [
                ("upper", "ARM"),
                ("lower", "FOREARM01"),
                ("hand", "HAND"),
                ("leg", "THIGH"),
                ("shin", "SHIN"),
                ("foot", "FOOT"),
            ]:
                roles[key + side] = f"DEF-{label}.{side}"
        overrides = {
            roles["hips"]: None,
            roles["chest"]: roles["hips"],
            "DEF-NECK": roles["chest"],
            roles["head"]: "DEF-NECK",
        }
        for side in ["L", "R"]:
            for key, parent in [
                ("upper", "chest"),
                ("lower", "upper" + side),
                ("hand", "lower" + side),
                ("leg", "hips"),
                ("shin", "leg" + side),
                ("foot", "shin" + side),
            ]:
                overrides[roles[key + side]] = roles[parent]
        for bone in source_arm.data.bones:
            name = bone.name
            if not name.startswith("DEF-"):
                continue
            if name.startswith(("DEF-FINGER", "DEF-INDEX", "DEF-THUMB")):
                prefix, side = name.rsplit(".", 1)
                overrides[name] = (
                    (prefix[:-2] + "01." + side)
                    if prefix.endswith("02")
                    else roles["hand" + side]
                )
            elif name.startswith(("DEF-EYE", "DEF-JAW", "DEF-MOUTH")):
                overrides[name] = roles["head"]
            elif name.startswith("DEF-TAIL"):
                index = int(name[-2:])
                overrides[name] = (
                    roles["hips"]
                    if index == 1
                    else "DEF-TAIL" + str(index - 1).zfill(2)
                )
            elif name.startswith("DEF-SCARF"):
                overrides[name] = (
                    "DEF-NECK" if "01" in name else name.replace("02", "01")
                )
        for image in bpy.data.images:
            candidate = SOURCE / "konqi" / Path(image.filepath).name
            if candidate.is_file():
                image.filepath = str(candidate)
                image.reload()
        # Evaluate subdivision before flattening, with the rig in its rest pose.
        source_arm.data.pose_position = "REST"
        for obj in objects:
            bpy.context.view_layer.objects.active = obj
            for mod in list(obj.modifiers):
                if mod.type not in ["ARMATURE"]:
                    bpy.ops.object.modifier_apply(modifier=mod.name)
        pts = [o.matrix_world @ v.co for o in objects for v in o.data.vertices]
        bottom = min(v.z for v in pts)
        scale = 1.8 / (max(v.z for v in pts) - bottom)
        matrix = Matrix.Diagonal((scale, scale, scale, 1)) @ Matrix.Translation(
            (0, 0, -bottom)
        )
        for obj in objects:
            for i, mat in enumerate(obj.data.materials):
                filename = (
                    "konqi-eye-iris.png"
                    if "eye" in obj.name.lower()
                    else (
                        "konqi_scarf_dif.png"
                        if "scarf" in obj.name.lower()
                        else "Konqi_dif.png"
                    )
                )
                images = [
                    bpy.data.images.load(
                        str(SOURCE / "konqi" / filename), check_existing=True
                    )
                ]
                obj.data.materials[i] = material(
                    (mat.name if mat else "Konqi") + " game",
                    (0.8, 0.8, 0.8),
                    images[0] if images else None,
                )
        arm = original_rig(objects, source_arm, matrix, roles, overrides)
    elif KIND == "miku":
        source_arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
        objects = [
            o
            for o in bpy.data.objects
            if o.type == "MESH" and o.find_armature() == source_arm
        ]
        for obj in objects:
            if obj.data.shape_keys:
                obj.shape_key_clear()
        roles.update(hips="センター", chest="上半身2", head="頭")
        for side, prefix in [("L", "左"), ("R", "右")]:
            for key, label in [
                ("upper", "腕"),
                ("lower", "ひじ"),
                ("hand", "手首"),
                ("leg", "足"),
                ("shin", "ひざ"),
                ("foot", "足首"),
            ]:
                roles[key + side] = prefix + label
        overrides = {roles["hips"]: None, roles["chest"]: roles["hips"]}
        for side in ["L", "R"]:
            for key, parent in [
                ("upper", "chest"),
                ("lower", "upper" + side),
                ("hand", "lower" + side),
                ("leg", "hips"),
                ("shin", "leg" + side),
                ("foot", "shin" + side),
            ]:
                overrides[roles[key + side]] = roles[parent]
        points = [o.matrix_world @ v.co for o in objects for v in o.data.vertices]
        bottom = min(v.z for v in points)
        scale = 1.8 / (max(v.z for v in points) - bottom)
        matrix = Matrix.Diagonal((scale, scale, scale, 1)) @ Matrix.Translation(
            (0, 0, -bottom)
        )
        arm = original_rig(objects, source_arm, matrix, roles, overrides)
    else:
        objects = [o for o in bpy.data.objects if o.type == "MESH"]
        points = [o.matrix_world @ v.co for o in objects for v in o.data.vertices]
        bottom = min(v.z for v in points)
        scale = 1.8 / (max(v.z for v in points) - bottom)
        if KIND == "gopher":
            matrix = (
                Matrix.Diagonal((scale, scale, scale, 1))
                @ Matrix.Rotation(math.pi / 2, 4, "Z")
                @ Matrix.Translation((1.02, 0.53, -bottom))
            )
            for obj in objects:
                for i, mat in enumerate(obj.data.materials):
                    obj.data.materials[i] = material(
                        mat.name + " game", tuple(mat.diffuse_color[:3])
                    )
        else:
            matrix = (
                Matrix.Diagonal((scale, scale, scale, 1))
                @ Matrix.Rotation(math.pi / 2, 4, "Z")
                @ Matrix.Translation((0.108, -0.055, -bottom))
            )
            for obj in objects:
                for i, mat in enumerate(obj.data.materials):
                    tex = next(
                        (
                            n.image
                            for n in mat.node_tree.nodes
                            if n.type == "TEX_IMAGE" and n.image
                        ),
                        None,
                    )
                    obj.data.materials[i] = material(mat.name + " game", (1, 1, 1), tex)
        if KIND == "sahur":
            for obj in objects:
                bm = bmesh.new()
                bm.from_mesh(obj.data)
                remove = []
                for v in bm.verts:
                    x, y, z = obj.matrix_world @ v.co
                    if z < -0.16 or (-0.16 < z < 0.20 and (y < -0.034 or y > 0.142)):
                        remove.append(v)
                bmesh.ops.delete(bm, geom=remove, context="VERTS")
                bm.to_mesh(obj.data)
                bm.free()
        flatten(objects, matrix)
        if KIND == "sahur":
            for obj in objects:
                bpy.context.view_layer.objects.active = obj
                mod = obj.modifiers.new("Game polygon budget", "DECIMATE")
                mod.ratio = 0.38
                bpy.ops.object.modifier_apply(modifier=mod.name)
        g = KIND == "gopher"
        hip = 0.40 if g else 0.58
        chest = 0.81 if g else 1.08
        head = 1.22 if g else 1.42
        spec = {
            "hips": ((0, 0, hip), (0, 0, hip + 0.1), None),
            "chest": ((0, 0, chest), (0, 0, chest + 0.1), "hips"),
            "head": ((0, 0, head), (0, 0, head + 0.1), "chest"),
        }
        for side, sign in [("R", 1), ("L", -1)]:
            shoulder = (
                sign * (0.42 if g else 0.22),
                0.40 if g else -0.04,
                0.96 if g else 1.18,
            )
            elbow = (
                sign * (0.45 if g else 0.28),
                0.55 if g else 0.025,
                0.76 if g else 0.88,
            )
            hand = (
                sign * (0.35 if g else 0.29),
                0.66 if g else 0.08,
                0.91 if g else 0.57,
            )
            leg = (sign * (0.23 if g else 0.12), 0, hip)
            knee = (leg[0], 0, 0.19 if g else 0.31)
            foot = (leg[0], 0.10, 0.07)
            for key, start, end, parent in [
                ("upper", shoulder, elbow, "chest"),
                ("lower", elbow, hand, "upper" + side),
                ("hand", hand, (hand[0], hand[1] + 0.055, hand[2]), "lower" + side),
                ("leg", leg, knee, "hips"),
                ("shin", knee, foot, "leg" + side),
                ("foot", foot, (foot[0], foot[1] + 0.12, foot[2]), "shin" + side),
            ]:
                spec[key + side] = (start, end, parent)
            if g:
                for obj in objects:
                    if "hand" in obj.name.lower() or "hnad" in obj.name.lower():
                        center = sum((v.co for v in obj.data.vertices), Vector()) / len(
                            obj.data.vertices
                        )
                        if center.x * sign > 0:
                            obj.data.transform(
                                Matrix.Translation(Vector(hand) - center)
                            )
                skin = objects[-1].data.materials[0]
                skin = material("Gopher arm skin", (0.761, 0.442, 0.18))
                objects.extend(
                    [
                        tube(shoulder, elbow, 0.038, skin, "arm link " + side),
                        tube(elbow, hand, 0.038, skin, "arm link lower " + side),
                    ]
                )
            else:
                wood = material("Sahur wood", (0.38, 0.115, 0.042))
                objects.extend(
                    [
                        tube(shoulder, elbow, 0.038, wood, "arm link " + side),
                        tube(elbow, hand, 0.033, wood, "arm link lower " + side),
                        tube(leg, knee, 0.045, wood, "wood leg upper " + side),
                        tube(knee, foot, 0.033, wood, "wood leg lower " + side),
                    ]
                )
                for center, size, label in [
                    (hand, (0.050, 0.075, 0.050), "wood hand "),
                    (foot, (0.070, 0.14, 0.055), "wood foot "),
                ]:
                    bpy.ops.mesh.primitive_uv_sphere_add(
                        segments=16, ring_count=8, location=center
                    )
                    obj = bpy.context.object
                    obj.name = label + side
                    obj.scale = size
                    bpy.ops.object.transform_apply(
                        location=True, rotation=True, scale=True
                    )
                    obj.data.materials.append(wood)
                    for face in obj.data.polygons:
                        face.use_smooth = True
                    objects.append(obj)
        if not g:
            core = tube((0, 0, 0.59), (0, 0, 1.35), 0.163, wood, "Wood body core")
            objects.append(core)
        arm = make_rig(spec)
        spatial_weights(objects, arm, roles, KIND)
    for obj in objects:
        mod = obj.modifiers.new("Character skin", "ARMATURE")
        mod.object = arm
        obj.parent = arm
    for obj in objects:
        while len(obj.data.uv_layers) > 1:
            obj.data.uv_layers.remove(obj.data.uv_layers[-1])
        for attr in list(obj.data.color_attributes):
            obj.data.color_attributes.remove(attr)
    animate(arm, roles)
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    for obj in objects:
        obj.select_set(True)
    OUT.mkdir(parents=True, exist_ok=True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(
        filepath=str(OUT / "model.glb"),
        export_format="GLB",
        use_selection=True,
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_force_sampling=True,
        export_nla_strips=True,
        export_anim_slide_to_zero=True,
        export_morph=False,
        export_extras=False,
    )
    import struct

    model = OUT / "model.glb"
    raw = model.read_bytes()
    length = struct.unpack_from("<I", raw, 12)[0]
    doc = json.loads(raw[20 : 20 + length])
    cursor = 20 + length
    size = struct.unpack_from("<I", raw, cursor)[0]
    payload = raw[cursor + 8 : cursor + 8 + size]
    doc.setdefault("extensionsUsed", []).append("KHR_materials_unlit")
    for mat in doc["materials"]:
        mat.setdefault("extensions", {})["KHR_materials_unlit"] = {}
        if KIND == "miku":
            # The VRM conversion marked every surface as blended. Depth-writing
            # cutouts keep the body from drawing over the skirt and sleeves.
            mat["alphaMode"] = "MASK"
            mat["alphaCutoff"] = 0.5
            mat["doubleSided"] = True
    encoded = json.dumps(doc, ensure_ascii=False, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    model.write_bytes(
        struct.pack("<III", 0x46546C67, 2, 28 + len(encoded) + len(payload))
        + struct.pack("<II", len(encoded), 0x4E4F534A)
        + encoded
        + struct.pack("<II", len(payload), 0x004E4942)
        + payload
    )
    arms = {}
    for side, label in [("R", "right"), ("L", "left")]:
        for key, role in [("upper", "upper"), ("lower", "lower"), ("hand", "hand")]:
            arms[label + "_" + key] = roles[role + side]
    arms["height"] = {"konqi": 0.55, "gopher": 0.51, "miku": 0.70, "sahur": 0.65}[KIND]
    arms["forward"] = 0.68 if KIND == "gopher" else 0.32 if KIND == "sahur" else 0.18
    arms["side"] = 0.18 if KIND == "gopher" else 0.27
    arms["two_handed"] = KIND != "gopher"
    if KIND in ["gopher", "sahur"]:
        arms["right_rotation"] = [-math.pi / 2, 0, 0]
        arms["left_rotation"] = [-math.pi / 2, 0, 0]
    catalog_path = ROOT / "assets/characters/catalog.json"
    catalog = json.loads(catalog_path.read_text())
    for entry in catalog["characters"]:
        if entry["id"] == KIND:
            entry["arms"] = arms
    catalog_path.write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + "\n")
    print("EXPORTED", KIND, (OUT / "model.glb").stat().st_size)


if __name__ == "__main__":
    run()
