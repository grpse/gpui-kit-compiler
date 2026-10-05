"""FlowCut's isolated Blender compositor worker. All user data travels as JSON, never code.
Run with: Blender --background --factory-startup --python blender_bridge.py -- catalog/render request.json
"""
import bpy
import json
import sys
from pathlib import Path


def plain(value):
    if isinstance(value, (bool, int, float, str)) or value is None:
        return value
    try:
        return list(value)
    except TypeError:
        return None


def category(identifier):
    name = identifier.removeprefix('CompositorNode').removeprefix('ShaderNode')
    if identifier.startswith('ShaderNodeTex'): return 'Texture'
    if name in ('VectorMath', 'VectorRotate', 'VectorCurve', 'Mix'): return 'Vector'
    if name in ('Clamp', 'FloatCurve', 'CombineRGB', 'SeparateRGB'): return 'Utilities'
    if identifier in ('NodeFrame', 'NodeReroute', 'CompositorNodeGroup'): return 'Layout'
    if name in ('Composite', 'Viewer', 'OutputFile', 'SplitViewer'): return 'Output'
    if name in ('Image', 'RLayers', 'MovieClip', 'Texture', 'RGB', 'Value', 'Time', 'BokehImage', 'Mask'): return 'Input'
    if name in ('BoxMask', 'EllipseMask', 'DoubleEdgeMask', 'IDMask', 'Cryptomatte', 'CryptomatteV2'): return 'Mask'
    if name != 'PremulKey' and any(part in name for part in ('Key', 'Matte', 'Spill')): return 'Keying'
    if any(part in name for part in ('Track', 'Stabilize')): return 'Tracking'
    if name in ('Rotate', 'Scale', 'Transform', 'Translate', 'CornerPin', 'Crop', 'Displace', 'Flip', 'MapUV', 'Lensdist', 'MovieDistortion'): return 'Transform'
    if name in ('Normal', 'VectorCurve', 'VecCurve', 'CurveVec', 'CombineXYZ', 'SeparateXYZ', 'VecBlur'): return 'Vector'
    if name in ('ImageInfo', 'ImageCoordinates', 'SceneTime', 'RelativeToPixel', 'Math', 'MapRange', 'MapValue', 'Levels', 'Normalize', 'Switch', 'SwitchView', 'Split', 'CombineColor', 'SeparateColor', 'CombRGBA', 'SepRGBA', 'CombHSVA', 'SepHSVA', 'CombYUVA', 'SepYUVA', 'CombYCCA', 'SepYCCA'): return 'Utilities'
    if any(part.lower() in name.lower() for part in ('Blur', 'Defocus', 'Filter', 'Glare', 'Denoise', 'Despeckle', 'Dilate', 'Inpaint', 'Kuwahara', 'Pixelate', 'Posterize', 'SunBeams', 'AntiAliasing')): return 'Filter'
    return 'Color'


# These RNA fields survive for old .blend versions; Blender 4.5 moved their controls into sockets.
PROPERTIES = {
    'CompositorNodeBlur': {'filter_type'},
    'CompositorNodeGlare': {'glare_type', 'quality'},
    'CompositorNodeBilateralblur': set(),
    'CompositorNodeBokehBlur': set(),
    'CompositorNodeDBlur': set(),
    'CompositorNodeVecBlur': set(),
}
SHARED_NODES = ['ShaderNodeBlackbody', 'ShaderNodeClamp', 'ShaderNodeCombineRGB', 'ShaderNodeCombineXYZ', 'ShaderNodeFloatCurve', 'ShaderNodeMapRange', 'ShaderNodeMath', 'ShaderNodeMix', 'ShaderNodeMixRGB', 'ShaderNodeRGBCurve', 'ShaderNodeSeparateRGB', 'ShaderNodeSeparateXYZ', 'ShaderNodeTexBrick', 'ShaderNodeTexChecker', 'ShaderNodeTexGabor', 'ShaderNodeTexGradient', 'ShaderNodeTexMagic', 'ShaderNodeTexNoise', 'ShaderNodeTexVoronoi', 'ShaderNodeTexWave', 'ShaderNodeTexWhiteNoise', 'ShaderNodeValToRGB', 'ShaderNodeValue', 'ShaderNodeVectorCurve', 'ShaderNodeVectorMath', 'ShaderNodeVectorRotate']
SKIP = set(bpy.types.Node.bl_rna.properties.keys()) | {'rna_type', 'type', 'inputs', 'outputs', 'internal_links', 'interface', 'node_tree', 'frame_duration', 'frame_start', 'frame_offset'}


