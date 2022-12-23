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
use std::f64;
    
    
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ComputeData {
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

const CLOTH_WIDTH: u32 = 10;
const NB_CLOTH_VERTICES: u32 = CLOTH_WIDTH * CLOTH_WIDTH;
const CLOTH_VERTEX_MASS: f32 = 5.0;
const CLOTH_FALL_HEIGHT: f32 = 3.5;
const SPHERE_RADIUS: f32 = (CLOTH_WIDTH as f32) / 8.5;
const SPHERE_OFFSET: f32 = (CLOTH_WIDTH as f32) / 2.0;

fn create_cloth_mesh(width: u16, altitude: f32) -> (Vec<Vertex>, Vec<u16>) {       //creates a cloth mesh of vertices of width x width
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

    let mut indices = Vec::new();

    for z in 0..height - 1 {
        for x in 0..width - 1 {
            let v0 = z * width + x;
            let v1 = z * width + x + 1;
            let v2 = (z + 1) * width + x;
            let v3 = (z + 1) * width + x + 1;
            indices.extend_from_slice(&[v0, v1, v2, v1, v3, v2]);
        }
    }

    (vertices, indices)
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
    compute_data_buffer: wgpu::Buffer,
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
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: context.get_aspect_ratio(),
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };


        let (_camera_buffer, camera_bind_group) = camera.create_camera_bind_group(context);

        //----- COMPUTE -----
        let compute_pipeline = context.create_compute_pipeline("Compute Pipeline", include_str!("compute.wgsl"));


        let compute_data = ComputeData {
            delta_time: 0.016,
            nb_cloth_vertices: NB_CLOTH_VERTICES,
            //gravity
            cloth_vertex_mass: CLOTH_VERTEX_MASS,
            gravity: 9.81,
            //springs
            structural_stiffness: 5.0,
            shear_stiffness: 4.0,
            bend_stiffness: 2.0,
            //collisions
            sphere_radius: SPHERE_RADIUS,
            sphere_offset: SPHERE_OFFSET,
        };

        let compute_data_buffer = context.create_buffer(&[compute_data], wgpu::BufferUsages::UNIFORM);

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

        let (cloth_vertices, cloth_indices) = create_cloth_mesh((CLOTH_WIDTH) as u16, CLOTH_FALL_HEIGHT);

        let cloth_vertex_buffer = context.create_buffer(&cloth_vertices, wgpu::BufferUsages::VERTEX);
        let cloth_index_buffer = context.create_buffer(&cloth_indices, wgpu::BufferUsages::INDEX);

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
            compute_data_buffer,
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
            let mut render_pass = frame.begin_render_pass(wgpu::Color {r: 1.0, g: 1.0, b: 1.0, a: 1.0});

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
        // // Update the Buffer that contains the delta_time
        // let compute_data = ComputeData {
        //     delta_time,
        //     nb_vertices: 100,
        //     vertex_mass: 0.5,
        //     gravity: 9.81,
        //     structural_stiffness: 5.0,
        //     shear_stiffness: 4.0,
        //     bend_stiffness: 2.0,
        // }; 
        // context.update_buffer(&self.compute_data_buffer, &[compute_data]);


        // let mut computation = Computation::new(context);


        // {
        //     let mut compute_pass = computation.begin_compute_pass();
        //     compute_pass.set_pipeline(&self.compute_pipeline);
        // }


        // computation.submit();
    }
}


fn main() {
    let window = Window::new();


    let context = window.get_context();


    let my_app = MyApp::new(context);


    window.run(my_app);
}