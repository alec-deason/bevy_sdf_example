use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::{Camera, OrthographicProjection, VisibleEntities, VisibleEntity},
        draw::DrawContext,
        pipeline::{PipelineDescriptor, PipelineSpecialization},
        render_graph::{base, base::MainPass, AssetRenderResourcesNode, RenderGraph},
        renderer::{BindGroup, RenderResource, RenderResourceBindings, RenderResources},
        shader::{ShaderStage, ShaderStages},
        RenderStage, RenderSystem,
    },
    transform::TransformSystem,
};
use std::collections::HashMap;

fn main() {
    App::build()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_scene.system())
        .add_startup_system(setup_renderer.system())
        .add_asset::<SDFFunctions>()
        .add_system_to_stage(
            CoreStage::PostUpdate,
            update_tiles
                .system()
                .after(RenderSystem::VisibleEntities)
                .after(TransformSystem::TransformPropagate),
        )
        .add_system_to_stage(RenderStage::Draw, draw.system())
        .run();
}

fn setup_scene(commands: &mut Commands) {
    commands.spawn(OrthographicCameraBundle::new_2d());
    commands.spawn((
        SDFObject::new(SDF::Circle(64.0), Color::rgb(0.0, 1.0, 1.0), 0.0),
        Transform::from_xyz(200.0, 0.0, 0.0),
        GlobalTransform::default(),
    ));
    commands.spawn((
        SDFObject::new(SDF::Rectangle(64.0, 64.0), Color::rgb(1.0, 0.0, 1.0), 1.0),
        Transform::from_xyz(-200.0, 0.0, 0.0),
        GlobalTransform::default(),
    ));
}

const MAX_FUNCTIONS: usize = 100;
const PANEL_WIDTH: usize = 16;
const PANEL_HEIGHT: usize = 16;

#[derive(Debug, RenderResources, TypeUuid)]
#[uuid = "29880e29-0f5f-4940-bc0b-1b19a2b35780"]
struct SDFFunctions {
    panel_width: u32,
    panel_height: u32,
    #[render_resources(buffer)]
    functions: Vec<f32>,
    #[render_resources(buffer)]
    tile_function_count: Vec<u32>,
    #[render_resources(buffer)]
    function_indices: Vec<u32>,
}
impl Default for SDFFunctions {
    fn default() -> Self {
        Self {
            panel_width: PANEL_WIDTH as u32,
            panel_height: PANEL_HEIGHT as u32,
            functions: vec![],
            tile_function_count: vec![],
            function_indices: vec![],
        }
    }
}
struct SDFPanel;

#[derive(Clone, Debug)]
pub struct SDFObject {
    pub function: SDF,
    pub noise_weight: f32,
    pub color: Color,
}
impl SDFObject {
    pub fn new(sdf: SDF, color: Color, noise_weight: f32) -> Self {
        Self {
            function: sdf,
            noise_weight,
            color,
        }
    }

    fn as_f32s(&self) -> [f32; 8] {
        let f = self.function.as_f32s();
        [
            f[0],
            f[1],
            f[2],
            self.noise_weight,
            self.color.r(),
            self.color.g(),
            self.color.b(),
            self.color.a(),
        ]
    }
}

#[derive(Clone, Debug)]
pub enum SDF {
    Circle(f32),
    Triangle(f32, f32),
    Rectangle(f32, f32),
}
impl Default for SDF {
    fn default() -> Self {
        SDF::Circle(0.0)
    }
}
impl SDF {
    fn as_f32s(&self) -> [f32; 3] {
        match self {
            SDF::Circle(r) => [0.0, *r, 0.0],
            SDF::Rectangle(hw, hh) => [1.0, *hw, *hh],
            SDF::Triangle(w, h) => [2.0, *w, *h],
        }
    }
}

struct SDFPipeline(Handle<PipelineDescriptor>);
fn setup_renderer(
    commands: &mut Commands,
    asset_server: Res<AssetServer>,
    mut pipelines: ResMut<Assets<PipelineDescriptor>>,
    mut functions: ResMut<Assets<SDFFunctions>>,
    mut render_graph: ResMut<RenderGraph>,
) {
    let vert = asset_server.load::<Shader, _>("sdf.vert");
    let frag = asset_server.load::<Shader, _>("sdf.frag");
    let pipeline_handle = pipelines.add(PipelineDescriptor::default_config(ShaderStages {
        vertex: vert,
        fragment: Some(frag),
    }));
    commands.insert_resource(SDFPipeline(pipeline_handle));

    render_graph.add_system_node(
        "sdf_functions",
        AssetRenderResourcesNode::<SDFFunctions>::new(false),
    );
    render_graph
        .add_node_edge("sdf_functions", base::node::MAIN_PASS)
        .unwrap();

    commands.spawn((
        SDFPanel,
        functions.add(SDFFunctions::default()),
        Draw::default(),
        MainPass,
    ));
}

