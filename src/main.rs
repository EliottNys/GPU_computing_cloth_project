use wgpu_bootstrap::{
    window::Window,
    frame::Frame,
    application::Application,
    texture::create_texture_bind_group,
    context::Context,
    camera::Camera,
    computation::Computation,
    wgpu,
    cgmath::{ self, prelude::* },
    default::{Vertex},
    geometry::icosphere,
};
use std::{f64, slice::SplitInclusive};
    
    
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct VertexVelocity {
    velocity: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Spring {
    vertex1: f32,
    vertex2: f32,
    rest_length: f32,
    stiffness: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ComputeData {
    delta_time: f32,
    nb_cloth_vertices: f32,
    nb_cloth_springs:f32,
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

// --------   PARAMETERS OF THE SIMULATION   --------
// ==================================================
const GRAVITY: f32 = 0.0981;
//CLOTH
const CLOTH_WIDTH: u32 = 30;
const NB_CLOTH_VERTICES: u32 = CLOTH_WIDTH * CLOTH_WIDTH;
const NB_CLOTH_SPRINGS: f32 = (6 * CLOTH_WIDTH.pow(2) - 10 * CLOTH_WIDTH + 2) as f32;
// const NB_CLOTH_SPRINGS: f32 = (2 * CLOTH_WIDTH.pow(2) - 2 * CLOTH_WIDTH) as f32; //only structural springs
const CLOTH_VERTEX_MASS: f32 = 0.5;
const CLOTH_FALL_HEIGHT: f32 = (CLOTH_WIDTH as f32) / 3.0;
// const STRUCTURAL_STIFFNESS: f32 = 300.0;
// const SHEAR_STIFFNESS: f32 = 4.0;
// const BEND_STIFFNESS: f32 = 2.0;
const STRUCTURAL_STIFFNESS: f32 = 200.0;
const SHEAR_STIFFNESS: f32 = 140.0;
const BEND_STIFFNESS: f32 = 70.0;
//SPHERE
const SPHERE_RADIUS: f32 = (CLOTH_WIDTH as f32) / 7.0;
const SPHERE_POSITION_X: f32 = 0.0;
const SPHERE_POSITION_Y: f32 = 0.0;
const SPHERE_POSITION_Z: f32 = 0.0;
// ==================================================

fn create_cloth_mesh(width: u16, altitude: f32) -> (Vec<Vertex>, Vec<u16>, Vec<VertexVelocity>, Vec<Spring>) {       //creates a cloth mesh of vertices of width x width

    // VERTICES
    let mut vertices = Vec::new();

    let height = width;

    for z in 0..height {
        for x in 0..width {
            let position = [((x as f32) - (width / 2) as f32), altitude, ((z as f32) - (width / 2) as f32)];
            let normal = [0.0, 1.0, 0.0];
            let tangent = [1.0, 0.0, 1.0];
            let tex_coords = [(x as f32) / (width - 1) as f32, (z as f32) / (height - 1) as f32];
            vertices.push(Vertex { position, normal, tangent, tex_coords });
        }
    }

    //INDICES (triangles)
    let mut indices = Vec::new();

    for z in 0..height - 1 {
        for x in 0..width - 1 {
            let v0 = z * width + x;
            let v1 = z * width + x + 1;
            let v2 = (z + 1) * width + x;
            let v3 = (z + 1) * width + x + 1;
            indices.extend_from_slice(&[v0, v1, v2, v1, v3, v2]);
            indices.extend_from_slice(&[v0, v2, v1, v1, v2, v3]);
        }
    }

    //VELOCITIES
    let mut velocities = Vec::new();
    for vertex in vertices.iter_mut() {
        velocities.push(VertexVelocity {velocity: [0.0, 0.0, 0.0]})
    }

    //SPRINGS
    let mut springs = Vec::new();
    for i in 0..NB_CLOTH_VERTICES {
        //- structural
        if i + 1 < NB_CLOTH_VERTICES && ((i+1) % CLOTH_WIDTH) != 0  {   //exclude right
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+1) as f32, rest_length: 1.0, stiffness: STRUCTURAL_STIFFNESS }  //horizontal
            );
        }
        if i + CLOTH_WIDTH < NB_CLOTH_VERTICES {    //exclude bottom
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+CLOTH_WIDTH) as f32, rest_length: 1.0, stiffness: STRUCTURAL_STIFFNESS }    //vertical
            );
        }
        //- shear
        if (i % CLOTH_WIDTH) != 0 && (i + CLOTH_WIDTH) < NB_CLOTH_VERTICES {  //exclude left and bottom
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+CLOTH_WIDTH-1) as f32, rest_length: 1.41421356, stiffness: SHEAR_STIFFNESS }    //diagonal NE-SW ; sqrt(2) = 1.41421356
            );
        }
        if ((i+1) % CLOTH_WIDTH) != 0 && (i + CLOTH_WIDTH) < NB_CLOTH_VERTICES {  //exclude right and bottom
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+CLOTH_WIDTH+1) as f32, rest_length: 1.41421356, stiffness: SHEAR_STIFFNESS }    //diagonal NW-SE ; sqrt(2) = 1.41421356
            );
        }
        //- bend
        if (i + 2*CLOTH_WIDTH) < NB_CLOTH_VERTICES {    //exclude two bottom rows
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+2*CLOTH_WIDTH) as f32, rest_length: 2.0, stiffness: BEND_STIFFNESS }    //vertical long
            );
        }
        if ((i + 1) % CLOTH_WIDTH) != 0 && ((i + 2) % CLOTH_WIDTH) != 0 {    //exclude two far-right columns
            springs.push(
                Spring { vertex1: i as f32, vertex2: (i+2) as f32, rest_length: 2.0, stiffness: BEND_STIFFNESS }    //horizontal long
            );
        }
    }
    // println!("number of springs: {}", springs.len());
    (vertices, indices, velocities, springs)
}

