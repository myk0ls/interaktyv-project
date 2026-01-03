import bpy
import json
from mathutils import Vector

# SETTINGS
CURVE_NAME = "BezierCurve"
OUTPUT_PATH = "//zuma_path.json"

curve_obj = bpy.data.objects[CURVE_NAME]

depsgraph = bpy.context.evaluated_depsgraph_get()
eval_obj = curve_obj.evaluated_get(depsgraph)

# Convert evaluated curve to mesh
mesh = eval_obj.to_mesh()

points = []

# Extract vertices in order
for v in mesh.vertices:
    world_pos = curve_obj.matrix_world @ v.co
    points.append([world_pos.x, world_pos.z, -world_pos.y])

# Cleanup
eval_obj.to_mesh_clear()

#Remove the last point if the same
if points[0] == points[-1]:
    points.pop(-1)
    
#reverse if needed
points.reverse()

data = {
    "name": CURVE_NAME,
    "points": points
}

with open(bpy.path.abspath(OUTPUT_PATH), "w") as f:
    json.dump(data, f, indent=2)

print(f"Exported {len(points)} points to {OUTPUT_PATH}")
