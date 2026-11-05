//! Connect a window with a renderer.
use crate::core::Color;
use crate::core::backend;
use crate::core::renderer;
use crate::graphics::color;
use crate::graphics::compositor;
use crate::graphics::{self, Antialiasing, Shell, Viewport};
use crate::{Engine, Renderer};

/// A window graphics backend for iced powered by `wgpu`.
pub struct Compositor {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    engine: Engine,
    settings: Settings,
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
    ///
    /// The message lists the options that were tried and, where the platform
    /// can enumerate them, the adapters that were available.
    #[error("no adapter was found for the options requested: {0}")]
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

        let mut adapter = None;
        let mut tried = Vec::new();

        for force_fallback_adapter in settings.software_fallback.attempts() {
            let options = wgpu::RequestAdapterOptions {
                force_fallback_adapter: *force_fallback_adapter,
                ..adapter_options
            };

            match instance.request_adapter(&options).await {
                Ok(found) => {
                    if *force_fallback_adapter {
                        log::warn!(
                            "No hardware adapter available; using a software \
                             adapter. Rendering will be slow."
                        );
                    }

                    adapter = Some(found);
                    break;
                }
                Err(error) => {
                    log::warn!("Adapter request failed ({options:?}): {error}");
                    tried.push(format!("{options:?}"));
                }
            }
        }

        let Some(adapter) = adapter else {
            #[cfg(not(target_arch = "wasm32"))]
            let available: Vec<_> = instance
                .enumerate_adapters(settings.backends)
                .await
                .iter()
                .map(wgpu::Adapter::get_info)
                .collect();

            #[cfg(target_arch = "wasm32")]
            let available = Vec::new();

            return Err(Error::NoAdapterFound(adapter_report(&tried, &available)));
        };

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

        if let Some(software_fallback) = SoftwareFallback::from_env() {
            settings.software_fallback = software_fallback;
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
        if width == 0 || height == 0 {
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

    /// Whether a software (fallback) adapter may be used.
    ///
    /// By default, it is [`SoftwareFallback::Automatic`].
    pub software_fallback: SoftwareFallback,
}

/// The policy for using a software (fallback) graphics adapter, such as
/// `llvmpipe` or WARP.
///
/// It can be overridden with the `ICED_SOFTWARE_FALLBACK` environment variable
/// (`never`, `auto` or `always`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoftwareFallback {
    /// Never use a software adapter.
    Never,
    /// Use a software adapter only if no hardware adapter is available.
    #[default]
    Automatic,
    /// Use a software adapter even if hardware adapters are available.
    Always,
}

impl SoftwareFallback {
    /// Parses a policy from the value of `ICED_SOFTWARE_FALLBACK`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "never" | "0" | "false" => Some(Self::Never),
            "auto" | "automatic" => Some(Self::Automatic),
            "always" | "1" | "true" => Some(Self::Always),
            _ => None,
        }
    }

    /// Reads the policy from the `ICED_SOFTWARE_FALLBACK` environment variable.
    pub fn from_env() -> Option<Self> {
        Self::parse(&std::env::var("ICED_SOFTWARE_FALLBACK").ok()?)
    }

    /// The values of `force_fallback_adapter` to try, in order.
    fn attempts(self) -> &'static [bool] {
        match self {
            Self::Never => &[false],
            Self::Automatic => &[false, true],
            Self::Always => &[true],
        }
    }
}

/// Describes a failed adapter selection, listing the options tried and the
/// adapters that were available.
fn adapter_report(tried: &[String], available: &[wgpu::AdapterInfo]) -> String {
    let available = if available.is_empty() {
        "none".to_owned()
    } else {
        available
            .iter()
            .map(|info| format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "tried [{}]; available adapters: {available}. Set ICED_SOFTWARE_FALLBACK=always \
         to force a software adapter, or WGPU_BACKEND to pick another backend",
        tried.join("; ")
    )
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            present_mode: wgpu::PresentMode::AutoVsync,
            backends: wgpu::Backends::all(),
            power_preference: backend::PowerPreference::None,
            antialiasing: None,
            software_fallback: SoftwareFallback::default(),
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
            software_fallback: SoftwareFallback::default(),
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

    #[test]
    fn attempts_follow_policy() {
        assert_eq!(SoftwareFallback::Never.attempts(), &[false]);
        assert_eq!(SoftwareFallback::Automatic.attempts(), &[false, true]);
        assert_eq!(SoftwareFallback::Always.attempts(), &[true]);
        assert_eq!(SoftwareFallback::default(), SoftwareFallback::Automatic);
    }

    #[test]
    fn parses_policy() {
        assert_eq!(
            SoftwareFallback::parse("never"),
            Some(SoftwareFallback::Never)
        );
        assert_eq!(
            SoftwareFallback::parse(" Auto "),
            Some(SoftwareFallback::Automatic)
        );
        assert_eq!(
            SoftwareFallback::parse("ALWAYS"),
            Some(SoftwareFallback::Always)
        );
        assert_eq!(SoftwareFallback::parse("1"), Some(SoftwareFallback::Always));
        assert_eq!(SoftwareFallback::parse("0"), Some(SoftwareFallback::Never));
        assert_eq!(SoftwareFallback::parse("sometimes"), None);
    }

    #[test]
    fn report_lists_adapters() {
        let empty = adapter_report(&["a".to_owned(), "b".to_owned()], &[]);
        assert!(empty.contains("tried [a; b]"));
        assert!(empty.contains("available adapters: none"));

        let mut info = wgpu::AdapterInfo::new(wgpu::DeviceType::Cpu, wgpu::Backend::Vulkan);
        info.name = "llvmpipe".to_owned();
        let report = adapter_report(&[], &[info]);
        assert!(report.contains("llvmpipe (Vulkan, Cpu)"));
        assert!(report.contains("ICED_SOFTWARE_FALLBACK"));
    }
}
