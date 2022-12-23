struct Vertex {
    //position
    position_x: f32,
    position_y: f32,
    position_z: f32,
    //normal
    normal_x: f32,
    normal_y: f32,
    normal_z: f32,
    //tangent
    tangent_x: f32,
    tangent_y: f32,
    tangent_z: f32,
    //tex_coords
    tex_coords_x: f32,
    tex_coords_y: f32,
}

struct Velocity {
    velocity_x: f32,
    velocity_y: f32,
    velocity_z: f32,
}

struct ComputationData {
    delta_time: f32,
    nb_cloth_vertices: u32,
    //gravity
    cloth_vertex_mass: f32,
    gravity: f32,
    //springs
    structural_stiffness: f32,
    shear_stiffness: f32,
    bend_stiffness: f32,
    //collisions
    sphere_radius: f32,
    sphere_offset: f32,
}

@group(0) @binding(0) var<storage, read_write> verticesData: array<Vertex>;
@group(1) @binding(0) var<storage, read_write> velocitiesData: array<Velocity>;
@group(2) @binding(0) var<uniform> data: ComputationData;

@compute @workgroup_size(64, 1, 1) 
fn main(@builtin(global_invocation_id) param: vec3<u32>) {
    if (param.x >= u32(data.nb_cloth_vertices)) {
          return;
    }

    //displacement
    verticesData[param.x].position_x += velocitiesData[param.x].velocity_x * data.delta_time;
    verticesData[param.x].position_y += velocitiesData[param.x].velocity_y * data.delta_time;
    verticesData[param.x].position_z += velocitiesData[param.x].velocity_z * data.delta_time;

    //gravity
    velocitiesData[param.x].velocity_y -= data.gravity * data.delta_time;
}
