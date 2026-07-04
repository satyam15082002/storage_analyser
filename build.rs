fn main() {
    // Embeds installer/icon/storage_analyser.ico as the compiled exe's own icon (what
    // Explorer/the taskbar/Alt-Tab show), via `windres` (part of the mingw64 toolchain
    // this project already builds with).
    embed_resource::compile("installer/icon/app.rc", embed_resource::NONE);
}
