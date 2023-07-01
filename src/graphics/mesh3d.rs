use crate::{
    graphics::{self, Canvas3d, DrawParam3d, Image},
    Context,
};

#[cfg(feature = "gltf")]
use crate::{GameError, GameResult};
#[cfg(feature = "gltf")]
use base64::Engine;
#[cfg(feature = "gltf")]
use image::EncodableLayout;
#[cfg(feature = "obj")]
use num_traits::FromPrimitive;
#[cfg(feature = "gltf")]
use std::path::Path;

use glam::{Mat4, Vec3};
use mint::{Vector2, Vector3};
use std::{mem, sync::Arc};
use wgpu::{util::DeviceExt, vertex_attr_array};

// Implementation tooken from bevy
/// An aabb stands for axis aligned bounding box. This is basically a cube that can't rotate.
#[derive(Debug, Copy, Clone)]
pub struct Aabb {
    /// The center of this `Aabb`
    pub center: mint::Vector3<f32>,
    /// The half_extents or half the size of this `Aabb` for each axis
    pub half_extents: mint::Vector3<f32>,
}

impl Default for Aabb {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO.into(),
            half_extents: Vec3::ZERO.into(),
        }
    }
}

impl Aabb {
    /// Create an `Aabb` from a minimum point and a maximum point
    #[inline]
    pub fn from_min_max(minimum: Vec3, maximum: Vec3) -> Self {
        let minimum = minimum;
        let maximum = maximum;
        let center = 0.5 * (maximum + minimum);
        let half_extents = 0.5 * (maximum - minimum);
        Self {
            center: center.into(),
            half_extents: half_extents.into(),
        }
    }
}

/// Transform3d is used to transform 3d objects.
#[derive(Debug, Copy, Clone)]
pub struct Transform3d {
    /// The position or translation of this `Transform3d`
    pub position: mint::Vector3<f32>,
    /// The rotation of this `Transform3d`
    pub rotation: mint::Quaternion<f32>,
    /// The scale of this `Transform3d`
    pub scale: mint::Vector3<f32>,
}

impl Default for Transform3d {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 0.0).into(),
            rotation: glam::Quat::IDENTITY.into(),
            scale: Vec3::new(1.0, 1.0, 1.0).into(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance3d {
    transform: [[f32; 4]; 4],
    color: [f32; 4],
}

impl Default for Instance3d {
    fn default() -> Self {
        Self::from_param(&DrawParam3d::default(), Vec3::ZERO)
    }
}

impl Instance3d {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBS: [wgpu::VertexAttribute; 5] = vertex_attr_array![
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4
        ];
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance3d>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRIBS,
        }
    }
    pub(crate) fn from_param<V>(param: &DrawParam3d, center: V) -> Self
    where
        V: Into<mint::Vector3<f32>>,
    {
        let offset: mint::Vector3<f32> = center.into();
        let pivot = if let Some(piv) = param.pivot {
            Vec3::from(piv) + Vec3::from(offset)
        } else {
            Vec3::from(param.transform.position) + Vec3::from(offset)
        };
        let transform = Mat4::from_translation(pivot)
            * Mat4::from_scale(param.transform.scale.into())
            * Mat4::from_quat(param.transform.rotation.into())
            * Mat4::from_translation(-(pivot))
            * Mat4::from_translation(Vec3::from(param.transform.position));

        Self {
            transform: transform.to_cols_array_2d(),
            color: param.color.into(),
        }
    }
}

// TODO: Allow custom vertex formats
/// The 3d Vertex format. Used for constructing meshes. At the moment it supports color, position, and texture coords
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod, Debug)]
#[repr(C)]
pub struct Vertex3d {
    /// The position of this vertex
    pub pos: [f32; 3],
    /// The texture uv of this vertex
    pub tex_coord: [f32; 2],
    /// The color of this vertex
    pub color: [f32; 4],
}

