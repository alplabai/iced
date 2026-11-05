//! Draw custom primitives.
use crate::core::{self, Rectangle};
use crate::graphics::Viewport;
use crate::graphics::futures::{MaybeSend, MaybeSync};

use rustc_hash::FxHashMap;
use std::any::{Any, TypeId};
use std::fmt::Debug;

/// The [`Primitive::pipeline_key`] used unless a [`Primitive`] overrides it.
const DEFAULT_KEY: u64 = 0;

/// The number of frames a keyed [`Pipeline`] may go unprepared before
/// [`Storage::trim`] drops it.
const MAX_IDLE_FRAMES: u64 = 120;

/// A batch of primitives.
pub type Batch = Vec<Instance>;

/// A set of methods which allows a [`Primitive`] to be rendered.
pub trait Primitive: Debug + MaybeSend + MaybeSync + 'static {
    /// The shared renderer of this [`Primitive`].
    ///
    /// Normally, this will contain a bunch of [`wgpu`] state; like
    /// a rendering pipeline, buffers, and textures.
    ///
    /// All instances of this [`Primitive`] type will share the same
    /// [`Renderer`].
    type Pipeline: Pipeline + MaybeSend + MaybeSync;

    /// The key selecting which [`Pipeline`] this [`Primitive`] is prepared
    /// and drawn with.
    ///
    /// All [`Primitive`]s of the same type and the same key share one
    /// [`Pipeline`]; a different key gets a separate one, created with
    /// [`Pipeline::new_keyed`]. This lets two widgets that use the same
    /// [`Primitive`] type keep independent GPU state without declaring a
    /// marker type for each.
    ///
    /// A [`Pipeline`] whose key is not `0` is dropped after it has not been
    /// prepared for a number of frames (see [`Storage::trim`]), so keys
    /// can be derived from short-lived data. The pipeline of key `0`, the
    /// default, is never evicted.
    fn pipeline_key(&self) -> u64 {
        DEFAULT_KEY
    }

    /// Processes the [`Primitive`], allowing for GPU buffer allocation.
    fn prepare(
        &self,
        pipeline: &mut Self::Pipeline,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bounds: &Rectangle,
        viewport: &Viewport,
    );

    /// Draws the [`Primitive`] in the given [`wgpu::RenderPass`].
    ///
    /// When possible, this should be implemented over [`render`](Self::render)
    /// since reusing the existing render pass should be considerably more
    /// efficient than issuing a new one.
    ///
    /// The viewport and scissor rect of the render pass provided is set
    /// to the bounds and clip bounds of the [`Primitive`], respectively.
    ///
    /// If you have complex composition needs, then you can leverage
    /// [`render`](Self::render) by returning `false` here.
    ///
    /// By default, it does nothing and returns `false`.
    fn draw(&self, _pipeline: &Self::Pipeline, _render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        false
    }

    /// Renders the [`Primitive`], using the given [`wgpu::CommandEncoder`].
    ///
    /// This will only be called if [`draw`](Self::draw) returns `false`.
    ///
    /// By default, it does nothing.
    fn render(
        &self,
        _pipeline: &Self::Pipeline,
        _encoder: &mut wgpu::CommandEncoder,
        _target: &wgpu::TextureView,
        _clip_bounds: &Rectangle<u32>,
    ) {
    }
}

/// The pipeline of a graphics [`Primitive`].
pub trait Pipeline: Any + MaybeSend + MaybeSync {
    /// Creates the [`Pipeline`] of a [`Primitive`].
    ///
    /// This will only be called once, when the first [`Primitive`] with this kind
    /// of [`Pipeline`] is encountered.
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self
    where
        Self: Sized;

    /// Creates the [`Pipeline`] of a [`Primitive`] for the given
    /// [`Primitive::pipeline_key`].
    ///
    /// This will only be called once per key. By default, it ignores the key
    /// and delegates to [`new`](Self::new).
    fn new_keyed(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        _key: u64,
    ) -> Self
    where
        Self: Sized,
    {
        Self::new(device, queue, format)
    }

    /// Trims any cached data in the [`Pipeline`].
    ///
    /// This will normally be called at the end of a frame.
    fn trim(&mut self) {}
}

pub(crate) trait Stored: Debug + MaybeSend + MaybeSync + 'static {
    fn prepare(
        &self,
        storage: &mut Storage,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        bounds: &Rectangle,
        viewport: &Viewport,
    );

    fn draw(&self, storage: &Storage, render_pass: &mut wgpu::RenderPass<'_>) -> bool;

    fn render(
        &self,
        storage: &Storage,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    );
}

#[derive(Debug)]
struct BlackBox<P: Primitive> {
    primitive: P,
}

