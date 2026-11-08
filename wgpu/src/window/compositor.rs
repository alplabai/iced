//! Connect a window with a renderer.
use crate::core::Color;
use crate::core::backend;
use crate::core::renderer;
use crate::graphics::color;
use crate::graphics::compositor;
use crate::graphics::{self, Antialiasing, Shell, Viewport};
use crate::{Engine, Renderer};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A window graphics backend for iced powered by `wgpu`.
pub struct Compositor {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    engine: Engine,
    settings: Settings,
}

/// Tracks whether the `wgpu` device of a [`Compositor`] is still usable.
///
/// It is updated from the callbacks `wgpu` invokes when the device is lost
/// (for example after a driver reset) or reports an error that no error scope
/// captured. Shared through an [`Arc`] because those callbacks may run on any
/// thread.
#[derive(Debug, Default)]
pub(crate) struct DeviceHealth {
    lost: AtomicBool,
    uncaptured_errors: AtomicU64,
}

impl DeviceHealth {
    /// Returns `true` once the device has been lost.
    ///
    /// A lost device never recovers: everything created from it must be
    /// recreated, so callers should stop rendering with it.
    pub(crate) fn is_lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }

    /// Returns how many errors the device reported outside of any error scope.
    #[allow(dead_code)] // Only read by tests until an app-facing hook lands.
    pub(crate) fn uncaptured_errors(&self) -> u64 {
        self.uncaptured_errors.load(Ordering::Relaxed)
    }

    fn mark_lost(&self, reason: wgpu::DeviceLostReason, message: &str) {
        log::error!("wgpu device lost ({reason:?}): {message}");

        self.lost.store(true, Ordering::Release);
    }

    fn record_error(&self, error: &wgpu::Error) {
        let count = self.uncaptured_errors.fetch_add(1, Ordering::Relaxed);

        // A broken pipeline can fail every frame, so only the first few
        // errors and then exponentially rarer ones are logged. Counting
        // stays exact.
        if should_log(count) {
            log::error!("wgpu uncaptured error (#{}): {error}", count + 1);
        }

        if panics_on(error) {
            panic!("wgpu validation error: {error}");
        }
    }

    /// Installs the lost and uncaptured error callbacks on the given device.
    ///
    /// Without them, `wgpu` panics on any uncaptured error.
    pub(crate) fn install(self: &Arc<Self>, device: &wgpu::Device) {
        let health = Arc::clone(self);

        device.set_device_lost_callback(move |reason, message| {
            health.mark_lost(reason, &message);
        });

        let health = Arc::clone(self);

        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            health.record_error(&error);
        }));
    }
}

/// Returns whether the uncaptured error with the given zero-based index is
/// logged: the first few, then exponentially rarer ones.
fn should_log(count: u64) -> bool {
    count < 8 || (count + 1).is_power_of_two()
}

/// Returns whether an uncaptured error must panic.
///
/// A validation error is a bug in the code or data given to `wgpu` (for example
/// a broken custom shader pipeline), so it keeps panicking in debug builds, or
/// when the `ICED_WGPU_PANIC_ON_ERROR` environment variable is set. Out of
/// memory and internal errors are only logged and counted.
fn panics_on(error: &wgpu::Error) -> bool {
    matches!(error, wgpu::Error::Validation { .. })
        && (cfg!(debug_assertions) || std::env::var_os("ICED_WGPU_PANIC_ON_ERROR").is_some())
}

/// A compositor error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    /// The surface creation failed.
    #[error("the surface creation failed: {0}")]
    SurfaceCreationFailed(#[from] wgpu::CreateSurfaceError),
    /// The surface is not compatible.
    #[error("the surface is not compatible")]
    IncompatibleSurface,
    /// No adapter was found for the options requested.
    #[error("no adapter was found for the options requested: {0:?}")]
    NoAdapterFound(String),
    /// No device request succeeded.
    #[error("no device request succeeded: {0:?}")]
    RequestDeviceFailed(Vec<(wgpu::Limits, wgpu::RequestDeviceError)>),
}

