fn main() {
    #[cfg(target_os = "linux")]
    {
        // WebKitGTK's DMA-BUF renderer can leave the entire webview blank on
        // some graphics drivers. Set this before GTK/WebKit is initialized.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // GTK3's Wayland backend can fail to create the webview on some
        // compositors. Prefer XWayland when it is available; explicit user
        // settings still take precedence.
        if std::env::var_os("GDK_BACKEND").is_none()
            && std::env::var_os("WAYLAND_DISPLAY").is_some()
            && std::env::var_os("DISPLAY").is_some()
        {
            std::env::set_var("GDK_BACKEND", "x11");
        }
    }
    blockyard_lib::run();
}