impl<P: Primitive> Stored for BlackBox<P> {
    fn prepare(
        &self,
        storage: &mut Storage,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        bounds: &Rectangle,
        viewport: &Viewport,
    ) {
        let key = self.primitive.pipeline_key();

        if !storage.has_keyed::<P>(key) {
            storage.store_keyed::<P, _>(key, P::Pipeline::new_keyed(device, queue, format, key));
        }

        let renderer = storage
            .get_mut_keyed::<P>(key)
            .expect("renderer should be initialized")
            .downcast_mut::<P::Pipeline>()
            .expect("renderer should have the proper type");

        self.primitive
            .prepare(renderer, device, queue, bounds, viewport);
    }

    fn draw(&self, storage: &Storage, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
        let renderer = storage
            .get_keyed::<P>(self.primitive.pipeline_key())
            .expect("renderer should be initialized")
            .downcast_ref::<P::Pipeline>()
            .expect("renderer should have the proper type");

        self.primitive.draw(renderer, render_pass)
    }

    fn render(
        &self,
        storage: &Storage,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let renderer = storage
            .get_keyed::<P>(self.primitive.pipeline_key())
            .expect("renderer should be initialized")
            .downcast_ref::<P::Pipeline>()
            .expect("renderer should have the proper type");

        self.primitive
            .render(renderer, encoder, target, clip_bounds);
    }
}

#[derive(Debug)]
/// An instance of a specific [`Primitive`].
pub struct Instance {
    /// The bounds of the [`Instance`].
    pub(crate) bounds: Rectangle,

    /// The [`Primitive`] to render.
    pub(crate) primitive: Box<dyn Stored>,
}

impl Instance {
    /// Creates a new [`Instance`] with the given [`Primitive`].
    pub fn new(bounds: Rectangle, primitive: impl Primitive) -> Self {
        Instance {
            bounds,
            primitive: Box::new(BlackBox { primitive }),
        }
    }
}

/// A renderer than can draw custom primitives.
pub trait Renderer: core::Renderer {
    /// Draws a custom primitive.
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: impl Primitive);
}

/// Stores custom, user-provided types.
#[derive(Default)]
pub struct Storage {
    pipelines: FxHashMap<(TypeId, u64), Entry>,
    frame: u64,
}

struct Entry {
    pipeline: Box<dyn Pipeline>,
    last_used: u64,
}

