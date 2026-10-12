use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers,
    path::{Path, math::point},
};
use rustybuzz::{
    Face, UnicodeBuffer,
    ttf_parser::{GlyphId, OutlineBuilder},
};
use std::collections::HashMap;

#[derive(Component)]
pub(super) struct VectorText {
    pub value: String,
    pub size: f32,
    pub area: Rect,
    pub panel_size: Vec2,
    pub align: Justify,
    pub font: FontKind,
    pub colors: [Vec4; 2],
    pub glow: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FontKind {
    Text,
    Digits,
}

#[derive(Resource)]
pub(super) struct VectorFont {
    text: FontGeometry,
    digits: FontGeometry,
}

struct FontGeometry {
    face: Face<'static>,
    glyphs: HashMap<u16, VertexBuffers<Vec2, u32>>,
    tessellator: FillTessellator,
}

impl Default for VectorFont {
    fn default() -> Self {
        Self {
            text: FontGeometry::new(include_bytes!("../../../assets/fonts/DejaVuSansMono.ttf")),
            digits: FontGeometry::new(include_bytes!(
                "../../../assets/fonts/DSEG7Classic-Bold.ttf"
            )),
        }
    }
}

impl VectorFont {
    pub fn digit_size(&self, height: f32) -> f32 {
        let bounds = self
            .digits
            .face
            .glyph_index('0')
            .and_then(|id| self.digits.face.glyph_bounding_box(id))
            .expect("bundled HUD font must contain digits");
        height * self.digits.face.units_per_em() as f32 / (bounds.y_max - bounds.y_min) as f32
    }

    pub fn digit_width(&self, size: f32) -> f32 {
        let advance = self
            .digits
            .face
            .glyph_index('0')
            .and_then(|id| self.digits.face.glyph_hor_advance(id))
            .expect("bundled HUD font must contain digits");
        size * advance as f32 / self.digits.face.units_per_em() as f32
    }

    pub fn meshes(&mut self, text: &VectorText) -> (Mesh, Option<Mesh>) {
        match text.font {
            FontKind::Text => self.text.meshes(text),
            FontKind::Digits => self.digits.meshes(text),
        }
    }
}

impl FontGeometry {
    fn new(bytes: &'static [u8]) -> Self {
        Self {
            face: Face::from_slice(bytes, 0).expect("bundled HUD font must be valid"),
            glyphs: HashMap::new(),
            tessellator: FillTessellator::new(),
        }
    }

