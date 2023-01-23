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

struct Spring {
    vertex1: f32,
    vertex2: f32,
    rest_length: f32,
    stiffness: f32,
}

struct ComputeData {
    delta_time: f32,
    nb_cloth_vertices: f32,
    nb_cloth_springs: f32,
    //gravity
    cloth_vertex_mass: f32,
    gravity: f32,
    //springs
    structural_stiffness: f32,
    shear_stiffness: f32,
    bend_stiffness: f32,
    //collisions
    sphere_radius: f32,
    sphere_position_x: f32,
    sphere_position_y: f32,
    sphere_position_z: f32,
}

@group(0) @binding(0) var<storage, read_write> verticesData: array<Vertex>;
@group(1) @binding(0) var<storage, read_write> velocitiesData: array<Velocity>;
@group(2) @binding(0) var<uniform> data: ComputeData;
@group(3) @binding(0) var<storage, read> springsData: array<Spring>;

@compute @workgroup_size(128, 1, 1)
fn main(@builtin(global_invocation_id) param: vec3<u32>) {
    if (param.x >= u32(data.nb_cloth_springs)) {
          return;
    }
    var spring_force = vec3<f32>(0.0, 0.0, 0.0);
    let spring = springsData[param.x];
    let vertex_index_1 = u32(spring.vertex1);
    let vertex_index_2 = u32(spring.vertex2);
    let rest_length = spring.rest_length;

    // calculate the distance between the two vertices
    let position_1 = vec3<f32>(verticesData[vertex_index_1].position_x, verticesData[vertex_index_1].position_y, verticesData[vertex_index_1].position_z);
    let position_2 = vec3<f32>(verticesData[vertex_index_2].position_x, verticesData[vertex_index_2].position_y, verticesData[vertex_index_2].position_z);
    var distance = length(position_1 - position_2);
    var direction = normalize(position_1 - position_2);

    // calculate the speed of the first vertex relative to the second
    let velocity_1 = vec3<f32>(velocitiesData[vertex_index_1].velocity_x, velocitiesData[vertex_index_1].velocity_y, velocitiesData[vertex_index_1].velocity_z);
    let velocity_2 = vec3<f32>(velocitiesData[vertex_index_2].velocity_x, velocitiesData[vertex_index_2].velocity_y, velocitiesData[vertex_index_2].velocity_z);
    let relative_velocity = length(velocity_1 - velocity_2);
    let velocity_direction = normalize(velocity_1 - velocity_2);
    

    spring_force += -spring.stiffness * (distance - rest_length) * direction;
    if relative_velocity != 0.0 {
        let damping_force = -4.0 * relative_velocity;
        spring_force += damping_force * velocity_direction;
    }
    let gravity_force = -data.gravity * data.cloth_vertex_mass;

    // update the velocity of the vertex
    velocitiesData[vertex_index_1].velocity_x += (spring_force.x / data.cloth_vertex_mass) * data.delta_time;
    velocitiesData[vertex_index_1].velocity_y += (spring_force.y + gravity_force / data.cloth_vertex_mass) * data.delta_time;
    velocitiesData[vertex_index_1].velocity_z += (spring_force.z/ data.cloth_vertex_mass) * data.delta_time;

    velocitiesData[vertex_index_2].velocity_x -= (spring_force.x / data.cloth_vertex_mass) * data.delta_time;
    velocitiesData[vertex_index_2].velocity_y -= (spring_force.y - gravity_force / data.cloth_vertex_mass) * data.delta_time;
    velocitiesData[vertex_index_2].velocity_z -= (spring_force.z / data.cloth_vertex_mass) * data.delta_time;

    // update the position of the vertex, or it crashes
    verticesData[vertex_index_1].position_x += 0.0;
}