impl Storage {
    /// Returns `true` if `Storage` contains a type `T`.
    pub fn has<T: 'static>(&self) -> bool {
        self.has_keyed::<T>(DEFAULT_KEY)
    }

    /// Returns `true` if `Storage` contains a type `T` stored under `key`.
    pub fn has_keyed<T: 'static>(&self, key: u64) -> bool {
        self.pipelines.contains_key(&(TypeId::of::<T>(), key))
    }

    /// Inserts the data `T` in to [`Storage`].
    pub fn store<T: 'static, P: Pipeline>(&mut self, pipeline: P) {
        self.store_keyed::<T, P>(DEFAULT_KEY, pipeline);
    }

    /// Inserts the data `T` in to [`Storage`] under `key`.
    pub fn store_keyed<T: 'static, P: Pipeline>(&mut self, key: u64, pipeline: P) {
        let _ = self.pipelines.insert(
            (TypeId::of::<T>(), key),
            Entry {
                pipeline: Box::new(pipeline),
                last_used: self.frame,
            },
        );
    }

    /// Returns a reference to the data with type `T` if it exists in [`Storage`].
    pub fn get<T: 'static>(&self) -> Option<&dyn Any> {
        self.get_keyed::<T>(DEFAULT_KEY)
    }

    /// Returns a reference to the data with type `T` stored under `key`,
    /// if it exists in [`Storage`].
    pub fn get_keyed<T: 'static>(&self, key: u64) -> Option<&dyn Any> {
        self.pipelines
            .get(&(TypeId::of::<T>(), key))
            .map(|entry| entry.pipeline.as_ref() as &dyn Any)
    }

    /// Returns a mutable reference to the data with type `T` if it exists in [`Storage`].
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut dyn Any> {
        self.get_mut_keyed::<T>(DEFAULT_KEY)
    }

    /// Returns a mutable reference to the data with type `T` stored under
    /// `key`, if it exists in [`Storage`].
    ///
    /// This marks the data as used in the current frame, which keeps it from
    /// being evicted by [`trim`](Self::trim).
    pub fn get_mut_keyed<T: 'static>(&mut self, key: u64) -> Option<&mut dyn Any> {
        let frame = self.frame;

        self.pipelines
            .get_mut(&(TypeId::of::<T>(), key))
            .map(|entry| {
                entry.last_used = frame;
                entry.pipeline.as_mut() as &mut dyn Any
            })
    }

    /// Trims the cache of all the pipelines in the [`Storage`], ends the
    /// current frame, and drops any keyed pipeline that has not been prepared
    /// for a number of frames.
    ///
    /// Pipelines stored under the default key are never dropped.
    pub fn trim(&mut self) {
        self.trim_idle(MAX_IDLE_FRAMES);
    }

    fn trim_idle(&mut self, max_idle_frames: u64) {
        let frame = self.frame;

        self.pipelines.retain(|&(_, key), entry| {
            key == DEFAULT_KEY || frame.saturating_sub(entry.last_used) <= max_idle_frames
        });

        for entry in self.pipelines.values_mut() {
            entry.pipeline.trim();
        }

        self.frame += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counted(Arc<AtomicUsize>);

    impl Pipeline for Counted {
        fn new(_: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) -> Self {
            unreachable!("tests build pipelines directly")
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            let _ = self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct Marker;

    #[test]
    fn keys_are_separate_slots() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut storage = Storage::default();

        storage.store::<Marker, _>(Counted(drops.clone()));
        storage.store_keyed::<Marker, _>(7, Counted(drops.clone()));

        assert!(storage.has::<Marker>());
        assert!(storage.has_keyed::<Marker>(0));
        assert!(storage.has_keyed::<Marker>(7));
        assert!(!storage.has_keyed::<Marker>(8));
        assert!(storage.get_keyed::<Marker>(8).is_none());
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        let a = storage.get_keyed::<Marker>(0).unwrap() as *const dyn Any as *const ();
        let b = storage.get_keyed::<Marker>(7).unwrap() as *const dyn Any as *const ();
        assert_ne!(a, b);
    }

    #[test]
    fn unprepared_keyed_pipelines_are_dropped() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut storage = Storage::default();

        storage.store::<Marker, _>(Counted(drops.clone()));
        storage.store_keyed::<Marker, _>(1, Counted(drops.clone()));
        storage.store_keyed::<Marker, _>(2, Counted(drops.clone()));

        for _ in 0..3 {
            // Only key 2 keeps being prepared.
            let _ = storage.get_mut_keyed::<Marker>(2);
            storage.trim_idle(1);
        }

        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(!storage.has_keyed::<Marker>(1));
        assert!(storage.has_keyed::<Marker>(2));
        // The default key is never evicted, even when idle.
        assert!(storage.has::<Marker>());
    }

    #[test]
    fn trim_keeps_recently_used_pipelines() {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut storage = Storage::default();

        storage.store_keyed::<Marker, _>(1, Counted(drops.clone()));

        for _ in 0..MAX_IDLE_FRAMES {
            storage.trim();
        }

        assert!(storage.has_keyed::<Marker>(1));
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        storage.trim();
        storage.trim();

        assert!(!storage.has_keyed::<Marker>(1));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let waker = futures::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);

        loop {
            if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }

            std::thread::yield_now();
        }
    }

    #[derive(Debug)]
    struct Keyed(u64);

    struct KeyedPipeline {
        key: u64,
        prepared: Vec<u64>,
    }

    impl Pipeline for KeyedPipeline {
        fn new(_: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) -> Self {
            Self {
                key: u64::MAX,
                prepared: Vec::new(),
            }
        }

        fn new_keyed(_: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat, key: u64) -> Self {
            Self {
                key,
                prepared: Vec::new(),
            }
        }
    }

    impl Primitive for Keyed {
        type Pipeline = KeyedPipeline;

        fn pipeline_key(&self) -> u64 {
            self.0
        }

        fn prepare(
            &self,
            pipeline: &mut KeyedPipeline,
            _: &wgpu::Device,
            _: &wgpu::Queue,
            _: &Rectangle,
            _: &Viewport,
        ) {
            assert_eq!(pipeline.key, self.0);
            pipeline.prepared.push(self.0);
        }
    }

    #[test]
    fn primitives_with_different_keys_get_their_own_pipeline() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let Ok(adapter) = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        })) else {
            eprintln!("no adapter available; skipping");
            return;
        };

        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("device");

        let viewport =
            Viewport::with_physical_size(core::Size::new(10, 10), core::renderer::Scale::default());
        let bounds = Rectangle::with_size(core::Size::new(10.0, 10.0));
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut storage = Storage::default();

        for key in [0, 5, 5, 6] {
            let instance = Instance::new(bounds, Keyed(key));

            instance
                .primitive
                .prepare(&mut storage, &device, &queue, format, &bounds, &viewport);
        }

        let prepared = |key| {
            storage
                .get_keyed::<Keyed>(key)
                .and_then(|pipeline| pipeline.downcast_ref::<KeyedPipeline>())
                .map(|pipeline| pipeline.prepared.clone())
        };

        assert_eq!(prepared(0), Some(vec![0]));
        assert_eq!(prepared(5), Some(vec![5, 5]));
        assert_eq!(prepared(6), Some(vec![6]));
        assert_eq!(prepared(7), None);
    }
}
