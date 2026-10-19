"""Turn the imported Unicorn Way OBJ into an Elden Ring asset FLVER, ready for Soulstruct's "Export Asset".

Run in Blender's Scripting tab after:
  1. Importing the template asset with Soulstruct (default: aeg030_518, a Caria Manor stone road).
  2. Importing reference/w101/converted/WC_Z06_Unicorn_Way.obj.

The script empties the template mesh (keeping its FLVER settings, material, UV and color layer names), gives every
Unicorn Way mesh matching UV and white vertex color layers, joins them all into the template and puts every face on
the template's first material. Select the template mesh afterwards and click "Export Asset" in the FLVER tab.
"""

import bmesh
import bpy

TEMPLATE = "aeg030_518"


def find_template():
    obj = bpy.context.active_object
    if obj and obj.type == "MESH" and obj.name.lower().startswith(TEMPLATE):
        return obj
    for obj in bpy.data.objects:
        if obj.type == "MESH" and obj.name.lower().startswith(TEMPLATE):
            return obj
    raise RuntimeError(f"No mesh named '{TEMPLATE}...'. Import the template asset with Soulstruct first.")


def is_template_part(obj, template):
    while obj:
        if obj == template or obj == template.parent or obj.name.lower().startswith("aeg"):
            return True
        obj = obj.parent
    return False


def prepare_source(obj, uv_names, colors):
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name=uv_names[0])
    first = me.uv_layers[0]
    for layer in list(me.uv_layers)[1:]:
        me.uv_layers.remove(layer)
    first.name = uv_names[0]
    me.uv_layers.active = first
    for name in uv_names[1:]:
        me.uv_layers.new(name=name, do_init=True)  # copy of the first UV layer

    for attr in list(me.color_attributes):
        me.color_attributes.remove(attr)
    for name, data_type, domain in colors:
        attr = me.color_attributes.new(name=name, type=data_type, domain=domain)
        attr.data.foreach_set("color", [1.0] * 4 * len(attr.data))


def main():
    template = find_template()
    me = template.data
    if not me.materials:
        raise RuntimeError(f"'{template.name}' has no material to use.")

    uv_names = [uv.name for uv in me.uv_layers] or ["UVTexture0"]
    colors = [(a.name, a.data_type, a.domain) for a in me.color_attributes]
    if not any(name == "VertexColors0" for name, _, _ in colors):
        colors.insert(0, ("VertexColors0", "BYTE_COLOR", "CORNER"))
    print(f"Template '{template.name}': UV layers {uv_names}, color layers {[c[0] for c in colors]}")

    sources = [o for o in bpy.context.scene.objects if o.type == "MESH" and not is_template_part(o, template)]
    if not sources:
        raise RuntimeError("No Unicorn Way meshes found. Import the OBJ first.")
    print(f"Joining {len(sources)} meshes into '{template.name}'")

    # Empty the template mesh but keep its layers, materials and FLVER properties.
    bm = bmesh.new()
    bm.from_mesh(me)
    bmesh.ops.delete(bm, geom=list(bm.verts), context="VERTS")
    bm.to_mesh(me)
    bm.free()

    for obj in sources:
        prepare_source(obj, uv_names, colors)

    if bpy.context.mode != "OBJECT":
        bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.select_all(action="DESELECT")
    for obj in sources:
        obj.select_set(True)
    template.hide_set(False)
    template.select_set(True)
    bpy.context.view_layer.objects.active = template
    bpy.ops.object.join()

    me = template.data
    me.polygons.foreach_set("material_index", [0] * len(me.polygons))
    while len(me.materials) > 1:
        me.materials.pop(index=len(me.materials) - 1)
    while len(template.FLVER.submesh_props) > 1:
        template.FLVER.submesh_props.remove(len(template.FLVER.submesh_props) - 1)

    # Static assets weight every vertex to one bone.
    if template.vertex_groups:
        template.vertex_groups[0].add(range(len(me.vertices)), 1.0, "REPLACE")

    print(f"Done: {len(me.vertices)} vertices, {len(me.polygons)} faces on material '{me.materials[0].name}'.")


main()