    fn meshes(&mut self, text: &VectorText) -> (Mesh, Option<Mesh>) {
        let em = self.face.units_per_em() as f32;
        let space = self.face.glyph_index(' ');
        let digit_advance = self
            .face
            .glyph_index('0')
            .and_then(|id| self.face.glyph_hor_advance(id));
        let advance = |id, x| {
            // Leading blanks reserve full digit cells in numeric counters.
            if text.font == FontKind::Digits && space == Some(GlyphId(id)) {
                digit_advance.unwrap_or_default() as f32 / em
            } else {
                x as f32 / em
            }
        };
        let lines = text
            .value
            .split('\n')
            .map(|line| {
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(line);
                buffer.guess_segment_properties();
                rustybuzz::shape(&self.face, &[], buffer)
            })
            .collect::<Vec<_>>();
        let widths = lines
            .iter()
            .map(|line| {
                line.glyph_positions()
                    .iter()
                    .zip(line.glyph_infos())
                    .map(|(p, glyph)| advance(glyph.glyph_id as u16, p.x_advance))
                    .sum::<f32>()
            })
            .collect::<Vec<_>>();
        let width = widths.iter().copied().fold(0.0_f32, f32::max);
        let size = text.size.min(text.area.width() / width.max(f32::EPSILON));
        let line_height = (self.face.ascender() - self.face.descender()) as f32 / em * size;
        let top = text.area.min.y + (text.area.height() - line_height * lines.len() as f32) * 0.5;
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let width = widths[index] * size;
            let offset = match text.align {
                Justify::Center => (text.area.width() - width) * 0.5,
                Justify::Right => text.area.width() - width,
                _ => 0.0,
            };
            let mut pen = Vec2::new(
                text.area.min.x + offset,
                top + index as f32 * line_height + self.face.ascender() as f32 / em * size,
            );
            for (glyph, placement) in line.glyph_infos().iter().zip(line.glyph_positions()) {
                let id = glyph.glyph_id as u16;
                if !self.glyphs.contains_key(&id) {
                    let mut outline = Outline::default();
                    self.face.outline_glyph(GlyphId(id), &mut outline);
                    let mut builder = Path::builder();
                    for command in outline.0 {
                        match command {
                            Command::Move(p) => {
                                builder.begin(point(p.x / em, p.y / em));
                            }
                            Command::Line(p) => {
                                builder.line_to(point(p.x / em, p.y / em));
                            }
                            Command::Quad(c, p) => {
                                builder.quadratic_bezier_to(
                                    point(c.x / em, c.y / em),
                                    point(p.x / em, p.y / em),
                                );
                            }
                            Command::Cubic(a, b, p) => {
                                builder.cubic_bezier_to(
                                    point(a.x / em, a.y / em),
                                    point(b.x / em, b.y / em),
                                    point(p.x / em, p.y / em),
                                );
                            }
                            Command::Close => builder.end(true),
                        }
                    }
                    let path = builder.build();
                    let mut geometry = VertexBuffers::new();
                    self.tessellator
                        .tessellate_path(
                            &path,
                            &FillOptions::default().with_tolerance(0.001),
                            &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| {
                                let p = vertex.position();
                                Vec2::new(p.x, p.y)
                            }),
                        )
                        .expect("bundled HUD glyph must tessellate");
                    self.glyphs.insert(id, geometry);
                }
                let geometry = &self.glyphs[&id];
                let start = positions.len() as u32;
                let offset =
                    Vec2::new(placement.x_offset as f32, -placement.y_offset as f32) / em * size;
                for vertex in &geometry.vertices {
                    let p = pen + offset + Vec2::new(vertex.x, -vertex.y) * size;
                    positions.push([
                        p.x / text.panel_size.x - 0.5,
                        0.5 - p.y / text.panel_size.y,
                        0.0,
                    ]);
                }
                indices.extend(geometry.indices.iter().map(|i| start + i));
                pen += Vec2::new(
                    advance(id, placement.x_advance),
                    -placement.y_advance as f32 / em,
                ) * size;
            }
        }
        if !positions.is_empty() {
            let (min, max) = if text.font == FontKind::Digits {
                let bounds = self
                    .face
                    .glyph_index('0')
                    .and_then(|id| self.face.glyph_bounding_box(id))
                    .unwrap();
                let baseline = top + self.face.ascender() as f32 / em * size;
                (
                    0.5 - (baseline - bounds.y_min as f32 / em * size) / text.panel_size.y,
                    0.5 - (baseline - bounds.y_max as f32 / em * size) / text.panel_size.y,
                )
            } else {
                (
                    positions.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min),
                    positions
                        .iter()
                        .map(|p| p[1])
                        .fold(f32::NEG_INFINITY, f32::max),
                )
            };
            let center = 0.5 - text.area.center().y / text.panel_size.y;
            for position in &mut positions {
                position[1] += center - (min + max) * 0.5;
            }
        }
        // Keep an empty label a valid mesh while drawing no triangles.
        if positions.is_empty() {
            positions.push([0.0, 0.0, 0.0]);
        }
        let bottom = 0.5 - text.area.max.y / text.panel_size.y;
        let height = text.area.height() / text.panel_size.y;
        let colors = positions
            .iter()
            .map(|p| {
                let t = ((p[1] - bottom) / height).clamp(0.0, 1.0);
                text.colors[0].lerp(text.colors[1], t).to_array()
            })
            .collect::<Vec<_>>();
        let glow = text.glow.then(|| {
            // A normalized Gaussian kernel samples the actual filled glyphs. This
            // avoids contour offsets and preserves holes and sharp corners.
            let kernel = [0.054488685, 0.24420135, 0.40261995, 0.24420135, 0.054488685];
            let mut glow_positions = Vec::with_capacity(positions.len() * 25);
            let mut glow_weights = Vec::with_capacity(positions.len() * 25);
            let mut glow_indices = Vec::with_capacity(indices.len() * 25);
            for (y, wy) in kernel.iter().enumerate() {
                for (x, wx) in kernel.iter().enumerate() {
                    let start = glow_positions.len() as u32;
                    let offset = Vec2::new(x as f32 - 2.0, y as f32 - 2.0) * 1.5 / text.panel_size;
                    for (p, color) in positions.iter().zip(&colors) {
                        glow_positions.push([p[0] + offset.x, p[1] + offset.y, p[2]]);
                        glow_weights.push([color[0], color[1], color[2], wx * wy]);
                    }
                    glow_indices.extend(indices.iter().map(|i| start + i));
                }
            }
            let count = glow_positions.len();
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, glow_positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, glow_weights)
            .with_inserted_indices(Indices::U32(glow_indices))
        });
        let count = positions.len();
        let fill = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices));
        (fill, glow)
    }
}

#[derive(Default)]
struct Outline(Vec<Command>);
enum Command {
    Move(Vec2),
    Line(Vec2),
    Quad(Vec2, Vec2),
    Cubic(Vec2, Vec2, Vec2),
    Close,
}
impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Move(Vec2::new(x, y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.push(Command::Line(Vec2::new(x, y)));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0
            .push(Command::Quad(Vec2::new(x1, y1), Vec2::new(x, y)));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.push(Command::Cubic(
            Vec2::new(x1, y1),
            Vec2::new(x2, y2),
            Vec2::new(x, y),
        ));
    }
    fn close(&mut self) {
        self.0.push(Command::Close);
    }
}