impl From<Error> for backend::Error {
    fn from(error: Error) -> Self {
        Self::GraphicsAdapterNotFound {
            backend: "wgpu",
            reason: backend::Reason::RequestFailed(error.to_string()),
        }
    }
}

impl Compositor {
    /// Returns the [`DeviceHealth`] of the device of this [`Compositor`].
    #[allow(dead_code)] // Only read by tests until an app-facing hook lands.
    pub(crate) fn health(&self) -> &Arc<DeviceHealth> {
        &self.engine.health
    }

    /// Requests a new [`Compositor`] with the given [`Settings`].
    ///
    /// Returns `None` if no compatible graphics adapter could be found.
    pub async fn request(
        settings: Settings,
        display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
    ) -> Result<Self, Error> {
        let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
            backends: settings.backends,
            flags: if cfg!(feature = "strict-assertions") {
                wgpu::InstanceFlags::debugging()
            } else {
                wgpu::InstanceFlags::empty()
            },
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(display))
        })
        .await;

        log::info!("{settings:#?}");

        #[cfg(not(target_arch = "wasm32"))]
        if log::max_level() >= log::LevelFilter::Info {
            let available_adapters: Vec<_> = instance
                .enumerate_adapters(settings.backends)
                .await
                .iter()
                .map(wgpu::Adapter::get_info)
                .collect();
            log::info!("Available adapters: {available_adapters:#?}");
        }

        #[allow(unsafe_code)]
        let compatible_surface = instance
            .create_surface(wgpu::SurfaceTarget::Window(Box::new(compatible_window)))
            .ok();

        let power_preference = match settings.power_preference {
            backend::PowerPreference::None => wgpu::PowerPreference::None,
            backend::PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            backend::PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
        };

        let adapter_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or(power_preference),
            compatible_surface: compatible_surface.as_ref(),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        };

        let adapter = instance
            .request_adapter(&adapter_options)
            .await
            .map_err(|_error| Error::NoAdapterFound(format!("{adapter_options:?}")))?;

        log::info!("Selected: {:#?}", adapter.get_info());

        let (format, alpha_mode) = compatible_surface
            .as_ref()
            .and_then(|surface| {
                let capabilities = surface.get_capabilities(&adapter);

                let formats = capabilities.formats.iter().copied();

                log::info!("Available formats: {formats:#?}");

                const BLACKLIST: &[wgpu::TextureFormat] = &[
                    wgpu::TextureFormat::Rgb10a2Unorm,
                    wgpu::TextureFormat::Rgb10a2Uint,
                    wgpu::TextureFormat::Rgba16Float,
                ];

                let mut formats = formats.filter(|format| {
                    format.required_features() == wgpu::Features::empty()
                        && !BLACKLIST.contains(format)
                });

                let format = if color::GAMMA_CORRECTION {
                    formats.find(wgpu::TextureFormat::is_srgb)
                } else {
                    formats.find(|format| !wgpu::TextureFormat::is_srgb(format))
                };

                let format = format.or_else(|| {
                    log::warn!("No format found!");

                    capabilities.formats.first().copied()
                });

                let alpha_modes = capabilities.alpha_modes;

                log::info!("Available alpha modes: {alpha_modes:#?}");

                let preferred_alpha =
                    if alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                        wgpu::CompositeAlphaMode::PreMultiplied
                    } else {
                        wgpu::CompositeAlphaMode::Auto
                    };

                format.zip(Some(preferred_alpha))
            })
            .ok_or(Error::IncompatibleSurface)?;

        log::info!("Selected format: {format:?} with alpha mode: {alpha_mode:?}");

        #[cfg(target_arch = "wasm32")]
        let limits = [wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())];

        #[cfg(not(target_arch = "wasm32"))]
        let limits = [
            wgpu::Limits::default().using_resolution(adapter.limits()),
            wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
        ];

        let limits = limits.into_iter().map(|limits| wgpu::Limits {
            max_non_sampler_bindings: 2048,
            ..limits
        });

        // Request optional features only when supported by the adapter (e.g., not available in WebGL2).
        let required_features = adapter.features()
            & (wgpu::Features::SHADER_F16
                | wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                | wgpu::Features::TEXTURE_FORMAT_NV12
                | wgpu::Features::TEXTURE_FORMAT_P010
                | wgpu::Features::VULKAN_EXTERNAL_MEMORY_DMA_BUF
                | wgpu::Features::VULKAN_EXTERNAL_MEMORY_FD
                | wgpu::Features::VULKAN_EXTERNAL_MEMORY_WIN32);

        let mut errors = Vec::new();

        for required_limits in limits {
            let result = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("iced_wgpu::window::compositor device descriptor"),
                    required_features,
                    required_limits: required_limits.clone(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                    trace: wgpu::Trace::Off,
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                })
                .await;

            match result {
                Ok((device, queue)) => {
                    let engine = Engine::new(
                        &adapter,
                        device,
                        queue,
                        format,
                        settings.antialiasing,
                        shell,
                    );

                    return Ok(Compositor {
                        instance,
                        adapter,
                        format,
                        alpha_mode,
                        engine,
                        settings,
                    });
                }
                Err(error) => {
                    errors.push((required_limits, error));
                }
            }
        }

        Err(Error::RequestDeviceFailed(errors))
    }
}

