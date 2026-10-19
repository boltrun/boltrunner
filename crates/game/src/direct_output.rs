use bevy::{
    camera::{CameraOutputMode, ClearColorConfig, RenderTarget},
    core_pipeline::core_3d::{
        AlphaMask3d, MainOpaquePass3dNode, MainTransparentPass3dNode, Opaque3d, Transmissive3d,
        Transparent3d,
        graph::{Core3d, Node3d},
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_graph::{NodeRunError, RenderGraph, RenderGraphContext, ViewNode, ViewNodeRunner},
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{PipelineCache, RenderPassDescriptor, StoreOp},
        renderer::{RenderAdapterInfo, RenderContext},
        view::{ExtractedView, ViewTarget},
    },
    ui_render::{
        TransparentUi, UiCameraView, UiViewTarget,
        graph::{NodeUi, SubGraphUi},
    },
};

// The WebGL client has two LDR views, MSAA Off and no screen-space postprocessing.
// Draw world, weapon and UI into the same sRGB output attachment, then let wgpu
// present it. This removes Bevy's otherwise redundant full-resolution copy.
pub(crate) fn install(app: &mut App) {
    app.add_systems(PostStartup, configure_cameras);
    app.sub_app_mut(RenderApp)
        .add_systems(RenderStartup, install_nodes);
}

fn configure_cameras(
    adapter: Res<RenderAdapterInfo>,
    mut cameras: Query<
        (
            &mut Camera,
            Has<crate::view::PlayerCamera>,
            &Msaa,
            Has<bevy::render::view::Hdr>,
            &RenderTarget,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
        ),
        With<Camera3d>,
    >,
) {
    assert_eq!(
        adapter.backend.to_str(),
        "gl",
        "Direct output targets the WebGL renderer"
    );
    assert_eq!(cameras.iter().count(), 2);
    for (mut camera, world, msaa, hdr, target, tone) in &mut cameras {
        assert_eq!(*msaa, Msaa::Off);
        assert!(!hdr && camera.viewport.is_none());
        assert_eq!(camera.order, if world { 0 } else { 1 });
        assert_eq!(matches!(camera.clear_color, ClearColorConfig::None), !world);
        assert!(
            tone.is_none_or(|tone| *tone == bevy::core_pipeline::tonemapping::Tonemapping::None)
        );
        assert!(matches!(
            target,
            RenderTarget::Window(bevy::window::WindowRef::Primary)
        ));
        camera.output_mode = CameraOutputMode::Skip;
    }
}

pub(crate) fn install_nodes(world: &mut World) {
    let opaque = ViewNodeRunner::new(DirectOpaque, world);
    let transparent = ViewNodeRunner::new(DirectTransparent, world);
    let ui = ViewNodeRunner::new(DirectUi, world);
    let mut graph = world.resource_mut::<RenderGraph>();
    let core = graph.get_sub_graph_mut(Core3d).unwrap();
    core.get_node_state_mut(Node3d::MainOpaquePass)
        .unwrap()
        .node = Box::new(opaque);
    core.get_node_state_mut(Node3d::MainTransparentPass)
        .unwrap()
        .node = Box::new(transparent);
    core.get_sub_graph_mut(SubGraphUi)
        .unwrap()
        .get_node_state_mut(NodeUi::UiPass)
        .unwrap()
        .node = Box::new(ui);
}