struct MyApp {
    camera_bind_group: wgpu::BindGroup,
    //cloth
    cloth_diffuse_bind_group: wgpu::BindGroup,
    cloth_pipeline: wgpu::RenderPipeline,
    cloth_vertex_buffer: wgpu::Buffer,
    cloth_index_buffer: wgpu::Buffer,
    nb_cloth_indices: usize,
    //compute
    compute_pipeline: wgpu::ComputePipeline,
    compute_springs_pipeline: wgpu::ComputePipeline,
    compute_vertices_bind_group: wgpu::BindGroup,
    compute_vertex_velocities_bind_group: wgpu::BindGroup,
    compute_springs_bind_group: wgpu::BindGroup,
    compute_data_bind_group: wgpu::BindGroup,
    compute_data_buffer: wgpu::Buffer,
    compute_data: ComputeData,
    //sphere
    sphere_diffuse_bind_group: wgpu::BindGroup,
    sphere_pipeline: wgpu::RenderPipeline,
    sphere_vertex_buffer: wgpu::Buffer,
    sphere_index_buffer: wgpu::Buffer,
    nb_sphere_indices: usize,
}

impl MyApp {
    fn new(context: &Context) -> Self {
        let camera = Camera {
            eye: (1.5 * (CLOTH_WIDTH as f32), 0.0, 0.0).into(),
            //eye: (10.0, -15.0, 10.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: context.get_aspect_ratio(),
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };


        let (_camera_buffer, camera_bind_group) = camera.create_camera_bind_group(context);

        //----- CLOTH -----
        let cloth_texture = context.create_srgb_texture("cloth.jpg", include_bytes!("cloth.jpg"));
        let cloth_diffuse_bind_group = create_texture_bind_group(context, &cloth_texture);

        let cloth_pipeline = context.create_render_pipeline(
            "Cloth Render Pipeline",
            include_str!("cloth_shader.wgsl"),
            &[Vertex::desc()],
            &[
                &context.texture_bind_group_layout,
                &context.camera_bind_group_layout,
            ],
            wgpu::PrimitiveTopology::TriangleList
        );

        let (cloth_vertices, cloth_indices, cloth_vertices_velocities, cloth_springs) = create_cloth_mesh((CLOTH_WIDTH) as u16, CLOTH_FALL_HEIGHT);

        let cloth_vertex_buffer = context.create_buffer(&cloth_vertices, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE);
        let cloth_index_buffer = context.create_buffer(&cloth_indices, wgpu::BufferUsages::INDEX);
        let cloth_vertex_velocity_buffer = context.create_buffer(&cloth_vertices_velocities, wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE);
        let cloth_spring_buffer = context.create_buffer(&cloth_springs.as_slice(), wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::UNIFORM);

        //----- COMPUTE -----
        let compute_pipeline = context.create_compute_pipeline("Compute Pipeline", include_str!("compute.wgsl"));
        let compute_springs_pipeline = context.create_compute_pipeline("Compute Springs Pipeline", include_str!("compute_springs.wgsl"));

        println!("{}",NB_CLOTH_SPRINGS);
        let compute_data = ComputeData {
            delta_time: 0.016,
            nb_cloth_vertices: NB_CLOTH_VERTICES as f32,
            nb_cloth_springs: NB_CLOTH_SPRINGS,
            //gravity
            cloth_vertex_mass: CLOTH_VERTEX_MASS,
            gravity: GRAVITY,
            //springs
            structural_stiffness: STRUCTURAL_STIFFNESS,
            shear_stiffness: SHEAR_STIFFNESS,
            bend_stiffness: BEND_STIFFNESS,
            //collisions
            sphere_radius: SPHERE_RADIUS * 1.05,
            sphere_position_x: SPHERE_POSITION_X,
            sphere_position_y: SPHERE_POSITION_Y,
            sphere_position_z: SPHERE_POSITION_Z,
        };

        let compute_data_buffer = context.create_buffer(&[compute_data], wgpu::BufferUsages::UNIFORM);

        let compute_vertices_bind_group = context.create_bind_group(
            "Compute Vertices Bind Group",
            &compute_pipeline.get_bind_group_layout(0),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloth_vertex_buffer.as_entire_binding(),
                },
            ],
        );

        let compute_vertex_velocities_bind_group = context.create_bind_group(
            "Compute Vertices Velocities Bind Group",
            &compute_pipeline.get_bind_group_layout(1),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloth_vertex_velocity_buffer.as_entire_binding(),
                },
            ],
        );

        let compute_springs_bind_group = context.create_bind_group(
            "Compute Springs Bind Group",
            &compute_springs_pipeline.get_bind_group_layout(3),
            &[
                wgpu::BindGroupEntry {
                    binding:0,
                    resource: cloth_spring_buffer.as_entire_binding(),
                },
            ],
        );

        let compute_data_bind_group = context.create_bind_group(
            "Compute Data Bind Group",
            &compute_pipeline.get_bind_group_layout(2),
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: compute_data_buffer.as_entire_binding(),
                },
            ],
        );

        //----- SPHERE -----
        let sphere_texture = context.create_srgb_texture("bowling_ball.png", include_bytes!("bowling_ball.png"));
        let sphere_diffuse_bind_group = create_texture_bind_group(context, &sphere_texture);

        let sphere_pipeline = context.create_render_pipeline(
            "Sphere Render Pipeline",
            include_str!("sphere_shader.wgsl"),
            &[Vertex::desc()],
            &[
                &context.texture_bind_group_layout,
                &context.camera_bind_group_layout
            ],
            wgpu::PrimitiveTopology::TriangleList
        );

        let (mut sphere_vertices, sphere_indices) = icosphere(4);

        // change the size of the sphere
        for vertex in sphere_vertices.iter_mut() {
            let mut posn = cgmath::Vector3::from(vertex.position);
            posn *= SPHERE_RADIUS as f32;
            vertex.position = posn.into()
        }
        
        let sphere_vertex_buffer = context.create_buffer(&sphere_vertices, wgpu::BufferUsages::VERTEX);
        let sphere_index_buffer = context.create_buffer(&sphere_indices, wgpu::BufferUsages::INDEX);


        Self {
            camera_bind_group,
            //cloth
            cloth_diffuse_bind_group,
            cloth_pipeline,
            cloth_vertex_buffer,
            cloth_index_buffer,
            nb_cloth_indices: cloth_indices.len(),
            //compute
            compute_pipeline,
            compute_springs_pipeline,
            compute_vertices_bind_group,
            compute_vertex_velocities_bind_group,
            compute_springs_bind_group,
            compute_data_bind_group,
            compute_data_buffer,
            compute_data,
            //sphere
            sphere_diffuse_bind_group,
            sphere_pipeline,
            sphere_vertex_buffer,
            sphere_index_buffer,
            nb_sphere_indices: sphere_indices.len(),
        }
    }
}