fn update_tiles(
    mut functions: ResMut<Assets<SDFFunctions>>,
    mut cameras: Query<(&Transform, &OrthographicProjection, &mut VisibleEntities)>,
    object_query: Query<(Entity, &GlobalTransform, &SDFObject)>,
    panel_query: Query<(Entity, &Handle<SDFFunctions>), With<SDFPanel>>,
) {
    if let (Some((entity, handle)), Some((camera_transform, proj, mut visible_entities))) =
        (panel_query.iter().next(), cameras.iter_mut().next())
    {
        visible_entities.value.push(VisibleEntity {
            entity,
            order: bevy::core::FloatOrd(0.0),
        });
        let mut functions = functions.get_mut(handle).unwrap();

        let mut indices = HashMap::new();
        let mut params = HashMap::new();
        let tree: Vec<_> = object_query
            .iter()
            .map(|(e, t, o)| {
                let (axis, mut r) = t.rotation.to_axis_angle();
                if axis.z < 0.0 {
                    r *= -1.0;
                }
                rstar::primitives::PointWithData::new(
                    (e, o, r, t.translation.z),
                    [t.translation.x, t.translation.y],
                )
            })
            .collect();
        let tree = rstar::RTree::bulk_load(tree);

        let w = (proj.right - proj.left) * proj.scale;
        let cw = w / PANEL_WIDTH as f32;
        let h = (proj.top - proj.bottom) * proj.scale;
        let ch = h / PANEL_HEIGHT as f32;
        let max_d = ((cw.max(ch) / camera_transform.scale.x) / 2.0 + 32.0f32 * 4.0).powi(2);
        for x in 0..PANEL_WIDTH {
            let xx = proj.left * proj.scale + x as f32 * cw + cw / 2.0;
            for y in 0..PANEL_HEIGHT {
                let yy = proj.bottom * proj.scale + y as f32 * ch + ch / 2.0;
                let pp = camera_transform.mul_vec3(Vec3::new(xx, yy, 0.0));
                let i = x + y * PANEL_WIDTH;
                for (j, (p, d)) in tree
                    .nearest_neighbor_iter_with_distance_2(&[pp.x, pp.y])
                    .enumerate()
                {
                    let (e, o, r, z) = p.data;
                    if j >= MAX_FUNCTIONS || d > max_d {
                        break;
                    }
                    indices.entry(e).or_insert(vec![]).push(i);
                    let p = p.position();
                    let mut ps = Vec::with_capacity(3 + 8);
                    ps.push(p[0]);
                    ps.push(p[1]);
                    ps.push(z);
                    ps.push(r);
                    ps.extend(&o.as_f32s());
                    params.insert(e, ps);
                }
            }
        }
        let mut params_vec: Vec<f32> = Vec::with_capacity(params.len() + 1);
        let params: HashMap<_, _> = params
            .into_iter()
            .enumerate()
            .map(|(i, (e, ps))| {
                params_vec.extend(&ps);
                (e, i)
            })
            .collect();
        params_vec.extend(
            &SDFObject::new(SDF::Circle(std::f32::NEG_INFINITY), Color::default(), 0.0).as_f32s(),
        );
        assert!(params_vec.len() < 2usize.pow(16));
        if params.len() > 0 {
            let mut per_tile_indices = vec![vec![]; PANEL_WIDTH * PANEL_HEIGHT];
            for (e, inds) in indices {
                for i in inds {
                    per_tile_indices[i].push(params[&e] as u32);
                }
            }
            let mut per_tile_counts = Vec::with_capacity(PANEL_WIDTH * PANEL_HEIGHT * 2);
            let mut offset = 0;
            let indices: Vec<_> = per_tile_indices
                .into_iter()
                .flat_map(|inds| {
                    per_tile_counts.push(inds.len() as u32);
                    per_tile_counts.push(offset as u32);
                    offset += inds.len();
                    inds
                })
                .collect();

            functions.functions = params_vec;
            functions.tile_function_count = per_tile_counts;
            functions.function_indices = indices;
        }
    }
}

fn draw(
    mut context: DrawContext,
    mut render_resource_bindings: ResMut<RenderResourceBindings>,
    functions: Res<Assets<SDFFunctions>>,
    pipeline_handle: Res<SDFPipeline>,
    mut panel_query: Query<(&mut Draw, &Handle<SDFFunctions>), With<SDFPanel>>,
) {
    if let Some((mut draw, handle)) = panel_query.iter_mut().next() {
        if functions
            .get(handle)
            .map(|f| f.functions.is_empty())
            .unwrap_or(true)
        {
            return;
        }
        context
            .set_pipeline(
                &mut draw,
                &pipeline_handle.0,
                &PipelineSpecialization::default(),
            )
            .unwrap();

        context
            .set_bind_groups_from_bindings(&mut draw, &mut [&mut render_resource_bindings])
            .unwrap();
        context.set_asset_bind_groups(&mut draw, &handle).unwrap();

        draw.draw(0..(PANEL_WIDTH * PANEL_HEIGHT * 6) as u32, 0..1);
    }
}