impl Vertex3d {
    /// Create a new vertex from a position, uv, and color
    pub fn new<V, T, C>(position: V, uv: T, color: C) -> Vertex3d
    where
        V: Into<Vector3<f32>>,
        T: Into<Vector2<f32>>,
        C: Into<Option<graphics::Color>>,
    {
        let position: Vector3<f32> = position.into();
        let uv: Vector2<f32> = uv.into();
        let color: Option<graphics::Color> = color.into();
        let color = color
            .unwrap_or(graphics::Color::new(1.0, 1.0, 1.0, 0.0))
            .into();
        Vertex3d {
            pos: position.into(),
            tex_coord: uv.into(),
            color,
        }
    }

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBS: [wgpu::VertexAttribute; 3] = vertex_attr_array![
            0 => Float32x3,
            1 => Float32x2,
            2 => Float32x4,
        ];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex3d>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBS,
        }
    }
}

/// A struct to help create `Mesh3d`
#[derive(Clone, Debug, Default)]
pub struct Mesh3dBuilder {
    /// Vector of the vertices that make up the mesh
    pub vertices: Vec<Vertex3d>,
    /// Vector of the indices used to index into the vertices of the mesh
    pub indices: Vec<u32>,
    /// The texture of the Mesh if any
    pub texture: Option<Image>,
}

impl Mesh3dBuilder {
    /// Create an empty `Mesh3dBuilder`
    pub fn new() -> Self {
        Self {
            vertices: Vec::default(),
            indices: Vec::default(),
            texture: None,
        }
    }

    /// Add data that makes up a mesh.
    pub fn from_data(
        &mut self,
        vertices: Vec<Vertex3d>,
        indices: Vec<u32>,
        texture: Option<Image>,
    ) -> &mut Self {
        self.vertices = vertices;
        self.indices = indices;
        self.texture = texture;
        self
    }

    /// Make a `Mesh3d` from this builder
    pub fn build(&self, ctx: &mut Context) -> Mesh3d {
        let verts = ctx
            .gfx
            .wgpu()
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(self.vertices.as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let inds = ctx
            .gfx
            .wgpu()
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(self.indices.as_slice()),
                usage: wgpu::BufferUsages::INDEX,
            });
        Mesh3d {
            vert_buffer: Some(Arc::new(verts)),
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            ind_buffer: Some(Arc::new(inds)),
            bind_group: None,
            texture: self.texture.clone(),
        }
    }
}

/// A 3d Mesh that can be rendered to `Canvas3d`
#[derive(Clone, Debug)]
pub struct Mesh3d {
    pub(crate) vert_buffer: Option<Arc<wgpu::Buffer>>,
    pub(crate) ind_buffer: Option<Arc<wgpu::Buffer>>,
    pub(crate) bind_group: Option<Arc<wgpu::BindGroup>>,
    /// The texture of this Mesh if any
    pub texture: Option<Image>,
    /// Vector of the vertices that make up this mesh
    pub vertices: Vec<Vertex3d>,
    /// Vector of the indices used to index into the vertices of this mesh
    pub indices: Vec<u32>,
}

impl Mesh3d {
    pub(crate) fn gen_bind_group(
        &mut self,
        canvas: &Canvas3d,
        pipeline_id: usize,
        sampler: graphics::Sampler,
    ) {
        // Allow custom one set through mesh
        let sampler = canvas.wgpu.device.create_sampler(&sampler.into());

        let bind_group = canvas
            .wgpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &canvas.pipelines[pipeline_id].0.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            self.texture
                                .as_ref()
                                .unwrap_or(&canvas.default_image)
                                .wgpu()
                                .1,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

        self.bind_group = Some(Arc::new(bind_group));
    }

    /// Get the bounding box of this mesh
    pub fn to_aabb(&self) -> Option<Aabb> {
        let mut minimum = Vec3::MAX;
        let mut maximum = Vec3::MIN;
        for p in self.vertices.iter() {
            minimum = minimum.min(Vec3::from_array(p.pos));
            maximum = maximum.max(Vec3::from_array(p.pos));
        }
        if minimum.x != std::f32::MAX
            && minimum.y != std::f32::MAX
            && minimum.z != std::f32::MAX
            && maximum.x != std::f32::MIN
            && maximum.y != std::f32::MIN
            && maximum.z != std::f32::MIN
        {
            Some(Aabb::from_min_max(minimum, maximum))
        } else {
            None
        }
    }
}

