"""Offline irradiance atlases for coordinate-authored solid blocks."""
import math
from pathlib import Path
import numpy as np
from PIL import Image

class VoxelBake:
    TILE=4
    STRIDE=6
    SIZE=2048
    def __init__(self,blocks,output):
        self.output=Path(output)
        opaque=[p for p,m in blocks.items() if not any(x in m for x in ('glass','water[','lantern[','chain['))]
        coordinates=np.asarray(opaque,dtype=np.int32)
        self.minimum=coordinates.min(axis=0)-1
        self.grid=np.zeros(tuple(coordinates.max(axis=0)-self.minimum+2),dtype=bool)
        q=coordinates-self.minimum
        self.grid[tuple(q.T)]=True
        self.lamps=np.asarray([p for p,m in blocks.items() if 'lantern[' in m],dtype=np.float32)
        self.atlases=[];self.count=0
    def visible(self,origins,delta):
        n=len(origins)
        delta=np.broadcast_to(delta,origins.shape)
        lower=self.minimum-.5;upper=self.minimum+np.asarray(self.grid.shape)-.5
        with np.errstate(divide='ignore',invalid='ignore'):
            lo=(lower-origins)/delta;hi=(upper-origins)/delta
            near=np.where(abs(delta)>1e-9,np.minimum(lo,hi),-np.inf).max(axis=1)
            far=np.where(abs(delta)>1e-9,np.maximum(lo,hi),np.inf).min(axis=1)
        parallel_outside=((abs(delta)<=1e-9)&((origins<lower)|(origins>=upper))).any(axis=1)
        intersects=(far>=np.maximum(near,0))&(near<1)&~parallel_outside
        entry=np.where((near>0)&intersects,near+1e-6,0).clip(0,1)
        origins=origins+delta*entry[:,None];delta=delta*(1-entry[:,None])
        cell=np.floor(origins+.5).astype(np.int32)
        step=np.sign(delta).astype(np.int32)
        with np.errstate(divide='ignore',invalid='ignore'):
            interval=np.where(abs(delta)>1e-9,1/abs(delta),np.inf)
            next_t=np.where(abs(delta)>1e-9,(cell+step*.5-origins)/delta,np.inf)
        active=intersects.copy();clear=np.ones(n,dtype=bool)
        limit=np.asarray(self.grid.shape)
        for _ in range(240):
            ids=np.flatnonzero(active)
            if not len(ids):break
            cells=cell[ids]-self.minimum
            inside=((cells>=0)&(cells<limit)).all(axis=1)
            active[ids[~inside]]=False
            ids=ids[inside];cells=cells[inside]
            blocked=self.grid[tuple(cells.T)]
            clear[ids[blocked]]=False;active[ids[blocked]]=False
            ids=ids[~blocked]
            if not len(ids):continue
            earliest=next_t[ids].min(axis=1)
            finished=earliest>=1
            active[ids[finished]]=False;ids=ids[~finished];earliest=earliest[~finished]
            axes=next_t[ids]<=earliest[:,None]+1e-7
            cell[ids]+=step[ids]*axes
            # Avoid inf * False producing NaN on axis-aligned rays.
            next_t[ids]=np.where(axes,next_t[ids]+interval[ids],next_t[ids])
        if active.any():raise RuntimeError('Offline ray did not finish')
        return clear
    def illuminate(self,points,normals):
        origins=points+normals*.015
        # Short hemisphere probes give corners/contact seams ambient occlusion.
        ao=np.zeros(len(points),dtype=np.float32)
        directions=np.asarray([(1,1,0),(-1,1,0),(0,1,1),(0,1,-1),(1,.3,1),(-1,.3,-1)],dtype=np.float32)
        for direction in directions:
            direction=direction/np.linalg.norm(direction)
            direction=np.broadcast_to(direction,points.shape).copy()
            back=(direction*normals).sum(axis=1)<0
            direction[back]*=-1
            ao+=self.visible(origins,direction*2.1)
        ambient=(.23+.27*ao/len(directions))*(.85+.15*np.maximum(normals[:,1],0))
        light=np.repeat(ambient[:,None],3,axis=1)
        sun=np.asarray([-.40,.83,.39],dtype=np.float32);sun/=np.linalg.norm(sun)
        cosine=np.maximum(normals@sun,0)
        ids=np.flatnonzero(cosine>0)
        visible=self.visible(origins[ids],sun*100)
        light[ids]+=cosine[ids,None]*visible[:,None]*np.asarray([.67,.71,.75])
        for lamp in self.lamps:
            delta=lamp-origins;distance=np.linalg.norm(delta,axis=1)
            cosine=np.maximum((normals*delta).sum(axis=1)/np.maximum(distance,.01),0)
            ids=np.flatnonzero((distance<14)&(cosine>0))
            if not len(ids):continue
            visible=self.visible(origins[ids],delta[ids]*.999)
            falloff=12/(1+.10*distance[ids]**2)*(1-distance[ids]/14)**2
            light[ids]+=(falloff*cosine[ids]*visible)[:,None]*np.asarray([1.,.67,.34])
        # Current baked shader applies the same 4.672 compensation as Quake maps.
        linear=np.clip(light/4.672,0,1)
        srgb=np.where(linear<=.0031308,linear*12.92,1.055*linear**(1/2.4)-.055)
        return np.round(srgb*255).astype(np.uint8)
    def batch(self,quads):
        if not quads:return []
        samples=[];normals=[]
        for points,normal,_,_ in quads:
            a=np.asarray(points[0]);u=np.asarray(points[1])-a;v=np.asarray(points[3])-a
            for y in range(self.TILE):
                for x in range(self.TILE):
                    samples.append(a+u*(x+.5)/self.TILE+v*(y+.5)/self.TILE);normals.append(normal)
        samples=np.asarray(samples,dtype=np.float32);normals=np.asarray(normals,dtype=np.float32)
        colors=np.concatenate([self.illuminate(samples[i:i+8192],normals[i:i+8192]) for i in range(0,len(samples),8192)]).reshape(-1,self.TILE,self.TILE,3)
        result=[];columns=self.SIZE//self.STRIDE;capacity=columns*columns
        for tile in colors:
            atlas=self.count//capacity;slot=self.count%capacity;self.count+=1
            if atlas==len(self.atlases):self.atlases.append(Image.new('RGB',(self.SIZE,self.SIZE)))
            x=(slot%columns)*self.STRIDE;y=(slot//columns)*self.STRIDE
            padded=np.pad(tile,((1,1),(1,1),(0,0)),mode='edge')
            self.atlases[atlas].paste(Image.fromarray(padded),(x,y))
            lo=((x+1)/self.SIZE,(y+1)/self.SIZE);hi=((x+5)/self.SIZE,(y+5)/self.SIZE)
            result.append((atlas,[(lo[0],lo[1]),(hi[0],lo[1]),(hi[0],hi[1]),(lo[0],hi[1])]))
        return result
    def save(self):
        names=[]
        for i,atlas in enumerate(self.atlases):
            name=f'lm_{i:04d}.png';atlas.save(self.output/name);names.append(name)
        return names