def socket_info(socket, index):
    return {'name': socket.name, 'kind': socket.type, 'index': index, 'enabled': socket.enabled and not socket.hide, 'default': plain(getattr(socket, 'default_value', None))}


def nested_parameters(owner, prefix):
    result = []
    if not hasattr(owner, 'bl_rna'): return result
    for prop in owner.bl_rna.properties:
        if prop.is_readonly or prop.identifier == 'rna_type' or prop.type not in ('FLOAT', 'INT', 'BOOLEAN', 'ENUM', 'STRING'): continue
        value = plain(getattr(owner, prop.identifier))
        if value is None or (prop.type == 'ENUM' and prop.is_enum_flag): continue
        kind = 'COLOR' if getattr(prop, 'is_array', False) and getattr(prop, 'subtype', '') in ('COLOR', 'COLOR_GAMMA') else 'VECTOR' if getattr(prop, 'is_array', False) else prop.type
        options = [{'value': e.identifier, 'label': e.name} for e in prop.enum_items] if prop.type == 'ENUM' else []
        if kind == 'ENUM' and not any(e['value'] == value for e in options): kind = 'STRING'
        result.append({'key': prefix + '.' + prop.identifier, 'label': prop.name, 'kind': kind, 'value': value, 'options': options, 'min': max(-1e9, getattr(prop, 'hard_min', -1e9)), 'max': min(1e9, getattr(prop, 'hard_max', 1e9))})
    return result