/// Creates a [`Compositor`] with the given [`Settings`] and window.
pub async fn new(
    settings: Settings,
    display: impl compositor::Display,
    compatible_window: impl compositor::Window,
    shell: Shell,
) -> Result<Compositor, Error> {
    Compositor::request(settings, display, compatible_window, shell).await
}

/// Presents the given primitives with the given [`Compositor`].
pub fn present(
    renderer: &mut Renderer,
    surface: &mut wgpu::Surface<'static>,
    viewport: &Viewport,
    background_color: Color,
    on_pre_present: impl FnOnce(),
) -> Result<(), compositor::SurfaceError> {
    let size = viewport.physical_size();

    if size.width == 0 || size.height == 0 {
        return Err(compositor::SurfaceError::Outdated);
    }

    match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => {
            let view = &frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let _submission = renderer.present(
                Some(background_color),
                frame.texture.format(),
                view,
                viewport,
            );

            // Present the frame
            on_pre_present();
            renderer.engine.queue.present(frame);

            Ok(())
        }
        wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
            Err(compositor::SurfaceError::Outdated)
        }
        wgpu::CurrentSurfaceTexture::Timeout => Err(compositor::SurfaceError::Timeout),
        wgpu::CurrentSurfaceTexture::Occluded => Err(compositor::SurfaceError::Occluded),
        wgpu::CurrentSurfaceTexture::Lost => Err(compositor::SurfaceError::Lost),
        wgpu::CurrentSurfaceTexture::Validation => Err(compositor::SurfaceError::Other),
    }
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = wgpu::Surface<'static>;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display,
        compatible_window: impl compositor::Window,
        shell: Shell,
    ) -> Result<Self, backend::Error> {
        if settings.backend.hardware().is_none() && !settings.backend.matches("wgpu") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "wgpu",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }

        let mut settings = Settings::from(settings);

        if let Some(backends) = wgpu::Backends::from_env() {
            settings.backends = backends;
        }

        if let Some(present_mode) = present_mode_from_env() {
            settings.present_mode = present_mode;
        }

        Ok(new(settings, display, compatible_window, shell).await?)
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Self::Renderer {
        Renderer::new(self.engine.clone(), settings)
    }

    fn create_surface(&mut self, window: impl compositor::Window) -> Self::Surface {
        self.instance
            .create_surface(wgpu::SurfaceTarget::Window(Box::new(window)))
            .expect("Create surface")
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        if width == 0 || height == 0 || self.engine.health.is_lost() {
            return;
        }

        let capabilities = surface.get_capabilities(&self.adapter);
        let has_copy_src = capabilities.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let usage = if has_copy_src {
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        };

        surface.configure(
            &self.engine.device,
            &wgpu::SurfaceConfiguration {
                usage,
                format: self.format,
                present_mode: self.settings.present_mode,
                width,
                height,
                alpha_mode: self.alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 1,
                color_space: wgpu::SurfaceColorSpace::Auto,
            },
        );
    }

    fn information(&self) -> compositor::Information {
        let information = self.adapter.get_info();

        compositor::Information {
            adapter: information.name,
            backend: format!("{:?}", information.backend),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &Viewport,
        background_color: Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        if self.engine.health.is_lost() {
            return Err(compositor::SurfaceError::DeviceLost);
        }

        present(
            renderer,
            surface,
            viewport,
            background_color,
            on_pre_present,
        )
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &Viewport,
        background_color: Color,
    ) -> Vec<u8> {
        if self.engine.health.is_lost() {
            return Vec::new();
        }

        renderer.screenshot(viewport, background_color)
    }
}

