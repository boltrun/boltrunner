"""Read checked-in, static COLLADA building assets without a runtime importer."""
from pathlib import Path
from urllib.parse import unquote
import xml.etree.ElementTree as ET
import numpy as np

NS={'c':'http://www.collada.org/2005/11/COLLADASchema'}
def floats(element):return np.fromstring(element.text or '',sep=' ') if element is not None else np.empty(0)

def load(path):
    path=Path(path);root=ET.parse(path).getroot()
    images={i.attrib['id']:unquote(i.findtext('c:init_from',namespaces=NS)).replace('\\','/').split('/')[-1] for i in root.findall('c:library_images/c:image',NS)}
    effects={}
    for effect in root.findall('c:library_effects/c:effect',NS):
        params={p.attrib['sid']:p for p in effect.findall('c:profile_COMMON/c:newparam',NS)}
        texture=effect.find('.//c:diffuse/c:texture',NS)
        color=effect.find('.//c:diffuse/c:color',NS)
        file=None
        if texture is not None:
            sampler=params.get(texture.attrib['texture'])
            surface_name=sampler.findtext('c:sampler2D/c:source',namespaces=NS) if sampler is not None else texture.attrib['texture']
            surface=params.get(surface_name)
            image=surface.findtext('c:surface/c:init_from',namespaces=NS) if surface is not None else surface_name
            file=images.get(image,image if image and image.lower().endswith('.png') else None)
        effects[effect.attrib['id']]={'texture':file,'color':floats(color).tolist() if color is not None else [1,1,1,1]}
    materials={m.attrib['id']:dict(effects[m.find('c:instance_effect',NS).attrib['url'].lstrip('#')],name=m.attrib.get('name',m.attrib['id'])) for m in root.findall('c:library_materials/c:material',NS)}
    geometries={}
    for geometry in root.findall('c:library_geometries/c:geometry',NS):
        mesh=geometry.find('c:mesh',NS)
        if mesh is None:continue
        sources={}
        for source in mesh.findall('c:source',NS):
            accessor=source.find('c:technique_common/c:accessor',NS);stride=int(accessor.attrib.get('stride','1'));offset=int(accessor.attrib.get('offset','0'));count=int(accessor.attrib['count']);data=floats(source.find('c:float_array',NS));sources[source.attrib['id']]=data[offset:offset+stride*count].reshape(count,stride)
        vertices={v.attrib['id']:{i.attrib['semantic']:i.attrib['source'].lstrip('#') for i in v.findall('c:input',NS)} for v in mesh.findall('c:vertices',NS)}
        parts=[]
        for primitive in list(mesh):
            kind=primitive.tag.split('}')[-1]
            if kind not in ('triangles','polylist','polygons'):continue
            bindings={i.attrib['semantic']:(int(i.attrib.get('offset','0')),i.attrib['source'].lstrip('#')) for i in primitive.findall('c:input',NS) if i.attrib['semantic']!='TEXCOORD' or i.attrib.get('set','0')=='0'}
            width=max(int(i.attrib.get('offset','0')) for i in primitive.findall('c:input',NS))+1
            polygons=[]
            if kind=='triangles':polygons=np.fromstring(primitive.findtext('c:p',namespaces=NS),sep=' ',dtype=np.int64).reshape(-1,3,width)
            elif kind=='polylist':
                data=np.fromstring(primitive.findtext('c:p',namespaces=NS),sep=' ',dtype=np.int64).reshape(-1,width);offset=0
                for count in np.fromstring(primitive.findtext('c:vcount',namespaces=NS),sep=' ',dtype=np.int64):polygons.append(data[offset:offset+count]);offset+=count
            else:polygons=[np.fromstring(p.text or '',sep=' ',dtype=np.int64).reshape(-1,width) for p in primitive.findall('c:p',NS)]
            triangles=[]
            for polygon in polygons:
                for k in range(1,len(polygon)-1):triangles.append(polygon[[0,k,k+1]])
            if not triangles:continue
            indices=np.array(triangles)
            def attribute(semantic):
                if semantic in bindings:
                    offset,source=bindings[semantic];return sources[source][indices[:,:,offset]]
                if 'VERTEX' in bindings:
                    offset,source=bindings['VERTEX'];target=vertices[source].get(semantic)
                    if target:return sources[target][indices[:,:,offset]]
                return None
            positions=attribute('POSITION');normals=attribute('NORMAL');uv=attribute('TEXCOORD')
            if positions is None:raise ValueError(f'Missing mesh positions in {geometry.attrib["id"]}')
            if normals is None:
                normal=np.cross(positions[:,1]-positions[:,0],positions[:,2]-positions[:,0]);normal/=np.maximum(np.linalg.norm(normal,axis=1)[:,None],1e-8);normals=np.repeat(normal[:,None,:],3,axis=1)
            if uv is None:uv=np.zeros((*positions.shape[:2],2))
            parts.append({'name':geometry.attrib.get('name',geometry.attrib['id']),'symbol':primitive.attrib.get('material'),'positions':positions[:,:,:3],'normals':normals[:,:,:3],'uv':uv[:,:,:2]})
        geometries[geometry.attrib['id']]=parts
    unit=root.find('c:asset/c:unit',NS);scale=float(unit.attrib.get('meter','1')) if unit is not None else 1.
    up=root.findtext('c:asset/c:up_axis','Y_UP',NS);rotation=np.eye(4)
    if up=='Z_UP':rotation[:3,:3]=np.array([[1,0,0],[0,0,1],[0,-1,0]])
    elif up=='X_UP':rotation[:3,:3]=np.array([[0,-1,0],[1,0,0],[0,0,1]])
    rotation[:3,:3]*=scale
    output=[]
    def node(element,parent):
        transform=np.eye(4)
        for value in list(element):
            kind=value.tag.split('}')[-1];values=floats(value);m=np.eye(4)
            if kind=='matrix':m=values.reshape(4,4).T
            elif kind=='translate':m[:3,3]=values
            elif kind=='scale':m[:3,:3]=np.diag(values)
            elif kind=='rotate':
                axis=values[:3];axis/=np.linalg.norm(axis);x,y,z=axis;angle=np.deg2rad(values[3]);c=np.cos(angle);s=np.sin(angle);C=1-c;m[:3,:3]=[[c+x*x*C,x*y*C-z*s,x*z*C+y*s],[y*x*C+z*s,c+y*y*C,y*z*C-x*s],[z*x*C-y*s,z*y*C+x*s,c+z*z*C]]
            else:continue
            transform=transform@m
        world=parent@transform
        for instance in element.findall('c:instance_geometry',NS):
            binds={b.attrib['symbol']:b.attrib['target'].lstrip('#') for b in instance.findall('c:bind_material/c:technique_common/c:instance_material',NS)}
            for source in geometries[instance.attrib['url'].lstrip('#')]:
                part=dict(source);pos=source['positions'];part['positions']=pos@world[:3,:3].T+world[:3,3]
                norm=source['normals']@np.linalg.inv(world[:3,:3]);norm/=np.maximum(np.linalg.norm(norm,axis=2)[...,None],1e-8);part['normals']=norm
                if np.linalg.det(world[:3,:3])<0:
                    for attr in ('positions','normals','uv'):part[attr]=part[attr][:,[0,2,1]]
                part['node']=element.attrib.get('name',element.attrib.get('id',''))
                part['material']=materials.get(binds.get(source['symbol'],source['symbol']),{'name':source['symbol'],'texture':None,'color':[1,1,1,1]})
                output.append(part)
        for child in element.findall('c:node',NS):node(child,world)
    scene_id=root.find('c:scene/c:instance_visual_scene',NS).attrib['url'].lstrip('#')
    scene=next(s for s in root.findall('c:library_visual_scenes/c:visual_scene',NS) if s.attrib['id']==scene_id)
    for element in scene.findall('c:node',NS):node(element,rotation)
    if not output:raise ValueError(f'No static scene meshes in {path}')
    return output