def describe(node):
    parameters = [{'key': 'label', 'label': 'Label', 'kind': 'STRING', 'value': node.label, 'options': [], 'min': 0, 'max': 0}]
    for prop in node.bl_rna.properties:
        if node.bl_idname in PROPERTIES and prop.identifier not in PROPERTIES[node.bl_idname]: continue
        if prop.identifier in SKIP or prop.is_readonly or (node.bl_idname == 'CompositorNodeOutputFile' and prop.identifier in ('base_path', 'active_input_index')): continue
        if prop.type not in ('FLOAT', 'INT', 'BOOLEAN', 'ENUM', 'STRING', 'POINTER'): continue
        if prop.type == 'POINTER':
            if prop.identifier not in ('image', 'clip', 'mask', 'scene', 'texture'): continue
            parameters.append({'key': prop.identifier, 'label': prop.name + ' file', 'kind': 'FILE', 'value': '', 'options': [], 'min': 0, 'max': 0})
            continue
        value = plain(getattr(node, prop.identifier))
        if prop.type == 'ENUM' and prop.is_enum_flag: continue
        if prop.type in ('FLOAT', 'INT') and not getattr(prop, 'is_array', False): value = max(prop.hard_min, min(prop.hard_max, value))
        parameters.append({'key': prop.identifier, 'label': prop.name, 'kind': 'COLOR' if getattr(prop, 'is_array', False) and getattr(prop, 'subtype', '') in ('COLOR', 'COLOR_GAMMA') else 'VECTOR' if getattr(prop, 'is_array', False) else 'STRING' if prop.type == 'ENUM' and not any(e.identifier == value for e in prop.enum_items) else prop.type, 'value': value, 'options': [{'value': e.identifier, 'label': e.name} for e in prop.enum_items] if prop.type == 'ENUM' else [], 'min': max(-1e9, getattr(prop, 'hard_min', -1e9)), 'max': min(1e9, getattr(prop, 'hard_max', 1e9))})
    for i, socket in enumerate(node.inputs):
        if hasattr(socket, 'default_value'):
            value = plain(socket.default_value)
            if value is None: continue
            parameters.append({'key': f'input:{i}', 'label': socket.name, 'kind': 'COLOR' if socket.type == 'RGBA' else 'VECTOR' if isinstance(value, list) else 'BOOLEAN' if isinstance(value, bool) else 'INT' if isinstance(value, int) else 'FLOAT', 'value': value, 'options': [], 'min': -1e9, 'max': 1e9})
    if node.bl_idname in ('CompositorNodeRGB', 'CompositorNodeValue', 'ShaderNodeValue'):
        value = plain(node.outputs[0].default_value)
        parameters.append({'key': 'output:0', 'label': node.outputs[0].name, 'kind': 'COLOR' if node.outputs[0].type == 'RGBA' else 'VECTOR' if isinstance(value, list) else 'BOOLEAN' if isinstance(value, bool) else 'FLOAT', 'value': value, 'options': [], 'min': -1e9, 'max': 1e9})
    if hasattr(node, 'color_ramp'):
        for key in ('interpolation', 'color_mode', 'hue_interpolation'):
            prop = node.color_ramp.bl_rna.properties[key]
            parameters.append({'key': 'color_ramp.' + key, 'label': prop.name, 'kind': 'ENUM', 'value': getattr(node.color_ramp, key), 'options': [{'value': e.identifier, 'label': e.name} for e in prop.enum_items], 'min': 0, 'max': 0})
        parameters.append({'key': 'color_ramp', 'label': 'Color ramp stops', 'kind': 'JSON', 'value': [{'position': e.position, 'color': list(e.color)} for e in node.color_ramp.elements], 'options': [], 'min': 0, 'max': 0})
    if hasattr(node, 'mapping'): parameters.extend(nested_parameters(node.mapping, 'mapping'))
    if hasattr(node, 'mapping') and hasattr(node.mapping, 'curves'):
        parameters.append({'key': 'curves', 'label': 'Curve points', 'kind': 'JSON', 'value': [[[p.location.x, p.location.y] for p in c.points] for c in node.mapping.curves], 'options': [], 'min': 0, 'max': 0})
    if node.bl_idname == 'CompositorNodeGroup':
        parameters.append({'key': 'group_file', 'label': 'Node group (.blend)', 'kind': 'FILE', 'value': '', 'options': [], 'min': 0, 'max': 0})
    if node.bl_idname == 'CompositorNodeOutputFile':
        parameters.append({'key': 'file_slots', 'label': 'Output slots (comma-separated)', 'kind': 'STRING', 'value': ', '.join(s.path for s in node.file_slots), 'options': [], 'min': 0, 'max': 0})
        for key in ('file_format', 'color_mode', 'color_depth', 'compression', 'quality', 'exr_codec'):
            prop = node.format.bl_rna.properties[key]
            parameters.append({'key': 'format.' + key, 'label': prop.name, 'kind': prop.type, 'value': plain(getattr(node.format, key)), 'options': [{'value': e.identifier, 'label': e.name} for e in prop.enum_items] if prop.type == 'ENUM' else [], 'min': getattr(prop, 'hard_min', -1e9), 'max': getattr(prop, 'hard_max', 1e9)})
    return {'id': node.bl_idname, 'label': node.bl_rna.name.removesuffix(' Node'), 'description': node.bl_rna.description, 'category': category(node.bl_idname), 'inputs': [socket_info(s, i) for i, s in enumerate(node.inputs)], 'outputs': [socket_info(s, i) for i, s in enumerate(node.outputs)], 'parameters': parameters}


def export_catalog(path):
    scene = bpy.context.scene
    scene.use_nodes = True
    tree = scene.node_tree
    tree.nodes.clear()
    catalog = []
    for identifier in sorted(dir(bpy.types)):
        if not identifier.startswith('CompositorNode') or identifier in ('CompositorNode', 'CompositorNodeCustomGroup'): continue
        try:
            node = tree.nodes.new(identifier)
            catalog.append(describe(node))
            tree.nodes.remove(node)
        except RuntimeError:
            pass
    for identifier in SHARED_NODES + ['NodeFrame', 'NodeReroute']:
        node = tree.nodes.new(identifier)
        if identifier == 'ShaderNodeMix': node.data_type = 'VECTOR'
        catalog.append(describe(node))
        tree.nodes.remove(node)
    labels = {}
    for definition in catalog: labels.setdefault(definition['label'], []).append(definition)
    for definitions in labels.values():
        if len(definitions) > 1:
            for definition in definitions:
                if definition['id'].startswith('CompositorNode'): definition['label'] += ' (Legacy)'
    for definition in catalog:
        if definition['id'] == 'ShaderNodeMix': definition['label'] = 'Mix Vector / Value / Color'
        if definition['id'] == 'CompositorNodeMixRGB': definition['label'] = 'Mix Color'
        if definition['id'] == 'ShaderNodeMixRGB': definition['label'] = 'Mix RGB (Legacy)'
    catalog.sort(key=lambda definition: definition['label'].lower())
    Path(path).write_text(json.dumps({'version': bpy.app.version_string, 'nodes': catalog}, indent=2))
    print(f'FlowCut catalog: {len(catalog)} node types')