/// The settings of a [`Compositor`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// The present mode of the [`Renderer`].
    ///
    /// [`Renderer`]: crate::Renderer
    pub present_mode: wgpu::PresentMode,

    /// The graphics backends to use.
    pub backends: wgpu::Backends,

    /// The power-usage preference for graphics adapters.
    ///
    /// By default, it is [`backend::PowerPreference::None`].
    pub power_preference: backend::PowerPreference,

    /// The antialiasing strategy that will be used for triangle primitives.
    ///
    /// By default, it is `None`.
    pub antialiasing: Option<Antialiasing>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            present_mode: wgpu::PresentMode::AutoVsync,
            backends: wgpu::Backends::all(),
            power_preference: backend::PowerPreference::None,
            antialiasing: None,
        }
    }
}

impl From<backend::Settings> for Settings {
    fn from(settings: backend::Settings) -> Self {
        let backends = settings
            .backend
            .hardware()
            .map(|api| match api {
                backend::Api::Best => wgpu::Backends::all(),
                backend::Api::Vulkan => wgpu::Backends::VULKAN,
                backend::Api::Metal => wgpu::Backends::METAL,
                backend::Api::DirectX12 => wgpu::Backends::DX12,
                backend::Api::OpenGL => wgpu::Backends::GL,
                backend::Api::WebGPU => wgpu::Backends::BROWSER_WEBGPU,
            })
            .unwrap_or_else(wgpu::Backends::all);

        Self {
            present_mode: if settings.vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            antialiasing: settings.antialiasing.then_some(Antialiasing::MSAAx4),
            backends,
            power_preference: settings.power_preference,
        }
    }
}

