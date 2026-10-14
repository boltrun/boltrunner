"""Read a static OBJ prefab and its diffuse MTL bindings for the map bake."""
from pathlib import Path
import shlex
import numpy as np

def load(path):
    path=Path(path);positions=[];normals=[];uvs=[];parts={};material=None;materials={}
    def index(value,count):
        value=int(value);return value-1 if value>0 else count+value
    for line in path.read_text().splitlines():
        tokens=shlex.split(line,comments=True)
        if not tokens:continue
        kind=tokens[0]
        if kind=='v':positions.append(list(map(float,tokens[1:4])))
        elif kind=='vn':normals.append(list(map(float,tokens[1:4])))
        elif kind=='vt':uvs.append(list(map(float,tokens[1:3])))
        elif kind=='mtllib':
            for filename in tokens[1:]:
                current=None
                for row in (path.parent/filename).read_text().splitlines():
                    values=shlex.split(row,comments=True)
                    if not values:continue
                    if values[0]=='newmtl':
                        current=' '.join(values[1:]);materials[current]=dict(name=current,texture=None,color=[1.,1.,1.,1.])
                    elif current and values[0]=='map_Kd':materials[current]['texture']=Path(values[-1].replace('\\','/')).name
                    elif current and values[0]=='Kd':materials[current]['color']=[*map(float,values[1:4]),1.]
        elif kind=='usemtl':material=' '.join(tokens[1:])
        elif kind=='f':
            polygon=[]
            for value in tokens[1:]:
                corner=value.split('/');polygon.append((index(corner[0],len(positions)),index(corner[1],len(uvs)) if len(corner)>1 and corner[1] else None,index(corner[2],len(normals)) if len(corner)>2 and corner[2] else None))
            for k in range(1,len(polygon)-1):parts.setdefault(material,[]).append([polygon[0],polygon[k],polygon[k+1]])
    p=np.asarray(positions);n=np.asarray(normals);uv=np.asarray(uvs);output=[]
    for key,faces in parts.items():
        points=np.array([[p[c[0]] for c in f] for f in faces]);tex=np.array([[uv[c[1]] if c[1] is not None else [0.,0.] for c in f] for f in faces]);normal=np.cross(points[:,1]-points[:,0],points[:,2]-points[:,0]);normal/=np.maximum(np.linalg.norm(normal,axis=1)[:,None],1e-8)
        normal=np.array([[n[c[2]] if c[2] is not None else normal[i] for c in f] for i,f in enumerate(faces)])
        output.append(dict(name=key,node=path.stem,symbol=key,positions=points,normals=normal,uv=tex,material=materials.get(key,dict(name=key,texture=None,color=[1.,1.,1.,1.]))))
    if not output:raise ValueError(f'No triangles in {path}')
    return output
