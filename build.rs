fn main() {
    // Use pkg-config to find all necessary C/C++ libraries.
    let poppler_cpp = pkg_config::probe_library("poppler-cpp").unwrap();
    let poppler_core = pkg_config::probe_library("poppler").unwrap();
    let glib = pkg_config::probe_library("glib-2.0").unwrap();

    // Combine the include paths from all required libraries.
    let mut includes = poppler_cpp.include_paths;
    includes.extend(poppler_core.include_paths);
    includes.extend(glib.include_paths);

    // Remove duplicate paths to be clean.
    includes.sort();
    includes.dedup();

    // Compile our C++ shim.
    cc::Build::new()
        .cpp(true)
        .file("src/text_extractor.cpp")
        // Add the combined include paths.
        .includes(includes)
        // Add the flag to compile with the C++20 standard, which is required
        // by modern versions of the Poppler library.
        .flag("-std=c++20")
        .compile("text_extractor");
}
