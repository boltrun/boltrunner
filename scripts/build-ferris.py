#!/usr/bin/env python3
"""Export the retained Ferris source with crab locomotion and weapon grips."""
import json
import math
from pathlib import Path
import struct

import bpy
from mathutils import Matrix, Quaternion, Vector

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets/characters/ferris"


def build():
    bpy.ops.wm.open_mainfile(filepath=str(ASSETS / "source.blend"))
    source = bpy.data.objects["Armature"]
    objects = [o for o in bpy.data.objects if o.type == "MESH" and o.find_armature() == source]
    # Bake the authored IK stance before replacing Blender-only control bones.
    bpy.context.view_layer.update()
    posed = {b.name: (source.matrix_world @ b.matrix, source.matrix_world @ b.head,
                      source.matrix_world @ b.tail) for b in source.pose.bones}
    for obj in objects:
        bpy.context.view_layer.objects.active = obj
        for modifier in list(obj.modifiers):
            if modifier.type == "SUBSURF":
                modifier.levels = 2
            bpy.ops.object.modifier_apply(modifier=modifier.name)
    points = [o.matrix_world @ v.co for o in objects for v in o.data.vertices]
    bottom = min(v.z for v in points)
    scale = 1.8 / (max(v.z for v in points) - bottom)
    transform = (Matrix.Diagonal((scale, scale, scale, 1))
                 @ Matrix.Rotation(math.pi, 4, "Z")
                 @ Matrix.Translation((0, 0, -bottom)))
    names = ["Head"]
    for side in ["R", "L"]:
        names.extend(f"{part}.{side}" for part in ["Arm", "Finger", "Thumb"])
        names.extend(f"{part}{i}.{side}" for i in range(3) for part in ["UpperLeg", "LowerLeg"])
    data = bpy.data.armatures.new("Ferris skeleton")
    arm = bpy.data.objects.new("Ferris", data)
    bpy.context.collection.objects.link(arm)
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    for name in names:
        matrix, head, tail = posed[name]
        bone = data.edit_bones.new(name)
        bone.matrix = transform @ matrix
        bone.length = (transform @ tail - transform @ head).length
        parent = source.data.bones[name].parent
        if name.startswith("Thumb"):
            parent = source.data.bones["Finger." + name[-1]]
        if parent:
            bone.parent = data.edit_bones[parent.name]
    for side in ["R", "L"]:
        finger = data.edit_bones[f"Finger.{side}"]
        grip = data.edit_bones.new(f"Grip.{side}")
        grip.head = finger.head.lerp(finger.tail, 0.55)
        grip.tail = finger.head.lerp(finger.tail, 0.85)
        grip.parent = finger
    bpy.ops.object.mode_set(mode="OBJECT")
    image = bpy.data.images["albedo.png"]
    image.scale(1024, 1024)
    image.pack()
    mat = bpy.data.materials.new("Ferris colors")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    nodes.clear()
    texture = nodes.new("ShaderNodeTexImage")
    texture.image = image
    shader = nodes.new("ShaderNodeEmission")
    output = nodes.new("ShaderNodeOutputMaterial")
    mat.node_tree.links.new(texture.outputs["Color"], shader.inputs["Color"])
    mat.node_tree.links.new(shader.outputs[0], output.inputs["Surface"])
    for obj in objects:
        matrix = transform @ obj.matrix_world
        obj.parent = None
        obj.data.transform(matrix)
        obj.matrix_world = Matrix.Identity(4)
        obj.animation_data_clear()
        obj.data.materials.clear()
        obj.data.materials.append(mat)
        for side in ["R", "L"]:
            group = obj.vertex_groups.get(f"Finger.{side}")
            if not group:
                continue
            grip = obj.vertex_groups.new(name=f"Grip.{side}")
            bone = data.bones[f"Finger.{side}"]
            direction = bone.tail_local - bone.head_local
            for vertex in obj.data.vertices:
                weight = next((g.weight for g in vertex.groups if g.group == group.index), 0)
                t = (vertex.co - bone.head_local).dot(direction) / direction.length_squared
                blend = min(1, max(0, (t - 0.35) / 0.3))
                if weight and blend:
                    group.add([vertex.index], weight * (1 - blend), "REPLACE")
                    grip.add([vertex.index], weight * blend, "REPLACE")
        modifier = obj.modifiers.new("Ferris skin", "ARMATURE")
        modifier.object = arm
        obj.parent = arm
    bpy.data.objects.remove(source, do_unlink=True)
    scene = bpy.context.scene
    scene.render.fps = 30
    arm.animation_data_create()
    rest = {b.name: b.matrix_local.to_quaternion() for b in data.bones}

    def rotate(name, axis, angle):
        q = rest[name]
        arm.pose.bones[name].rotation_quaternion = q.inverted() @ Quaternion(axis, angle) @ q

    for clip, duration in [("Idle", 2.4), ("Walk", 0.8), ("Run", 0.5),
                           ("Jump", 0.7), ("Dash", 0.45), ("Death", 1.0)]:
        action = bpy.data.actions.new(clip)
        arm.animation_data.action = action
        count = round(duration * 30)
        for frame in range(count + 1):
            t = frame / count
            phase = t * math.tau
            moving = clip in ["Walk", "Run"]
            fall = t * t * (3 - 2 * t) if clip == "Death" else 0
            for bone in arm.pose.bones:
                bone.rotation_mode = "QUATERNION"
                bone.rotation_quaternion = Quaternion()
                bone.location = Vector()
            bob = (0.025 if moving else 0.008) * (1 - math.cos(phase * 2))
            rotate("Head", (0, 1, 0), 1.5 * fall)
            if clip == "Dash":
                rotate("Head", (1, 0, 0), -0.18)
            height = bob + 0.55 * fall + (0.15 if clip == "Dash" else 0)
            if clip == "Death":
                height += 0.25 * math.sin(math.pi * t)
            arm.pose.bones["Head"].location = rest["Head"].inverted() @ Vector((0, 0, height))
            for side, sign in [("R", 1), ("L", -1)]:
                for i in range(3):
                    p = phase + i * math.tau / 3 + (math.pi if side == "L" else 0)
                    swing = (0.20 if clip == "Run" else 0.12) * math.sin(p) if moving else 0
                    lift = max(0, math.cos(p)) * 0.20 if moving else 0
                    if clip in ["Jump", "Dash"]:
                        lift = 0.22
                    rotate(f"UpperLeg{i}.{side}", (0, 0, 1), swing)
                    # Fold each of the six legs independently, preserving the crab stance.
                    q = Quaternion((0, 1, 0), -sign * (lift + 0.6 * fall))
                    bone = arm.pose.bones[f"UpperLeg{i}.{side}"]
                    bone.rotation_quaternion = bone.rotation_quaternion @ (rest[bone.name].inverted() @ q @ rest[bone.name])
                    rotate(f"LowerLeg{i}.{side}", (0, 1, 0), sign * (lift * 0.6 + fall * 0.4))
                rotate(f"Arm.{side}", (0, 0, 1), sign * (0.035 * math.sin(phase) + 0.3 * fall))
            for bone in arm.pose.bones:
                bone.keyframe_insert(data_path="rotation_quaternion", frame=frame)
                if bone.name == "Head":
                    bone.keyframe_insert(data_path="location", frame=frame)
        track = arm.animation_data.nla_tracks.new()
        track.name = clip
        track.strips.new(clip, 1, action)
        track.mute = True
    arm.animation_data.action = None
    for bone in arm.pose.bones:
        bone.rotation_quaternion = Quaternion()
        bone.location = Vector()
    scene.frame_set(0)
    bpy.ops.object.select_all(action="DESELECT")
    for obj in [arm, *objects]:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.export_scene.gltf(filepath=str(ASSETS / "model.glb"), export_format="GLB",
        use_selection=True, export_animations=True, export_animation_mode="NLA_TRACKS",
        export_force_sampling=True, export_nla_strips=True, export_anim_slide_to_zero=True,
        export_morph=False, export_extras=False)
    # Use the same unlit material path as the other cartoon characters.
    path = ASSETS / "model.glb"
    raw = path.read_bytes()
    size = struct.unpack_from("<I", raw, 12)[0]
    document = json.loads(raw[20:20 + size])
    payload = raw[20 + size:]
    document.setdefault("extensionsUsed", []).append("KHR_materials_unlit")
    for material in document["materials"]:
        texture = material.pop("emissiveTexture")
        material.pop("emissiveFactor", None)
        material["pbrMetallicRoughness"] = {"baseColorTexture": texture,
            "baseColorFactor": [1, 1, 1, 1], "metallicFactor": 0, "roughnessFactor": 1}
        material.setdefault("extensions", {})["KHR_materials_unlit"] = {}
    encoded = json.dumps(document, ensure_ascii=False, separators=(",", ":")).encode()
    encoded += b" " * (-len(encoded) % 4)
    path.write_bytes(struct.pack("<III", 0x46546C67, 2, 20 + len(encoded) + len(payload))
        + struct.pack("<II", len(encoded), 0x4E4F534A) + encoded + payload)


if __name__ == "__main__":
    build()