#[cfg(feature = "gltf")]
// This is needed to handle ascii gltf files
struct DataUri<'a> {
    mime_type: &'a str,
    base64: bool,
    data: &'a str,
}

#[cfg(feature = "gltf")]
fn split_once(input: &str, delimiter: char) -> Option<(&str, &str)> {
    let mut iter = input.splitn(2, delimiter);
    Some((iter.next()?, iter.next()?))
}

#[cfg(feature = "gltf")]
impl<'a> DataUri<'a> {
    fn parse(uri: &'a str) -> Result<DataUri<'a>, ()> {
        let uri = uri.strip_prefix("data:").ok_or(())?;
        let (mime_type, data) = split_once(uri, ',').ok_or(())?;

        let (mime_type, base64) = match mime_type.strip_suffix(";base64") {
            Some(mime_type) => (mime_type, true),
            None => (mime_type, false),
        };

        Ok(DataUri {
            mime_type,
            base64,
            data,
        })
    }

    fn decode(&self) -> GameResult<Vec<u8>> {
        if self.base64 {
            if let Ok(vec) = base64::engine::general_purpose::STANDARD_NO_PAD.decode(self.data) {
                Ok(vec)
            } else {
                Err(GameError::CustomError(
                    "Failed to decode base64".to_string(),
                ))
            }
        } else {
            Ok(self.data.as_bytes().to_owned())
        }
    }
}

#[cfg(feature = "obj")]
impl<I: FromPrimitive> obj::FromRawVertex<I> for Vertex3d {
    fn process(
        vertices: Vec<(f32, f32, f32, f32)>,
        _normals: Vec<(f32, f32, f32)>,
        tex_coords: Vec<(f32, f32, f32)>,
        polygons: Vec<obj::raw::object::Polygon>,
    ) -> obj::ObjResult<(Vec<Self>, Vec<I>)> {
        let verts = if vertices.len() == tex_coords.len() {
            std::iter::zip(vertices, tex_coords)
                .map(|v| {
                    println!("{:?}", v);
                    Vertex3d {
                        pos: [v.0 .0, v.0 .1, v.0 .2],
                        // tex_coord: [0.0, 0.0],
                        tex_coord: [v.1 .0, v.1 .1],
                        color: [1.0, 1.0, 1.0, 1.0],
                    }
                })
                .collect()
        } else {
            vertices
                .iter()
                .map(|v| Vertex3d {
                    pos: [v.0, v.1, v.2],
                    tex_coord: [0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0],
                })
                .collect()
        };
        let mut inds = Vec::with_capacity(polygons.len() * 3);
        {
            let mut map = |pi: usize| -> obj::ObjResult<()> {
                inds.push(match I::from_usize(pi) {
                    Some(val) => val,
                    None => {
                        return obj::ObjResult::Err(obj::ObjError::Load(obj::LoadError::new(
                            obj::LoadErrorKind::IndexOutOfRange,
                            "Unable to convert the index from usize",
                        )));
                    }
                });
                Ok(())
            };

            for polygon in polygons {
                match polygon {
                    obj::raw::object::Polygon::P(ref vec) if vec.len() == 3 => {
                        for &pi in vec {
                            map(pi)?
                        }
                    }
                    obj::raw::object::Polygon::PT(ref vec)
                    | obj::raw::object::Polygon::PN(ref vec)
                        if vec.len() == 3 =>
                    {
                        for &(pi, _) in vec {
                            map(pi)?
                        }
                    }
                    obj::raw::object::Polygon::PTN(ref vec) if vec.len() == 3 => {
                        for &(pi, _, _) in vec {
                            map(pi)?
                        }
                    }
                    _ => {
                        return Err(obj::ObjError::Load(obj::LoadError::new(
                            obj::LoadErrorKind::UntriangulatedModel,
                            "Meshes must be triangulated",
                        )))
                    }
                }
            }
        }
        Ok((verts, inds))
    }
}

/// Model is an abstracted type for holding things like obj, gltf, or anything that may be made up of multiple meshes.
#[derive(Debug, Default)]
pub struct Model {
    /// The center of the model
    pub center: Option<Vec3>,
    /// The `Transform3d` of the model
    pub transform: Transform3d,
    /// The meshes that make up the model
    pub meshes: Vec<Mesh3d>,
}

impl Model {
    /// Load gltf or obj depending on extension type
    #[cfg(all(feature = "obj", feature = "gltf"))]
    pub fn from_path(
        ctx: &mut Context,
        path: impl AsRef<Path>,
        image: impl Into<Option<Image>>,
    ) -> GameResult<Self> {
        if let Some(extension) = path.as_ref().extension() {
            if extension == "obj" {
                Model::from_obj(ctx, path, image)
            } else if extension == "gltf" || extension == "glb" {
                Model::from_gltf(ctx, path)
            } else {
                Err(GameError::CustomError("Not a obj or gltf file".to_string()))
            }
        } else {
            Err(GameError::CustomError(
                "Failed to get extension".to_string(),
            ))
        }
    }