/// Obtains a [`wgpu::PresentMode`] from the current environment
/// configuration, if set.
///
/// The value returned by this function can be changed by setting
/// the `ICED_PRESENT_MODE` env variable. The possible values are:
///
/// - `vsync` → [`wgpu::PresentMode::AutoVsync`]
/// - `no_vsync` → [`wgpu::PresentMode::AutoNoVsync`]
/// - `immediate` → [`wgpu::PresentMode::Immediate`]
/// - `fifo` → [`wgpu::PresentMode::Fifo`]
/// - `fifo_relaxed` → [`wgpu::PresentMode::FifoRelaxed`]
/// - `mailbox` → [`wgpu::PresentMode::Mailbox`]
pub fn present_mode_from_env() -> Option<wgpu::PresentMode> {
    let present_mode = std::env::var("ICED_PRESENT_MODE").ok()?;

    match present_mode.to_lowercase().as_str() {
        "vsync" => Some(wgpu::PresentMode::AutoVsync),
        "no_vsync" => Some(wgpu::PresentMode::AutoNoVsync),
        "immediate" => Some(wgpu::PresentMode::Immediate),
        "fifo" => Some(wgpu::PresentMode::Fifo),
        "fifo_relaxed" => Some(wgpu::PresentMode::FifoRelaxed),
        "mailbox" => Some(wgpu::PresentMode::Mailbox),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a device with an adapter, if the machine has one.
    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        futures::executor::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter: false,
                    ..wgpu::RequestAdapterOptions::default()
                })
                .await
                .ok()?;

            adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .ok()
        })
    }

    #[test]
    fn starts_healthy() {
        let health = DeviceHealth::default();

        assert!(!health.is_lost());
        assert_eq!(health.uncaptured_errors(), 0);
    }

    #[test]
    fn lost_device_is_flagged() {
        let health = DeviceHealth::default();

        health.mark_lost(wgpu::DeviceLostReason::Unknown, "driver reset");

        assert!(health.is_lost());
    }

    #[test]
    fn destroying_the_device_marks_it_lost() {
        let Some((device, _queue)) = device() else {
            eprintln!("no graphics adapter available; skipping");
            return;
        };

        let health = Arc::new(DeviceHealth::default());
        health.install(&device);

        assert!(!health.is_lost());

        device.destroy();
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        assert!(health.is_lost());
    }

    #[test]
    fn screenshot_returns_early_on_a_lost_device() {
        let (instance, adapter) = futures::executor::block_on(async {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok();

            (instance, adapter)
        });

        let _instance = instance;

        let Some(adapter) = adapter else {
            eprintln!("no graphics adapter available; skipping");
            return;
        };

        let Ok((device, queue)) =
            futures::executor::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        else {
            return;
        };

        let engine = Engine::new(
            &adapter,
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            None,
            Shell::headless(),
        );

        let mut renderer =
            Renderer::new(engine.clone(), crate::core::renderer::Settings::default());
        let viewport = Viewport::with_physical_size(
            crate::core::Size::new(8, 8),
            crate::core::renderer::Scale::default(),
        );

        assert!(!renderer.screenshot(&viewport, Color::BLACK).is_empty());

        engine
            .health
            .mark_lost(wgpu::DeviceLostReason::Unknown, "test");

        assert!(renderer.screenshot(&viewport, Color::BLACK).is_empty());

        #[cfg(feature = "image")]
        {
            use crate::core::image::Renderer as _;

            assert!(
                renderer
                    .load_image(&crate::core::image::Handle::from_rgba(1, 1, vec![0; 4]))
                    .is_err()
            );
        }
    }

    fn error_source() -> wgpu::ErrorSource {
        "test".into()
    }

    #[test]
    fn out_of_memory_and_internal_errors_are_counted_without_panicking() {
        let health = DeviceHealth::default();

        health.record_error(&wgpu::Error::OutOfMemory {
            source: error_source(),
        });
        health.record_error(&wgpu::Error::Internal {
            source: error_source(),
            description: "internal".to_owned(),
        });

        assert_eq!(health.uncaptured_errors(), 2);
        assert!(!health.is_lost());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "wgpu validation error")]
    fn validation_errors_panic_in_debug_builds() {
        let health = DeviceHealth::default();

        health.record_error(&wgpu::Error::Validation {
            source: error_source(),
            description: "broken pipeline".to_owned(),
        });
    }

    #[test]
    fn error_logging_is_rate_limited() {
        let logged = (0..1000).filter(|&n| should_log(n)).count();

        assert!(should_log(0) && should_log(7));
        assert!(!should_log(8) && !should_log(100));
        assert!(logged < 20, "logged {logged} of 1000");
    }

    #[test]
    fn only_validation_errors_may_panic() {
        assert!(!panics_on(&wgpu::Error::OutOfMemory {
            source: error_source(),
        }));
        assert!(!panics_on(&wgpu::Error::Internal {
            source: error_source(),
            description: String::new(),
        }));
    }
}
