"""FlyTest Blender bridge.

Usage (headless):
    blender --background --python blender/flytest_bridge.py -- \
        --events runtime-output/events.jsonl \
        --output runtime-output/flytest.blend

The script is intentionally dependency-free beyond Blender's bundled bpy API.
It creates a small arena and fly when the objects do not already exist, then
bakes JSONL runtime events into transform/material keyframes.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any, Iterable

import bpy
from mathutils import Vector


COLLECTION_NAME = "FlyTest"
BODY_NAME = "FlyTest_Body"
LEFT_WING_NAME = "FlyTest_Wing_L"
RIGHT_WING_NAME = "FlyTest_Wing_R"
LEFT_EYE_NAME = "FlyTest_Eye_L"
RIGHT_EYE_NAME = "FlyTest_Eye_R"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Bake FlyTest JSONL events into Blender")
    parser.add_argument("--events", required=True, type=Path, help="JSONL event log")
    parser.add_argument("--output", type=Path, help="output .blend path")
    parser.add_argument("--fps", type=int, default=60)
    parser.add_argument("--reset", action="store_true", help="remove the generated FlyTest collection first")
    argv = sys.argv
    if "--" in argv:
        argv = argv[argv.index("--") + 1 :]
    return parser.parse_args(argv)


def load_events(path: Path) -> list[dict[str, Any]]:
    events: list[dict[str, Any]] = []
    with path.open("r", encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            line = line.strip()
            if not line:
                continue
            try:
                value = json.loads(line)
            except json.JSONDecodeError as exc:
                raise RuntimeError(f"invalid JSON at {path}:{line_number}: {exc}") from exc
            if isinstance(value, dict):
                events.append(value)
    return events


def get_collection() -> bpy.types.Collection:
    collection = bpy.data.collections.get(COLLECTION_NAME)
    if collection is None:
        collection = bpy.data.collections.new(COLLECTION_NAME)
        bpy.context.scene.collection.children.link(collection)
    return collection


def remove_generated_collection() -> None:
    collection = bpy.data.collections.get(COLLECTION_NAME)
    if collection is None:
        return
    for obj in list(collection.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.collections.remove(collection)


def move_to_collection(obj: bpy.types.Object, collection: bpy.types.Collection) -> None:
    for current in list(obj.users_collection):
        current.objects.unlink(obj)
    collection.objects.link(obj)


def ensure_material(name: str, color: tuple[float, float, float, float]) -> bpy.types.Material:
    material = bpy.data.materials.get(name)
    if material is None:
        material = bpy.data.materials.new(name)
    material.diffuse_color = color
    material.use_nodes = True
    principled = material.node_tree.nodes.get("Principled BSDF")
    if principled is not None:
        principled.inputs["Base Color"].default_value = color
        principled.inputs["Roughness"].default_value = 0.35
    return material


def ensure_object(name: str, object_type: str) -> bpy.types.Object:
    obj = bpy.data.objects.get(name)
    if obj is None or obj.type != object_type:
        if obj is not None:
            bpy.data.objects.remove(obj, do_unlink=True)
        obj = bpy.data.objects.new(name, None)
    return obj


def create_uv_sphere(name: str, location: tuple[float, float, float], scale: tuple[float, float, float], material: bpy.types.Material) -> bpy.types.Object:
    bpy.ops.mesh.primitive_uv_sphere_add(segments=24, ring_count=12, location=location)
    obj = bpy.context.object
    obj.name = name
    obj.scale = scale
    obj.data.materials.append(material)
    move_to_collection(obj, get_collection())
    return obj


def create_fly() -> dict[str, bpy.types.Object]:
    collection = get_collection()
    body_material = ensure_material("FlyTest_Body_Material", (0.12, 0.18, 0.28, 1.0))
    wing_material = ensure_material("FlyTest_Wing_Material", (0.65, 0.82, 1.0, 0.7))
    eye_material = ensure_material("FlyTest_Eye_Material", (0.85, 0.05, 0.02, 1.0))
    arena_material = ensure_material("FlyTest_Arena_Material", (0.025, 0.03, 0.06, 1.0))

    body = bpy.data.objects.get(BODY_NAME)
    if body is None:
        body = create_uv_sphere(BODY_NAME, (0.0, 0.0, 1.0), (0.22, 0.12, 0.10), body_material)
    else:
        move_to_collection(body, collection)
    body.parent = None

    left_wing = bpy.data.objects.get(LEFT_WING_NAME)
    if left_wing is None:
        bpy.ops.mesh.primitive_plane_add(size=0.55, location=(-0.28, 0.0, 1.0))
        left_wing = bpy.context.object
        left_wing.name = LEFT_WING_NAME
        left_wing.rotation_euler = (0.0, 0.15, -0.25)
        left_wing.data.materials.append(wing_material)
        move_to_collection(left_wing, collection)
    right_wing = bpy.data.objects.get(RIGHT_WING_NAME)
    if right_wing is None:
        bpy.ops.mesh.primitive_plane_add(size=0.55, location=(0.28, 0.0, 1.0))
        right_wing = bpy.context.object
        right_wing.name = RIGHT_WING_NAME
        right_wing.rotation_euler = (0.0, -0.15, 0.25)
        right_wing.data.materials.append(wing_material)
        move_to_collection(right_wing, collection)

    for name, x in ((LEFT_EYE_NAME, -0.10), (RIGHT_EYE_NAME, 0.10)):
        eye = bpy.data.objects.get(name)
        if eye is None:
            eye = create_uv_sphere(name, (x, -0.10, 1.10), (0.055, 0.035, 0.055), eye_material)
        else:
            move_to_collection(eye, collection)
        eye.parent = body

    if bpy.data.objects.get("FlyTest_Arena") is None:
        bpy.ops.mesh.primitive_plane_add(size=14.0, location=(0.0, 0.0, 0.0))
        arena = bpy.context.object
        arena.name = "FlyTest_Arena"
        arena.data.materials.append(arena_material)
        move_to_collection(arena, collection)
    return {"body": body, "left_wing": left_wing, "right_wing": right_wing}


def ensure_camera_and_light() -> None:
    scene = bpy.context.scene
    camera = bpy.data.objects.get("FlyTest_Camera")
    if camera is None:
        camera_data = bpy.data.cameras.new("FlyTest_Camera")
        camera = bpy.data.objects.new("FlyTest_Camera", camera_data)
        get_collection().objects.link(camera)
    camera.location = (0.0, -7.5, 4.5)
    camera.rotation_euler = (math.radians(63.0), 0.0, 0.0)
    camera.data.lens = 52
    scene.camera = camera

    light = bpy.data.objects.get("FlyTest_KeyLight")
    if light is None:
        light_data = bpy.data.lights.new("FlyTest_KeyLight", type="AREA")
        light_data.energy = 900.0
        light_data.shape = "DISK"
        light_data.size = 5.0
        light = bpy.data.objects.new("FlyTest_KeyLight", light_data)
        get_collection().objects.link(light)
    light.location = (2.5, -3.0, 6.0)
    light.rotation_euler = (math.radians(35.0), 0.0, math.radians(25.0))


def action_value(event: dict[str, Any], name: str, default: float = 0.0) -> float:
    action = event.get("action", {})
    value = action.get(name, default) if isinstance(action, dict) else default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def observation_value(event: dict[str, Any], name: str, default: float = 0.0) -> float:
    observation = event.get("observation", {})
    value = observation.get(name, default) if isinstance(observation, dict) else default
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def world_position(event: dict[str, Any]) -> Vector:
    world = event.get("world", {})
    position = world.get("position", [0.0, 0.0, 1.0]) if isinstance(world, dict) else [0.0, 0.0, 1.0]
    if not isinstance(position, (list, tuple)) or len(position) != 3:
        return Vector((0.0, 0.0, 1.0))
    try:
        return Vector(tuple(float(value) for value in position))
    except (TypeError, ValueError):
        return Vector((0.0, 0.0, 1.0))


def bake_events(events: Iterable[dict[str, Any]], fps: int) -> None:
    scene = bpy.context.scene
    scene.render.fps = fps
    scene.frame_start = 1
    frames = 0
    body_material = bpy.data.materials.get("FlyTest_Body_Material")
    for event in events:
        tick = int(event.get("observation", {}).get("tick", frames)) + 1
        frames += 1
        position = world_position(event)
        turn = action_value(event, "turn")
        throttle = action_value(event, "throttle")
        vertical = action_value(event, "vertical")
        wingbeat = action_value(event, "wingbeat_hz", 100.0)
        light = observation_value(event, "light", 0.5)
        body = bpy.data.objects.get(BODY_NAME)
        left_wing = bpy.data.objects.get(LEFT_WING_NAME)
        right_wing = bpy.data.objects.get(RIGHT_WING_NAME)
        if body is not None:
            body.location = position
            body.rotation_euler = (0.0, vertical * 0.15, turn * 0.35)
            body.keyframe_insert(data_path="location", frame=tick)
            body.keyframe_insert(data_path="rotation_euler", frame=tick)
        if left_wing is not None:
            flap = math.sin(tick * max(0.1, wingbeat) / max(fps, 1) * math.tau)
            left_wing.location = position + Vector((-0.28, 0.0, 0.02))
            left_wing.rotation_euler = (flap * (0.25 + throttle * 0.25), 0.15, -0.25)
            left_wing.keyframe_insert(data_path="location", frame=tick)
            left_wing.keyframe_insert(data_path="rotation_euler", frame=tick)
        if right_wing is not None:
            flap = math.sin(tick * max(0.1, wingbeat) / max(fps, 1) * math.tau)
            right_wing.location = position + Vector((0.28, 0.0, 0.02))
            right_wing.rotation_euler = (-flap * (0.25 + throttle * 0.25), -0.15, 0.25)
            right_wing.keyframe_insert(data_path="location", frame=tick)
            right_wing.keyframe_insert(data_path="rotation_euler", frame=tick)
        if body_material is not None and body_material.use_nodes:
            principled = body_material.node_tree.nodes.get("Principled BSDF")
            if principled is not None:
                emission_name = "Emission Color" if "Emission Color" in principled.inputs else "Emission"
                if emission_name in principled.inputs:
                    principled.inputs[emission_name].default_value = (
                        light * 0.7,
                        light * 0.25,
                        0.05,
                        1.0,
                    )
                if "Emission Strength" in principled.inputs:
                    principled.inputs["Emission Strength"].default_value = light * 2.0
    scene.frame_end = max(scene.frame_start, frames)


def main() -> None:
    args = parse_args()
    events = load_events(args.events)
    if args.reset:
        remove_generated_collection()
    create_fly()
    ensure_camera_and_light()
    bake_events(events, args.fps)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        bpy.ops.wm.save_as_mainfile(filepath=str(args.output))
        print(f"Saved FlyTest scene: {args.output}")
    print(f"Baked {len(events)} FlyTest events")


if __name__ == "__main__":
    main()