    /// Load obj file. Only triangulated obj's are supported. Keep in mind mtl file's currently don't affect the obj's rendering
    #[cfg(feature = "obj")]
    pub fn from_obj(
        ctx: &mut Context,
        path: impl AsRef<Path>,
        image: impl Into<Option<Image>>,
    ) -> GameResult<Self> {
        let file = ctx.fs.open(path)?;
        let buf_reader = std::io::BufReader::new(file);
        match obj::load_obj(buf_reader) {
            Ok(obj) => {
                let mut img = Image::from_color(ctx, 1, 1, Some(graphics::Color::WHITE));
                let image: Option<Image> = image.into();
                if let Some(image) = image {
                    img = image;
                }
                let mesh = Mesh3dBuilder::new()
                    .from_data(obj.vertices, obj.indices, Some(img))
                    .build(ctx);
                let mut model = Model {
                    center: None,
                    transform: Transform3d::default(),
                    meshes: vec![mesh],
                };

                model.center = Some(model.to_aabb().unwrap_or_default().center.into());
                Ok(model)
            }
            Err(f) => Err(GameError::CustomError(f.to_string())),
        }
    }
    /// Load gltf file.
    #[cfg(feature = "gltf")]
    pub fn from_gltf(ctx: &mut Context, path: impl AsRef<Path>) -> GameResult<Self> {
        const VALID_MIME_TYPES: &[&str] = &["application/octet-stream", "application/gltf-buffer"];
        let file = ctx.fs.open(path)?;
        let mut meshes = Vec::default();
        if let Ok(gltf) = gltf::Gltf::from_reader(file) {
            let mut buffer_data = Vec::new();
            for buffer in gltf.buffers() {
                match buffer.source() {
                    gltf::buffer::Source::Uri(uri) => {
                        let uri = percent_encoding::percent_decode_str(uri)
                            .decode_utf8()
                            .unwrap();
                        let uri = uri.as_ref();
                        let buffer_bytes = match DataUri::parse(uri) {
                            Ok(data_uri) if VALID_MIME_TYPES.contains(&data_uri.mime_type) => {
                                data_uri.decode()?
                            }
                            Ok(_) => {
                                return Err(GameError::CustomError(
                                    "Buffer Format Unsupported".to_string(),
                                ))
                            }
                            Err(()) => {
                                return Err(GameError::CustomError("Failed to decode".to_string()))
                            }
                        };
                        buffer_data.push(buffer_bytes);
                    }
                    gltf::buffer::Source::Bin => {
                        if let Some(blob) = gltf.blob.as_deref() {
                            buffer_data.push(blob.into());
                        } else {
                            return Err(GameError::CustomError("MissingBlob".to_string()));
                        }
                    }
                }
            }
            for mesh in gltf.meshes() {
                for primitive in mesh.primitives() {
                    let reader =
                        primitive.reader(|buffer| Some(buffer_data[buffer.index()].as_slice()));
                    let mut image = Image::from_color(ctx, 1, 1, Some(graphics::Color::WHITE));
                    let texture_source = &primitive
                        .material()
                        .pbr_metallic_roughness()
                        .base_color_texture()
                        .map(|tex| tex.texture().source().source());
                    if let Some(source) = texture_source {
                        match source {
                            gltf::image::Source::View { view, mime_type } => {
                                let parent_buffer_data = &buffer_data[view.buffer().index()];
                                let data = &parent_buffer_data
                                    [view.offset()..view.offset() + view.length()];
                                let mime_type = mime_type.replace('/', ".");
                                let dynamic_img = image::load_from_memory_with_format(
                                    data,
                                    image::ImageFormat::from_path(mime_type)
                                        .unwrap_or(image::ImageFormat::Png),
                                )
                                .unwrap_or_default()
                                .into_rgba8();
                                image = Image::from_pixels(
                                    ctx,
                                    dynamic_img.as_bytes(),
                                    wgpu::TextureFormat::Rgba8UnormSrgb,
                                    dynamic_img.width(),
                                    dynamic_img.height(),
                                );
                            }
                            gltf::image::Source::Uri { uri, mime_type } => {
                                let uri = percent_encoding::percent_decode_str(uri)
                                    .decode_utf8()
                                    .unwrap();
                                let uri = uri.as_ref();
                                let bytes = match DataUri::parse(uri) {
                                    Ok(data_uri) => data_uri.decode()?,
                                    Err(()) => {
                                        return Err(GameError::CustomError(
                                            "Failed to decode".to_string(),
                                        ))
                                    }
                                };
                                let dynamic_img = image::load_from_memory_with_format(
                                    bytes.as_bytes(),
                                    image::ImageFormat::from_path(mime_type.unwrap_or_default())
                                        .unwrap_or(image::ImageFormat::Png),
                                )
                                .unwrap_or_default()
                                .into_rgba8();
                                image = Image::from_pixels(
                                    ctx,
                                    dynamic_img.as_bytes(),
                                    wgpu::TextureFormat::Rgba8UnormSrgb,
                                    dynamic_img.width(),
                                    dynamic_img.height(),
                                );
                            }
                        };
                    }
                    let mut vertices = Vec::default();
                    if let Some(vertices_read) = reader.read_positions() {
                        vertices = vertices_read
                            .map(|x| {
                                let pos = Vec3::new(x[0], x[1], x[2]);
                                Vertex3d::new(
                                    pos,
                                    glam::Vec2::ZERO,
                                    graphics::Color::new(1.0, 1.0, 1.0, 0.0),
                                )
                            })
                            .collect();
                    }
                    if let Some(tex_coords) = reader.read_tex_coords(0).map(|v| v.into_f32()) {
                        let mut idx = 0;
                        tex_coords.for_each(|tex_coord| {
                            vertices[idx].tex_coord = tex_coord;

                            idx += 1;
                        });
                    }
                    let mut indices = Vec::new();
                    if let Some(indices_raw) = reader.read_indices() {
                        indices.append(&mut indices_raw.into_u32().collect::<Vec<u32>>());
                    }

                    let mesh = Mesh3dBuilder::new()
                        .from_data(vertices, indices, Some(image))
                        .build(ctx);
                    meshes.push(mesh);
                }
            }

            let mut model = Model {
                center: None,
                transform: Transform3d::default(),
                meshes,
            };

            model.center = Some(model.to_aabb().unwrap_or_default().center.into());
            return Ok(model);
        }
        Err(GameError::CustomError(
            "Failed to load gltf model".to_string(),
        ))
    }
    /// Generate an aabb for this Model
    pub fn to_aabb(&self) -> Option<Aabb> {
        let mut minimum = Vec3::MAX;
        let mut maximum = Vec3::MIN;
        for mesh in self.meshes.iter() {
            for p in mesh.vertices.iter() {
                minimum = minimum.min(Vec3::from_array(p.pos));
                maximum = maximum.max(Vec3::from_array(p.pos));
            }
        }
        if minimum.x != std::f32::MAX
            && minimum.y != std::f32::MAX
            && minimum.z != std::f32::MAX
            && maximum.x != std::f32::MIN
            && maximum.y != std::f32::MIN
            && maximum.z != std::f32::MIN
        {
            Some(Aabb::from_min_max(minimum, maximum))
        } else {
            None
        }
    }
}
