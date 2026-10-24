use crate::hud_font::{FontKind, VectorFont, VectorText};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    ecs::{query::QueryData, system::SystemParam},
    light::NotShadowCaster,
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
    transform::TransformSystems,
    window::PrimaryWindow,
};
use boltrunner_client::{NetworkStats, Session};
use boltrunner_shared::{
    PlayerId, PlayerState, TICK_HZ,
    match_state::{MatchState, ScoreRow},
    powerups::PickupKind,
};
use lightyear::prelude::Predicted;

const HUD_SCALE: f32 = 0.75;
const MAX_DASH_ICONS: usize = 8;
const EFFECT_FADE_SECONDS: f32 = 0.25 / 3.0;
const HEALTH_COLOR: Color = Color::srgb(0.55, 0.85, 1.0);
const DASH_COLOR: Color = Color::srgb(0.60, 1.0, 0.70);
const CROWN_COLOR: Color = Color::srgb(1.0, 0.84, 0.20);
const GLOW_WIDTH: f32 = 8.0;
const GRADIENT_BOTTOM: f32 = 0.74;
const GRADIENT_TOP_LIGHTENING: f32 = 0.25;
const RANK_MARKER_X: f32 = 33.0;
const RANK_MARKER_SIZE: f32 = 18.0;
const RANK_CROWN_SIZE: f32 = 22.0;

fn gradient_colors(color: Color, enabled: bool) -> [Vec4; 2] {
    let base = color.to_linear().to_vec4();
    if enabled {
        [
            (base.xyz() * GRADIENT_BOTTOM).extend(1.0),
            base.lerp(Vec4::ONE, GRADIENT_TOP_LIGHTENING),
        ]
    } else {
        [base; 2]
    }
}

pub struct HudPlugin;
impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<HudShape>::default())
            .add_plugins(MaterialPlugin::<HudTextGlow>::default())
            .init_resource::<VectorFont>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (present, animate_powerups, sync_shape_glow)
                    .chain()
                    .after(boltrunner_client::NetworkPresentation),
            )
            .add_systems(PostUpdate, layout.before(TransformSystems::Propagate));
    }
}