def load_asset(node, key, value):
    if key == 'group_file':
        with bpy.data.libraries.load(value, link=False) as (source, target): target.node_groups = list(source.node_groups)
        groups = [g for g in target.node_groups if g and g.bl_idname == 'CompositorNodeTree']
        if not groups: raise ValueError('The .blend file contains no compositor node group.')
        node.node_tree = groups[0]
    elif key == 'image': node.image = bpy.data.images.load(value, check_existing=True)
    elif key == 'clip' and Path(value).suffix.lower() != '.blend': node.clip = bpy.data.movieclips.load(value, check_existing=True)
    else:
        collection = {'scene': 'scenes', 'mask': 'masks', 'texture': 'textures', 'clip': 'movieclips'}[key]
        with bpy.data.libraries.load(value, link=False) as (source, target):
            names = getattr(source, collection)
            if not names: raise ValueError(f'The .blend file contains no {collection}.')
            setattr(target, collection, names[:1])
        setattr(node, key, getattr(target, collection)[0])


def apply_settings(node, settings):
    settings = dict(settings)
    # Data blocks and socket layouts must exist before input defaults are assigned.
    for key in ('group_file', 'file_slots', 'image', 'clip', 'scene', 'mask', 'texture'):
        if key in settings:
            value = settings.pop(key)
            if key == 'file_slots':
                names = [name.strip() for name in value.replace(',', '\n').splitlines() if name.strip()]
                if not names or len(names) > 32: raise ValueError('File Output needs 1–32 named slots.')
                node.file_slots.clear()
                for name in names:
                    if Path(name).name != name or name in ('.', '..'): raise ValueError('Output slots need plain names, without folder paths.')
                    node.file_slots.new(name)
            elif value: load_asset(node, key, value)
    for key, value in settings.items():
        if key.startswith('color_ramp.'):
            setattr(node.color_ramp, key.split('.')[1], value)
        elif key.startswith('mapping.'):
            setattr(node.mapping, key.split('.')[1], value)
        elif key.startswith('format.'):
            setattr(node.format, key.split('.')[1], value)
        elif key == 'group_file' and not value:
            continue
        elif key.startswith('output:'):
            node.outputs[int(key.split(':')[1])].default_value = value
        elif key.startswith('input:'):
            index = int(key.split(':')[1])
            node.inputs[index].default_value = value
        elif key == 'color_ramp':
            ramp = node.color_ramp
            while len(ramp.elements) > 2: ramp.elements.remove(ramp.elements[-1])
            for i, stop in enumerate(value):
                element = ramp.elements[i] if i < 2 else ramp.elements.new(stop['position'])
                element.position = stop['position']; element.color = stop['color']
        elif key == 'curves':
            for curve, points in zip(node.mapping.curves, value):
                while len(curve.points) > 2: curve.points.remove(curve.points[-1])
                for i, xy in enumerate(points):
                    point = curve.points[i] if i < 2 else curve.points.new(*xy)
                    point.location = xy
            node.mapping.update()
        else:
            prop = node.bl_rna.properties.get(key)
            if prop and prop.type == 'ENUM' and value == '': continue
            setattr(node, key, value)
    if hasattr(node, 'mapping') and hasattr(node.mapping, 'curves'): node.mapping.update()


