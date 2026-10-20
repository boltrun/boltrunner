"""Import original detailed city prefabs at authored, measured city transforms."""
from pathlib import Path
import json,math,hashlib,shutil
import numpy as np
from PIL import Image
from dae_model import load
HERE=Path(__file__).resolve().parent

def rotation(yaw):
    c,s=math.cos(math.radians(yaw)),math.sin(math.radians(yaw))
    return np.array([[c,0,s],[0,1,0],[-s,0,c]])

def prepare(world,rotate,sun,textures,first_material,source_geometry=None,source_public_centers=()):
    catalog=json.loads((HERE/'hd/assets.json').read_text())
    placements=json.loads((HERE/'placements.json').read_text())['buildings']
    models={};material_ids={};materials=[];chunks=[];volumes=[];source_volumes=[];windmills=[];collision=[];foundation_edges=[]
    def model(key):
        if key not in models:
            asset=catalog[key];path=HERE/'hd'/asset.get('mesh',asset.get('dae'))
            loader=load
            if path.suffix=='.obj':
                from obj_model import load as loader
            models[key]=[p for p in loader(path) if 'shadow' not in str(p['name']).lower()]
            for p in models[key]:p['positions']*=asset.get('unit_scale',1.)
        return models[key]
    def material(key,part):
        filename=part['material']['texture']
        if filename is None:raise ValueError('Every detailed building surface must have a diffuse texture')
        source=HERE/'hd'/catalog[key]['textures'][filename]
        digest=hashlib.sha256(source.read_bytes()).hexdigest()
        if digest not in material_ids:
            index=first_material+len(materials);material_ids[digest]=index
            name='hd-'+digest[:12]+'.png';shutil.copyfile(source,textures/name)
            image=Image.open(source)
            cutout='A' in image.getbands() and image.getchannel('A').getextrema()[0]<128
            materials.append(dict(id=index,name='mondstadt/hd/'+filename,texture=name,glow=None,width=image.width,height=image.height,emissive=False,alpha=cutout,blend=False))
        return material_ids[digest]
    for placement in placements:
        key=placement['asset'];parts=model(key);rotor_info=catalog[key].get('rotor');parts=[p for p in parts if not rotor_info or p['name']!=rotor_info['material']];R=rotation(placement['yaw']);scale=placement['scale'];position=np.array(placement['position'])
        raw=np.concatenate([p['positions'].reshape(-1,3) for p in parts]);lo=raw.min(0);hi=raw.max(0)
        volumes.append((key,R,scale,position,lo,hi))
        if source_geometry is not None and placement.get('replace_faces') and key!='props_barbatos':
            original=source_geometry[placement['replace_faces']]
            source_volumes.append((key,original.min((0,1)),original.max((0,1))))
        if placement.get('replacement_bounds_raw'):
            source_volumes.append((key,*map(np.array,placement['replacement_bounds_raw'])))
        grouped={}
        for part in parts:
            mat=material(key,part);p=part['positions']@R.T*scale+position;n=part['normals']@R.T
            color=.195+.035*np.maximum(n@sun,0)
            data=np.concatenate((world(p),n@rotate,part['uv']*[1,-1]+[0,1],np.zeros((*p.shape[:2],2)),np.repeat(color[...,None],3,axis=2),np.ones((*p.shape[:2],1))),axis=2)
            grouped.setdefault(mat,[]).append(data.reshape(-1,14))
            area=np.linalg.norm(np.cross(p[:,1]-p[:,0],p[:,2]-p[:,0]),axis=1)/2
            # Keep physics affordable: large supporting roof/wall triangles and
            # the actual low porch/stair treads, excluding sculpted ornaments.
            up=n.mean(1)[:,1]
            local_height=part['positions'].mean(1)[:,1]
            low_treads=(up>.7)&(local_height<4)&(area>.012)
            roofs=(up>.65)&(area>.45)
            walls=(abs(up)<.2)&(area>.7)
            if key in ('cathedral','headquarters'):
                collision.extend(p[((up>.45)&(area>.45))|((key=='cathedral')&(abs(up)<.2)&(area>.7))])
            else:collision.extend(p[low_treads|roofs|walls])
            # Only the lowest large structural facade edges can need a plinth;
            # upper windows, wooden braces and sculpted details are excluded.
            surface=str(part['material']['texture']).lower()
            part_bottom=float(part['positions'][:,:,1].min())
            structural=any(token in surface for token in ('wall','bricks','base'))
            if structural and part_bottom<lo[1]+.9:
                lower_walls=(abs(up)<.6)&(area>.01)
                for triangle,local,normal in zip(p[lower_walls],part['positions'][lower_walls],n.mean(1)[lower_walls]):
                    for j in range(3):
                        a,b=triangle[j],triangle[(j+1)%3]
                        if (max(local[j,1],local[(j+1)%3,1])<part_bottom+.80
                                and abs(a[1]-b[1])<.60
                                and np.linalg.norm((b-a)[[0,2]])>.10):
                            foundation_edges.append((a,b,normal))
        chunks.extend((mat,np.concatenate(data)) for mat,data in grouped.items())
        if rotor_info:
            hub=np.array(rotor_info['pivot'])@R.T*scale+position
            axis=np.array(rotor_info['axis'])@R.T@rotate
            windmills.append(dict(hub=world(hub).tolist(),axis=axis.tolist(),scale=scale,angular_speed=.32))
    rotor_key='services_windmill';info=catalog[rotor_key]['rotor']
    part=next(p for p in model(rotor_key) if p['name']==info['material'])
    rotor_mat=material(rotor_key,part)
    axis=np.array(info['axis']);basis=rotation(math.degrees(math.atan2(axis[0],axis[2])))
    positions=(part['positions']-info['pivot'])@basis
    normals=part['normals']@basis
    rotor=dict(positions=positions.reshape(-1,3).round(6).tolist(),normals=normals.reshape(-1,3).round(6).tolist(),uvs=(part['uv']*[1,-1]+[0,1]).reshape(-1,2).round(6).tolist(),texture=next(m['texture'] for m in materials if m['id']==rotor_mat))
    # Neighbouring source houses are complete assemblies, not fragments of the
    # replacement plot. Match geometry before any material UV remapping, so the
    # same source faces survive in render and collision despite atlas changes.
    retained={tuple(np.round(center,4)) for center in source_public_centers}
    if source_geometry is not None:
        restoration=json.loads((HERE/'restoration.json').read_text())
        for house in restoration['retained_neighbour_houses']['assemblies']:
            for face in house['source_faces']:
                retained.add(tuple(np.round(source_geometry[face].mean(0),4)))
    def replaces(data,physics=False):
        p=data[:,:3]@rotate;p[:,1]-=53.1
        center=p.mean(0)
        if tuple(np.round(center,4)) in retained:return False
        tex=data[:,6:8].mean(0)
        if tex[0]<.5 and .125<tex[1]<.625:
            return False
        # Detached HLOD dormers/chimneys are separate components, and can sit
        # outside the replacement prefab's bounds. Remove architecture in the
        # original property's footprint as well; public paving stays above.
        for source_key,lo,hi in source_volumes:
            if physics and source_key=='headquarters':continue
            if (np.all(center[[0,2]]>=lo[[0,2]]-1.5)
                    and np.all(center[[0,2]]<=hi[[0,2]]+1.5)
                    and lo[1]+.35<center[1]<hi[1]+15):
                return True
        for key,R,scale,position,lo,hi in volumes:
            if physics and key=='props_barbatos':continue
            if physics and key=='headquarters' and data[:,4].mean()<=.45:continue
            q=(center-position)@R/scale
            if np.all(q>=lo-[1.3,-.8,1.3]) and np.all(q<=hi+[1.3,2.,1.3]):
                # Ground/retaining walls remain intact. Only architecture above
                # each building foundation is replaced, never public streets.
                if q[1]>lo[1]+.9:return True
        return False
    return chunks,materials,windmills,rotor,replaces,collision,foundation_edges