impl Application for MyApp {
    fn render(&self, context: &Context) -> Result<(), wgpu::SurfaceError> {
        let mut frame = Frame::new(context)?;

        {
            let mut render_pass = frame.begin_render_pass(wgpu::Color {r: 0.25, g: 0.25, b: 0.35, a: 1.0});

            //sphere
            render_pass.set_pipeline(&self.sphere_pipeline);
            render_pass.set_bind_group(0, &self.sphere_diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.sphere_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.sphere_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..(self.nb_sphere_indices as u32), 0, 0..1);

            //cloth
            render_pass.set_pipeline(&self.cloth_pipeline);
            render_pass.set_bind_group(0, &self.cloth_diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cloth_vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.cloth_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..(self.nb_cloth_indices as u32), 0, 0..1);
        }


            frame.present();


        Ok(())
    }


    fn update(&mut self, context: &Context, delta_time: f32) {
        // Update the Buffer that contains the delta_time
        let compute_data = ComputeData {
            delta_time,
            nb_cloth_vertices: NB_CLOTH_VERTICES as f32,
            nb_cloth_springs: NB_CLOTH_SPRINGS,
            //gravity
            cloth_vertex_mass: CLOTH_VERTEX_MASS,
            gravity: GRAVITY,
            //springs
            structural_stiffness: STRUCTURAL_STIFFNESS,
            shear_stiffness: SHEAR_STIFFNESS,
            bend_stiffness: BEND_STIFFNESS,
            //collisions
            sphere_radius: SPHERE_RADIUS * 1.15,
            sphere_position_x: SPHERE_POSITION_X,
            sphere_position_y: SPHERE_POSITION_Y,
            sphere_position_z: SPHERE_POSITION_Z,
        }; 
        context.update_buffer(&self.compute_data_buffer, &[compute_data]);


        let mut computation = Computation::new(context);


        {
            let mut compute_pass = computation.begin_compute_pass();

            compute_pass.set_pipeline(&self.compute_springs_pipeline);
            compute_pass.set_bind_group(0, &self.compute_vertices_bind_group, &[]);
            compute_pass.set_bind_group(1, &self.compute_vertex_velocities_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.compute_data_bind_group, &[]);
            compute_pass.set_bind_group(3, &self.compute_springs_bind_group, &[]);
            compute_pass.dispatch_workgroups(((NB_CLOTH_SPRINGS) as f64/64.0).ceil() as u32, 1, 1);

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(2, &self.compute_data_bind_group, &[]);
            compute_pass.dispatch_workgroups(((NB_CLOTH_VERTICES) as f64/64.0).ceil() as u32, 1, 1);
        }

        computation.submit();
    }
}


fn main() {
    let window = Window::new();


    let context = window.get_context();


    let my_app = MyApp::new(context);


    window.run(my_app);
}