def render(request):
    scene = bpy.context.scene
    scene.use_nodes = True
    tree = scene.node_tree; tree.nodes.clear()
    scene.render.engine = 'CYCLES'; scene.cycles.device = 'CPU'; scene.cycles.samples = 1
    scene.render.resolution_x = request['width']; scene.render.resolution_y = request['height']; scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'; scene.render.image_settings.color_mode = 'RGBA'; scene.render.image_settings.color_depth = '8'
    scene.view_settings.view_transform = 'Standard'; scene.view_settings.look = 'None'
    scene.render.filepath = request['output']
    scene.frame_set(request.get('frame', 1))
    nodes = {}
    for entry in request['nodes']:
        node = tree.nodes.new(entry['type'])
        node.name = str(entry['id']); node.mute = entry.get('muted', False)
        apply_settings(node, entry.get('settings', {}))
        # File output always remains within the current render directory.
        if node.bl_idname == 'CompositorNodeOutputFile': node.base_path = str(Path(request['output']).parent / 'files')
        nodes[entry['id']] = node
    for link in request['links']:
        tree.links.new(nodes[link['source']].outputs[link['output']], nodes[link['target']].inputs[link['input']])
    if not any(n.bl_idname == 'CompositorNodeRLayers' for n in nodes.values()):
        for obj in list(scene.objects):
            if obj.type != 'CAMERA': bpy.data.objects.remove(obj, do_unlink=True)
    active = nodes[request['viewer']]
    for node in list(tree.nodes):
        if node.bl_idname == 'CompositorNodeComposite' and node != active: tree.nodes.remove(node)
    if active.bl_idname == 'CompositorNodeComposite': tree.nodes.active = active
    else:
        output = tree.nodes.new('CompositorNodeComposite')
        socket = active.outputs[request.get('viewer_output', 0)] if active.outputs else next(socket for socket in active.inputs if socket.is_linked).links[0].from_socket
        tree.links.new(socket, output.inputs[0])
        tree.nodes.active = output
    bpy.ops.render.render(write_still=True)
    print('FlowCut render complete')


def main():
    args = sys.argv[sys.argv.index('--') + 1:]
    if args[0] == 'catalog': export_catalog(args[1])
    elif args[0] == 'render': render(json.loads(Path(args[1]).read_text()))
    elif args[0] == 'fixture':
        request = json.loads(Path(args[1]).read_text())
        group = bpy.data.node_groups.new('FlowCut exposure fixture', 'CompositorNodeTree')
        group.use_fake_user = True
        group.interface.new_socket(name='Image', in_out='INPUT', socket_type='NodeSocketColor')
        group.interface.new_socket(name='Image', in_out='OUTPUT', socket_type='NodeSocketColor')
        source = group.nodes.new('NodeGroupInput'); effect = group.nodes.new('CompositorNodeExposure'); output = group.nodes.new('NodeGroupOutput')
        effect.inputs[1].default_value = -1.0
        group.links.new(source.outputs[0], effect.inputs[0]); group.links.new(effect.outputs[0], output.inputs[0])
        bpy.ops.wm.save_as_mainfile(filepath=request['output'])
    elif args[0] == 'validate':
        bpy.context.scene.use_nodes = True
        tree = bpy.context.scene.node_tree
        catalog = json.loads(Path(args[1]).read_text())
        errors = []
        for definition in catalog['nodes']:
            node = tree.nodes.new(definition['id'])
            try: apply_settings(node, {p['key']: float(p['value']) if p['kind'] == 'FLOAT' else p['value'] for p in definition['parameters']})
            except Exception as error: errors.append(definition['id'] + ': ' + str(error))
            tree.nodes.remove(node)
        if errors: raise ValueError('\n'.join(errors))
        print(f'FlowCut validated {len(catalog["nodes"])} node defaults')
    elif args[0] == 'describe':
        request = json.loads(Path(args[1]).read_text())
        bpy.context.scene.use_nodes = True
        node = bpy.context.scene.node_tree.nodes.new(request['type'])
        apply_settings(node, request['settings'])
        Path(request['output']).write_text(json.dumps(describe(node)))
    else: raise ValueError('Unknown compositor worker request')

if __name__ == '__main__':
    try: main()
    except Exception as error:
        print('FLOWCUT_ERROR: ' + str(error)); raise
