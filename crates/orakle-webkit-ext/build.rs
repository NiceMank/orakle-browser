fn main() {
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rerun-if-changed=src/ext_shim.c");
        let lib = pkg_config::Config::new()
            .probe("webkit2gtk-web-extension-4.1")
            .expect("webkit2gtk-web-extension-4.1 doit être disponible pour orakle-webkit-ext");

        let mut build = cc::Build::new();
        build.file("src/ext_shim.c");
        for inc in &lib.include_paths {
            build.include(inc);
        }
        build.compile("orakle_webkit_shim");
    }
}