// Analytic shapes are shaded directly at framebuffer resolution. Text is
// tessellated from font outlines; neither goes through a panel texture.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct HudShape {
    #[uniform(0)]
    parameters: Vec4,
    #[uniform(1)]
    dimensions: Vec4,
    #[uniform(2)]
    color_bottom: Vec4,
    #[uniform(3)]
    color_top: Vec4,
}
impl Material for HudShape {
    fn fragment_shader() -> ShaderRef {
        "shaders/hud.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        if self.parameters.w > 0.0 {
            AlphaMode::Add
        } else {
            AlphaMode::Blend
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct HudTextGlow {
    #[uniform(0)]
    color: Vec4,
}
impl Material for HudTextGlow {
    fn fragment_shader() -> ShaderRef {
        "shaders/hud_glow.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

#[derive(Component)]
struct ShapeGlow(Handle<HudShape>);
#[derive(Component)]
struct TextGlow(Entity);

#[derive(Clone, Copy)]
enum Shape {
    Ring,
    Chevron,
    Cross,
    Bullets,
    Lightning,
    Crown,
    Arrow,
    Health,
    Line,
}
impl Shape {
    fn color(self) -> Color {
        match self {
            Self::Cross | Self::Health => HEALTH_COLOR,
            Self::Chevron => DASH_COLOR,
            Self::Bullets => crate::powerups::pickup_color(PickupKind::RapidFire),
            Self::Lightning => crate::powerups::pickup_color(PickupKind::Speed),
            Self::Crown => CROWN_COLOR,
            _ => Color::WHITE,
        }
    }
}
#[derive(Component, Clone, Copy)]
enum Panel {
    Effects,
    Timer,
    Ranking,
    Health,
    Feed,
    Stats,
    Respawn,
}
impl Panel {
    fn size(self) -> Vec2 {
        match self {
            Self::Effects => Vec2::new(188.0, 80.0),
            Self::Timer => Vec2::new(184.0, 30.0),
            Self::Ranking => Vec2::new(416.0, 272.0),
            Self::Health => Vec2::new(324.0, 96.0),
            Self::Feed => Vec2::new(512.0, 180.0),
            Self::Stats => Vec2::new(320.0, 32.0),
            Self::Respawn => Vec2::new(448.0, 56.0),
        }
    }
    fn anchor(self) -> Vec2 {
        match self {
            Self::Effects => Vec2::new(0.0, 0.0),
            Self::Timer | Self::Respawn => Vec2::new(0.5, 0.0),
            Self::Ranking => Vec2::new(1.0, 0.0),
            Self::Health => Vec2::new(0.0, 1.0),
            Self::Feed | Self::Stats => Vec2::new(1.0, 1.0),
        }
    }
    fn rotation(self) -> Quat {
        let anchor = self.anchor();
        Quat::from_euler(
            EulerRot::YXZ,
            (1.0 - anchor.x * 2.0) * 12_f32.to_radians(),
            (1.0 - anchor.y * 2.0) * 7_f32.to_radians(),
            0.0,
        )
    }
}
#[derive(Component)]
enum Field {
    Health,
    Timer,
    Rank(usize),
    OwnRank,
    DashCount,
    Feed,
    Stats,
    Respawn,
}
#[derive(Component)]
enum Progress {
    Health,
    Ring(usize),
    Dash(usize),
}
#[derive(Component)]
struct DashIcon(usize);
#[derive(Component)]
struct Effect {
    index: usize,
    opacity: f32,
    offset: f32,
}
#[derive(Component)]
struct EffectShape(usize);
#[derive(Component)]
struct RankMarker(usize);
#[derive(Component)]
struct LeaderCrown;
#[derive(Component)]
struct OwnRankRow;
#[derive(QueryData)]
#[query_data(mutable)]
struct HudVisibility {
    visibility: &'static mut Visibility,
    dash: Option<&'static DashIcon>,
    marker: Option<&'static RankMarker>,
    own_row: Option<&'static OwnRankRow>,
}
#[derive(SystemParam)]
struct HudUi<'w, 's> {
    shapes: ResMut<'w, Assets<HudShape>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    font: ResMut<'w, VectorFont>,
    fields: Query<
        'w,
        's,
        (
            &'static Field,
            &'static mut VectorText,
            &'static mut Mesh3d,
            Option<&'static TextGlow>,
        ),
        Without<ShapeGlow>,
    >,
    text_glows: Query<'w, 's, &'static mut Mesh3d, (Without<Field>, Without<ShapeGlow>)>,
    progress: Query<'w, 's, (&'static Progress, &'static MeshMaterial3d<HudShape>)>,
    crowns: Query<'w, 's, &'static mut Transform, With<LeaderCrown>>,
    visible: Query<'w, 's, HudVisibility>,
}
fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_corners(Vec2::new(x, y), Vec2::new(x + width, y + height))
}
struct Painter<'a> {
    size: Vec2,
    glow: bool,
    quad: &'a Handle<Mesh>,
    text_material: &'a Handle<StandardMaterial>,
    glow_material: &'a Handle<HudTextGlow>,
    shapes: &'a mut Assets<HudShape>,
    meshes: &'a mut Assets<Mesh>,
    font: &'a mut VectorFont,
}
impl Painter<'_> {
    fn shape(&mut self, parent: &mut ChildSpawnerCommands, kind: Shape, area: Rect) -> Entity {
        self.colored_shape(parent, kind, area, kind.color())
    }
    fn colored_shape(
        &mut self,
        parent: &mut ChildSpawnerCommands,
        kind: Shape,
        area: Rect,
        color: Color,
    ) -> Entity {
        let size = area.size();
        let center = area.center();
        let [color_bottom, color_top] = gradient_colors(
            color,
            matches!(kind, Shape::Chevron | Shape::Cross | Shape::Health),
        );
        let material = HudShape {
            parameters: Vec4::new(kind as u32 as f32, 1.0, 1.0, 0.0),
            dimensions: size.extend(GLOW_WIDTH).extend(0.0),
            color_bottom,
            color_top,
        };
        let base = self.shapes.add(material.clone());
        let mut entity = parent.spawn((
            Mesh3d(self.quad.clone()),
            MeshMaterial3d(base.clone()),
            Transform::from_xyz(
                center.x / self.size.x - 0.5,
                0.5 - center.y / self.size.y,
                0.0,
            )
            .with_scale((size / self.size).extend(1.0)),
            RenderLayers::layer(crate::weapon::VIEW_LAYER),
            NotShadowCaster,
        ));
        if self.glow {
            let mut glow = material;
            glow.parameters.w = 1.0;
            let glow = self.shapes.add(glow);
            entity.with_children(|root| {
                root.spawn((
                    ShapeGlow(base),
                    Mesh3d(self.quad.clone()),
                    MeshMaterial3d(glow),
                    Transform::from_xyz(0.0, 0.0, -0.0001)
                        .with_scale(((size + GLOW_WIDTH * 2.0) / size).extend(1.0)),
                    RenderLayers::layer(crate::weapon::VIEW_LAYER),
                    NotShadowCaster,
                ));
            });
        }
        entity.id()
    }
    fn text(
        &mut self,
        parent: &mut ChildSpawnerCommands,
        field: Field,
        size: f32,
        area: Rect,
        align: Justify,
    ) {
        let color = match field {
            Field::Health => HEALTH_COLOR,
            Field::DashCount => DASH_COLOR,
            _ => Color::WHITE,
        };
        let text = VectorText {
            value: String::new(),
            size,
            area,
            panel_size: self.size,
            align,
            font: match field {
                Field::Health | Field::Timer => FontKind::Digits,
                _ => FontKind::Text,
            },
            colors: gradient_colors(color, matches!(field, Field::Health | Field::DashCount)),
            glow: self.glow,
        };
        let (mesh, glow) = self.font.meshes(&text);
        let glow = glow.map(|glow| {
            parent
                .spawn((
                    Mesh3d(self.meshes.add(glow)),
                    MeshMaterial3d(self.glow_material.clone()),
                    Transform::from_xyz(0.0, 0.0, -0.0001),
                    RenderLayers::layer(crate::weapon::VIEW_LAYER),
                    NotShadowCaster,
                ))
                .id()
        });
        let mut entity = parent.spawn((
            field,
            text,
            Mesh3d(self.meshes.add(mesh)),
            MeshMaterial3d(self.text_material.clone()),
            Transform::default(),
            RenderLayers::layer(crate::weapon::VIEW_LAYER),
            NotShadowCaster,
        ));
        if let Some(glow) = glow {
            entity.insert(TextGlow(glow));
        }
    }
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut shapes: ResMut<Assets<HudShape>>,
    mut glows: ResMut<Assets<HudTextGlow>>,
    mut font: ResMut<VectorFont>,
) {
    let quad = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![
                [-0.5, 0.5, 0.0],
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
            ],
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; 4])
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        )
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3])),
    );
    let text_material = materials.add(StandardMaterial {
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let glow_material = glows.add(HudTextGlow { color: Vec4::ONE });
    let health_size = font.digit_size(44.0);
    let timer_size = font.digit_size(30.0);
    let dash_offset = font.digit_width(health_size);
    for panel in [
        Panel::Effects,
        Panel::Timer,
        Panel::Ranking,
        Panel::Health,
        Panel::Feed,
        Panel::Stats,
        Panel::Respawn,
    ] {
        let size = panel.size();
        let mut paint = Painter {
            size,
            glow: matches!(panel, Panel::Health | Panel::Effects | Panel::Timer),
            quad: &quad,
            text_material: &text_material,
            glow_material: &glow_material,
            shapes: &mut shapes,
            meshes: &mut meshes,
            font: &mut font,
        };
        commands
            .spawn((panel, Transform::default(), Visibility::Hidden))
            .with_children(|root| match panel {
                Panel::Effects => {
                    for index in 0..2 {
                        root.spawn((
                            Effect {
                                index,
                                opacity: 0.0,
                                offset: 0.0,
                            },
                            Transform::default(),
                            Visibility::Hidden,
                        ))
                        .with_children(|effect| {
                            let color = crate::powerups::pickup_color(if index == 0 {
                                PickupKind::RapidFire
                            } else {
                                PickupKind::Speed
                            });
                            let ring = paint.colored_shape(
                                effect,
                                Shape::Ring,
                                rect(0.0, 0.0, 80.0, 80.0),
                                color,
                            );
                            effect
                                .commands()
                                .entity(ring)
                                .insert((Progress::Ring(index), EffectShape(index)));
                            let icon = paint.shape(
                                effect,
                                if index == 0 {
                                    Shape::Bullets
                                } else {
                                    Shape::Lightning
                                },
                                rect(23.0, 22.0, 34.0, 36.0),
                            );
                            effect.commands().entity(icon).insert(EffectShape(index));
                        });
                    }
                }
                Panel::Timer => paint.text(
                    root,
                    Field::Timer,
                    timer_size,
                    rect(0.0, 0.0, size.x, size.y),
                    Justify::Center,
                ),
                Panel::Ranking => {
                    let shared_marker_x = RANK_MARKER_X;
                    let text_x = 58.0;
                    for index in 0..5 {
                        let y = index as f32 * 40.0;
                        if index == 0 {
                            let crown = paint.shape(
                                root,
                                Shape::Crown,
                                rect(
                                    shared_marker_x - (RANK_CROWN_SIZE - RANK_MARKER_SIZE) * 0.5,
                                    y + 5.0,
                                    RANK_CROWN_SIZE,
                                    RANK_CROWN_SIZE,
                                ),
                            );
                            root.commands().entity(crown).insert(LeaderCrown);
                        }
                        let marker = paint.shape(
                            root,
                            Shape::Arrow,
                            rect(shared_marker_x, y + 8.0, RANK_MARKER_SIZE, RANK_MARKER_SIZE),
                        );
                        root.commands().entity(marker).insert(RankMarker(index));
                        paint.text(
                            root,
                            Field::Rank(index),
                            21.0,
                            rect(text_x, y, size.x - text_x, 36.0),
                            Justify::Left,
                        );
                    }
                    root.spawn((OwnRankRow, Transform::default(), Visibility::Inherited))
                        .with_children(|row| {
                            paint.shape(
                                row,
                                Shape::Line,
                                rect(text_x, 218.0, size.x - text_x - 8.0, 1.0),
                            );
                            paint.shape(
                                row,
                                Shape::Arrow,
                                rect(shared_marker_x, 232.0, RANK_MARKER_SIZE, RANK_MARKER_SIZE),
                            );
                            paint.text(
                                row,
                                Field::OwnRank,
                                21.0,
                                rect(text_x, 224.0, size.x - text_x, 36.0),
                                Justify::Left,
                            );
                        });
                }
                Panel::Health => {
                    for index in 0..MAX_DASH_ICONS {
                        let icon = paint.shape(
                            root,
                            Shape::Chevron,
                            rect(dash_offset + index as f32 * 38.0, 0.0, 32.0, 24.0),
                        );
                        root.commands()
                            .entity(icon)
                            .insert((DashIcon(index), Progress::Dash(index)));
                    }
                    paint.text(
                        root,
                        Field::DashCount,
                        18.0,
                        rect(0.0, 0.0, 324.0, 28.0),
                        Justify::Right,
                    );
                    paint.text(
                        root,
                        Field::Health,
                        health_size,
                        rect(0.0, 44.0, 110.0, 44.0),
                        Justify::Right,
                    );
                    paint.shape(root, Shape::Cross, rect(122.0, 44.0, 44.0, 44.0));
                    let health = paint.shape(root, Shape::Health, rect(178.0, 44.0, 142.0, 44.0));
                    root.commands().entity(health).insert(Progress::Health);
                }
                Panel::Feed => paint.text(
                    root,
                    Field::Feed,
                    17.0,
                    rect(0.0, 0.0, size.x, size.y),
                    Justify::Right,
                ),
                Panel::Stats => paint.text(
                    root,
                    Field::Stats,
                    15.0,
                    rect(0.0, 0.0, size.x, size.y),
                    Justify::Right,
                ),
                Panel::Respawn => paint.text(
                    root,
                    Field::Respawn,
                    20.0,
                    rect(0.0, 0.0, size.x, size.y),
                    Justify::Center,
                ),
            });
    }
}
fn set_fill(materials: &mut Assets<HudShape>, handle: &Handle<HudShape>, fraction: f32) {
    let fraction = fraction.clamp(0.0, 1.0);
    if materials
        .get(handle)
        .is_some_and(|material| material.parameters.y != fraction)
    {
        materials.get_mut(handle).unwrap().parameters.y = fraction;
    }
}
fn seconds(ticks: u16) -> u32 {
    (ticks as f64 / TICK_HZ).ceil() as u32
}
fn ranked_name(rank: usize, row: &ScoreRow) -> String {
    let mut short: String = row.nickname.chars().take(17).collect();
    if row.nickname.chars().count() > 17 {
        short.push('…');
    }
    format!("{}. {short} ({}/{})", rank + 1, row.kills, row.deaths)
}
fn present(
    session: Res<Session>,
    stats: Res<NetworkStats>,
    players: Query<(&PlayerId, &PlayerState), With<Predicted>>,
    rounds: Query<&MatchState>,
    mut ui: HudUi,
    time: Res<Time<Real>>,
    mut stats_elapsed: Local<f32>,
) {
    if !session.is_playing() {
        return;
    }
    *stats_elapsed += time.delta_secs();
    let update_stats = *stats_elapsed >= 0.1;
    if update_stats {
        *stats_elapsed = 0.0;
    }
    let local = players.iter().next();
    let state = local.map(|(_, state)| state);
    let health = format!("{:>3}", state.map_or(0, |state| state.health.0));
    let round = rounds.iter().next();
    let ranked = round.map(MatchState::ranked).unwrap_or_default();
    let own_rank = local.and_then(|(id, _)| ranked.iter().position(|row| row.id == id.0));
    let crown_center = if own_rank == Some(0) {
        RANK_CROWN_SIZE * 0.5
    } else {
        RANK_MARKER_X + RANK_MARKER_SIZE * 0.5
    };
    let crown_x = crown_center / Panel::Ranking.size().x - 0.5;
    for mut transform in &mut ui.crowns {
        if transform.translation.x != crown_x {
            transform.translation.x = crown_x;
        }
    }
    let effect_ticks = state.map_or([0; 2], |s| [s.rapid_fire_ticks, s.speed_ticks]);
    let effect_duration = state.map_or([1.0; 2], |s| {
        [s.tuning.rapid_fire_duration, s.tuning.speed_duration]
    });
    let capacity = state.map_or(0, |s| s.tuning.dash_charges as usize);
    let charges = state.map_or(0, |s| s.dash.charges as usize);
    let recharge = state.map_or(0.0, |s| {
        if s.death.is_some() {
            0.0
        } else {
            1.0 - s.dash.recharge_ticks as f32
                / (s.tuning.dash_recharge_time * TICK_HZ as f32).max(1.0)
        }
    });
    for (progress, material) in &ui.progress {
        let fraction = match *progress {
            Progress::Health => state.map_or(0.0, |s| {
                s.health.0 as f32 / s.tuning.max_health.max(1) as f32
            }),
            Progress::Ring(i) => {
                effect_ticks[i] as f32 / (effect_duration[i] * TICK_HZ as f32).max(1.0)
            }
            Progress::Dash(i) => {
                if i < charges {
                    1.0
                } else if i == charges {
                    recharge
                } else {
                    0.0
                }
            }
        };
        set_fill(&mut ui.shapes, &material.0, fraction);
    }
    for (field, mut value, mut mesh, glow) in &mut ui.fields {
        let label = match *field {
            Field::Health => health.clone(),
            Field::Timer => round.map_or_else(
                || "--:--".into(),
                |round| {
                    format!(
                        "{:02}:{:02}",
                        round.remaining_seconds / 60,
                        round.remaining_seconds % 60
                    )
                },
            ),
            Field::Rank(index) => ranked
                .get(index)
                .map_or_else(String::new, |row| ranked_name(index, row)),
            Field::OwnRank => own_rank
                .filter(|rank| *rank >= 5)
                .map_or_else(String::new, |rank| ranked_name(rank, ranked[rank])),
            Field::DashCount => {
                if capacity > MAX_DASH_ICONS {
                    format!("{charges}/{capacity}")
                } else {
                    String::new()
                }
            }
            Field::Feed => round.map_or_else(String::new, |round| {
                round
                    .kill_feed
                    .iter()
                    .rev()
                    .take(5)
                    .map(|entry| {
                        format!(
                            "{}  ›  {}",
                            entry.killer.as_deref().unwrap_or("Arena"),
                            entry.victim
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }),
            Field::Stats => {
                if !update_stats {
                    continue;
                }
                let fps = if stats.frame_ms > 0.0 {
                    1000.0 / stats.frame_ms
                } else {
                    0.0
                };
                let ping = stats
                    .ping_ms
                    .map_or_else(|| "—".into(), |ms| format!("{ms:.0}"));
                format!("{fps:.0} FPS  /  {ping} MS")
            }
            Field::Respawn => state
                .and_then(|state| {
                    state.death.map(|death| {
                        if state.match_paused {
                            "WAITING FOR NEXT MATCH".into()
                        } else {
                            format!("RESPAWN IN {}", seconds(death.remaining_ticks))
                        }
                    })
                })
                .unwrap_or_default(),
        };
        let row = match *field {
            Field::Rank(index) => ranked.get(index).copied(),
            Field::OwnRank => own_rank.map(|rank| ranked[rank]),
            _ => None,
        };
        let colors = row.map_or(value.colors, |row| {
            let [r, g, b] = row.color.rgb();
            [Color::srgb(r, g, b).to_linear().to_vec4(); 2]
        });
        if value.value != label || value.colors != colors {
            value.value = label;
            value.colors = colors;
            let (base, halo) = ui.font.meshes(&value);
            mesh.0 = ui.meshes.add(base);
            if let (Some(glow), Some(halo)) = (glow, halo)
                && let Ok(mut mesh) = ui.text_glows.get_mut(glow.0)
            {
                mesh.0 = ui.meshes.add(halo);
            }
        }
    }
    for HudVisibilityItem {
        mut visibility,
        dash,
        marker,
        own_row,
    } in &mut ui.visible
    {
        let visible = if let Some(dash) = dash {
            dash.0 < capacity
        } else if let Some(marker) = marker {
            own_rank == Some(marker.0)
        } else if own_row.is_some() {
            own_rank.is_some_and(|rank| rank >= 5)
        } else {
            continue;
        };
        let next = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
}

fn animate_powerups(
    session: Res<Session>,
    players: Query<&PlayerState, With<Predicted>>,
    mut effects: Query<(&mut Effect, &mut Transform, &mut Visibility)>,
    icons: Query<(&EffectShape, &MeshMaterial3d<HudShape>)>,
    mut materials: ResMut<Assets<HudShape>>,
    time: Res<Time<Real>>,
    mut order: Local<Vec<usize>>,
) {
    if !session.is_playing() {
        return;
    }
    let ticks = players
        .iter()
        .next()
        .map_or([0; 2], |s| [s.rapid_fire_ticks, s.speed_ticks]);
    let mut visible = [false; 2];
    for (effect, _, _) in &effects {
        visible[effect.index] = effect.opacity > 0.0;
    }
    // An expired icon keeps its slot until its fade finishes.
    order.retain(|&index| ticks[index] > 0 || visible[index]);
    for (index, remaining) in ticks.iter().enumerate() {
        if *remaining > 0 && !order.contains(&index) {
            order.push(index);
        }
    }
    order.sort_by_key(|&index| ticks[index]);
    let slide = 1.0 - (-time.delta_secs() / 0.07).exp();
    let mut opacity = [0.0; 2];
    for (mut effect, mut transform, mut visibility) in &mut effects {
        if let Some(slot) = order.iter().position(|&index| index == effect.index) {
            let target = slot as f32 * 108.0;
            if !visible[effect.index] {
                effect.offset = target;
            } else {
                effect.offset += (target - effect.offset) * slide;
            }
        }
        effect.opacity = if ticks[effect.index] > 0 {
            (effect.opacity + time.delta_secs() / EFFECT_FADE_SECONDS).min(1.0)
        } else {
            (effect.opacity - time.delta_secs() / EFFECT_FADE_SECONDS).max(0.0)
        };
        opacity[effect.index] = effect.opacity;
        let x = effect.offset / Panel::Effects.size().x;
        if transform.translation.x != x {
            transform.translation.x = x;
        }
        let next = if effect.opacity > 0.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
    }
    for (effect, material) in &icons {
        let alpha = opacity[effect.0];
        if materials
            .get(&material.0)
            .is_some_and(|material| material.parameters.z != alpha)
        {
            materials.get_mut(&material.0).unwrap().parameters.z = alpha;
        }
    }
}

fn sync_shape_glow(
    glows: Query<(&ShapeGlow, &MeshMaterial3d<HudShape>)>,
    mut materials: ResMut<Assets<HudShape>>,
) {
    for (source, glow) in &glows {
        let Some(base) = materials.get(&source.0) else {
            continue;
        };
        let mut parameters = base.parameters;
        parameters.w = 1.0;
        if materials
            .get(&glow.0)
            .is_some_and(|glow| glow.parameters != parameters)
        {
            materials.get_mut(&glow.0).unwrap().parameters = parameters;
        }
    }
}

fn layout(
    session: Res<Session>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<&Projection, With<crate::weapon::WeaponCamera>>,
    mut panels: Query<(&Panel, &mut Transform, &mut Visibility)>,
) {
    let Projection::Perspective(projection) = &**camera else {
        return;
    };
    let viewport = Vec2::new(window.width(), window.height());
    if viewport.min_element() <= 0.0 {
        return;
    }
    let viewport_scale = (viewport.x / 1600.0).min(viewport.y / 900.0);
    let scale = viewport_scale * HUD_SCALE;
    let distance = 0.12;
    let unit = 2.0 * distance * (projection.fov * 0.5).tan() / viewport.y;
    for (panel, mut transform, mut visibility) in &mut panels {
        let visible = if session.is_playing() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != visible {
            *visibility = visible;
        }
        let size = panel.size();
        let panel_scale = if matches!(panel, Panel::Ranking) {
            scale * 0.85
        } else {
            scale
        };
        let anchor = panel.anchor();
        let mut margin = Vec2::splat(28.0) * viewport_scale;
        if matches!(panel, Panel::Respawn) {
            margin.y += 120.0 * scale;
        }
        if matches!(panel, Panel::Feed) {
            margin.y += 68.0 * scale;
        }
        let center = margin + size * panel_scale * 0.5;
        let center = Vec2::new(
            center.x + (viewport.x - center.x * 2.0) * anchor.x,
            center.y + (viewport.y - center.y * 2.0) * anchor.y,
        );
        let mut next = Transform {
            translation: Vec3::new(
                (center.x - viewport.x * 0.5) * unit,
                (viewport.y * 0.5 - center.y) * unit,
                -distance,
            ),
            rotation: panel.rotation(),
            scale: (size * panel_scale * unit).extend(1.0),
        };
        // Keep each outer edge inside its margin after perspective projection.
        for _ in 0..3 {
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for corner in [
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.5, 0.5, 0.0),
                Vec3::new(-0.5, 0.5, 0.0),
            ] {
                let p = next.transform_point(corner);
                let screen = viewport * 0.5 + Vec2::new(p.x, -p.y) / unit * (distance / -p.z);
                min = min.min(screen);
                max = max.max(screen);
            }
            let edge = min + (max - min) * anchor;
            let target = margin + (viewport - margin * 2.0) * anchor;
            let correction = target - edge;
            next.translation.x += correction.x * unit;
            next.translation.y -= correction.y * unit;
        }
        if *transform != next {
            *transform = next;
        }
    }
}