struct DirectOpaque;
impl ViewNode for DirectOpaque {
    type ViewQuery = <MainOpaquePass3dNode as ViewNode>::ViewQuery;
    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        context: &mut RenderContext<'w>,
        (camera, view, target, depth, sky_pipeline, sky_group, uniform, resolution_override): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        assert!(!view.hdr && resolution_override.is_none() && camera.viewport.is_none());
        assert_eq!(
            target.main_texture_format(),
            target.out_texture_view_format()
        );
        let (Some(opaques), Some(masks)) = (
            world.get_resource::<ViewBinnedRenderPhases<Opaque3d>>(),
            world.get_resource::<ViewBinnedRenderPhases<AlphaMask3d>>(),
        ) else {
            return Ok(());
        };
        let (Some(opaque), Some(mask)) = (
            opaques.get(&view.retained_view_entity),
            masks.get(&view.retained_view_entity),
        ) else {
            return Ok(());
        };
        if let Some(phases) = world.get_resource::<ViewSortedRenderPhases<Transmissive3d>>() {
            assert!(
                phases
                    .get(&view.retained_view_entity)
                    .is_none_or(|phase| phase.items.is_empty()),
                "Direct output does not support screen-space transmission"
            );
        }
        let clear = match camera.clear_color {
            ClearColorConfig::Default => Some(world.resource::<ClearColor>().0.to_linear()),
            ClearColorConfig::Custom(color) => Some(color.to_linear()),
            ClearColorConfig::None => None,
        };
        // Both views share this output attachment's clear-once state. The world
        // clears it; the weapon, transparent geometry and UI retain its color.
        let colors = [Some(target.out_texture_color_attachment(clear))];
        let diagnostics = context.diagnostic_recorder();
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("direct_opaque"),
            color_attachments: &colors,
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let span = diagnostics.pass_span(&mut pass, "direct_opaque");
        if !opaque.is_empty() {
            if let Err(error) = opaque.render(&mut pass, world, graph.view_entity()) {
                error!("Error rendering opaque geometry: {error:?}");
            }
        }
        if !mask.is_empty() {
            if let Err(error) = mask.render(&mut pass, world, graph.view_entity()) {
                error!("Error rendering masked geometry: {error:?}");
            }
        }
        if let (Some(pipeline), Some(bind_group)) = (sky_pipeline, sky_group) {
            if let Some(pipeline) = world
                .resource::<PipelineCache>()
                .get_render_pipeline(pipeline.0)
            {
                pass.set_render_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group.0.0, &[uniform.offset, bind_group.0.1]);
                pass.draw(0..3, 0..1);
            }
        }
        span.end(&mut pass);
        Ok(())
    }
}

struct DirectTransparent;
impl ViewNode for DirectTransparent {
    type ViewQuery = <MainTransparentPass3dNode as ViewNode>::ViewQuery;
    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        context: &mut RenderContext<'w>,
        (camera, view, target, depth, resolution_override): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        assert!(camera.viewport.is_none() && resolution_override.is_none());
        let Some(phases) = world.get_resource::<ViewSortedRenderPhases<Transparent3d>>() else {
            return Ok(());
        };
        let Some(phase) = phases.get(&view.retained_view_entity) else {
            return Ok(());
        };
        if phase.items.is_empty() {
            return Ok(());
        }
        let colors = [Some(target.out_texture_color_attachment(None))];
        let diagnostics = context.diagnostic_recorder();
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("direct_transparent"),
            color_attachments: &colors,
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let span = diagnostics.pass_span(&mut pass, "direct_transparent");
        if let Err(error) = phase.render(&mut pass, world, graph.view_entity()) {
            error!("Error rendering transparent geometry: {error:?}");
        }
        span.end(&mut pass);
        Ok(())
    }
}

struct DirectUi;
impl ViewNode for DirectUi {
    type ViewQuery = (
        &'static ExtractedView,
        &'static UiViewTarget,
        Option<&'static UiCameraView>,
    );
    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        context: &mut RenderContext<'w>,
        (view, ui_target, ui_camera): QueryItem<'w, '_, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        let Some(phases) = world.get_resource::<ViewSortedRenderPhases<TransparentUi>>() else {
            return Ok(());
        };
        let Some(phase) = phases.get(&view.retained_view_entity) else {
            return Ok(());
        };
        if phase.items.is_empty() {
            return Ok(());
        }
        let (Some(target), Some(camera)) = (
            world.get::<ViewTarget>(ui_target.0),
            world.get::<ExtractedCamera>(ui_target.0),
        ) else {
            return Ok(());
        };
        assert!(camera.viewport.is_none());
        assert_eq!(
            target.main_texture_format(),
            target.out_texture_view_format()
        );
        let colors = [Some(target.out_texture_color_attachment(None))];
        let diagnostics = context.diagnostic_recorder();
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("direct_ui"),
            color_attachments: &colors,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let span = diagnostics.pass_span(&mut pass, "direct_ui");
        if let Err(error) = phase.render(
            &mut pass,
            world,
            ui_camera.map_or(graph.view_entity(), |camera| camera.0),
        ) {
            error!("Error rendering UI: {error:?}");
        }
        span.end(&mut pass);
        Ok(())
    }
}
