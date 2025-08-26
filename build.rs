fn main() {
    // Use pkg-config to find all necessary C/C++ libraries.
    // By handling the Result, we can provide a much more helpful error
    // message if a dependency is not found, instead of panicking.
    let poppler_cpp = pkg_config::probe_library("poppler-cpp")
        .expect("Failed to find poppler-cpp. Please ensure that the poppler development libraries are installed.");
    let poppler_core = pkg_config::probe_library("poppler")
        .expect("Failed to find poppler (core). Please ensure that the poppler development libraries are installed.");
    let glib = pkg_config::probe_library("glib-2.0").expect(
        "Failed to find glib-2.0. Please ensure that the glib development libraries are installed.",
    );

    // Combine the include paths from all required libraries.
    let mut includes = poppler_cpp.include_paths;
    includes.extend(poppler_core.include_paths);
    includes.extend(glib.include_paths);

    // Remove duplicate paths to be clean.
    includes.sort();
    includes.dedup();

    cc::Build::new()
        .cpp(true)
        .file("src/text_extractor.cpp")
        // Add the flag to compile with the C++20 standard.
        .flag("-std=c++20")
        // Use the .includes() method, which is the idiomatic way to add
        // header search paths with the cc crate. This avoids breaking
        // the compiler's standard library search.
        .includes(includes)
        .compile("text_extractor");
}
