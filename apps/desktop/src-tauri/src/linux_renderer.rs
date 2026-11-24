use std::{
    env, fs,
    os::unix::fs::FileTypeExt,
    path::PathBuf,
    process::{Command, Stdio},
};

const DMABUF_OVERRIDE: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

pub(crate) fn configure_dmabuf() {
    if env::var_os(DMABUF_OVERRIDE).is_some() {
        return;
    }

    let has = |name| env::var_os(name).is_some_and(|value| !value.is_empty());
    let wayland = has("WAYLAND_DISPLAY")
        || has("WAYLAND_SOCKET")
        || env::var_os("XDG_RUNTIME_DIR").is_some_and(|runtime| {
            fs::metadata(PathBuf::from(runtime).join("wayland-0"))
                .is_ok_and(|metadata| metadata.file_type().is_socket())
        });
    let backends = env::var("GDK_BACKEND").ok();
    let Some(platform) = display_platform(backends.as_deref(), wayland, has("DISPLAY")) else {
        return;
    };

    // EGL initializes the graphics driver. Keep the probe's mappings out of the app.
    let Ok(output) = Command::new("/usr/bin/timeout")
        .args([
            "-k",
            "0.05s",
            "0.25s",
            "/usr/bin/eglinfo",
            "-B",
            "-p",
            platform,
        ])
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return;
    };
    if !output.status.success()
        || !reports_software(&String::from_utf8_lossy(&output.stdout), platform)
    {
        return;
    }

    if fs::read_dir("/proc/self/task").is_ok_and(|tasks| tasks.count() == 1) {
        // SAFETY: Called at the binary entry point before the runtime starts.
        // The process has a single thread and starts no concurrent work here.
        unsafe { env::set_var(DMABUF_OVERRIDE, "1") };
    }
}

fn display_platform(backends: Option<&str>, wayland: bool, x11: bool) -> Option<&'static str> {
    for backend in backends.unwrap_or("*").split(',') {
        match backend {
            "*" => {
                return if wayland {
                    Some("wayland")
                } else {
                    x11.then_some("x11")
                };
            }
            "wayland" if wayland => return Some("wayland"),
            "x11" if x11 => return Some("x11"),
            "wayland" | "x11" => {}
            _ => return None,
        }
    }
    None
}

fn reports_software(output: &str, platform: &str) -> bool {
    let expected = format!("{platform} platform:");
    let mut renderers = output
        .lines()
        .skip_while(|line| !line.trim().eq_ignore_ascii_case(&expected))
        .skip(1)
        .take_while(|line| !line.trim_end().ends_with(" platform:"))
        .filter_map(|line| {
            let (label, renderer) = line.split_once(':')?;
            let label = label.trim();
            (label.starts_with("OpenGL ") && label.ends_with(" renderer")).then_some(renderer)
        });
    let software = |renderer: &str| {
        let name = renderer.trim().to_ascii_lowercase();
        let driver = name
            .split(|c: char| c.is_ascii_whitespace() || c == '(')
            .next();
        matches!(
            driver,
            Some("llvmpipe" | "softpipe" | "swrast" | "kms_swrast" | "lavapipe" | "swiftshader")
        ) || name == "software rasterizer"
    };
    renderers.next().is_some_and(software) && renderers.all(software)
}

#[cfg(test)]
mod tests {
    use super::{display_platform, reports_software};

    #[test]
    fn probes_the_platform_selected_by_gtk() {
        assert_eq!(display_platform(None, false, true), Some("x11"));
        assert_eq!(display_platform(None, true, true), Some("wayland"));
        assert_eq!(
            display_platform(Some("x11,wayland"), true, true),
            Some("x11")
        );
        assert_eq!(
            display_platform(Some("x11,wayland"), true, false),
            Some("wayland")
        );
        assert_eq!(display_platform(Some("broadway"), false, true), None);
        assert_eq!(display_platform(None, false, false), None);
    }

    #[test]
    fn recognizes_software_rendering_on_the_requested_platform() {
        for name in [
            "llvmpipe (LLVM 19.1.7)",
            "softpipe",
            "swrast",
            "Software Rasterizer",
        ] {
            let output = format!("X11 platform:\nOpenGL ES profile renderer: {name}\n");
            assert!(reports_software(&output, "x11"), "{name}");
        }
        assert!(reports_software(
            "Wayland platform:\nOpenGL core profile renderer: llvmpipe (LLVM 19.1.7)\n",
            "wayland",
        ));
    }

    #[test]
    fn leaves_hardware_and_unconfirmed_renderers_unchanged() {
        let output = "X11 platform:\nOpenGL ES profile renderer: AMD Radeon\n\nSurfaceless platform:\nOpenGL ES profile renderer: llvmpipe (LLVM 19.1.7)";
        assert!(!reports_software(output, "x11"));
        assert!(!reports_software(output, "wayland"));
        assert!(!reports_software(
            "X11 platform:\neglInitialize failed",
            "x11"
        ));
        assert!(!reports_software(
            "X11 platform:\nOpenGL core profile renderer: llvmpipe\nOpenGL ES profile renderer: AMD Radeon",
            "x11",
        ));
    }
}
