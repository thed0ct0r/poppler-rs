use std::os::raw::{c_char, c_double, c_int, c_uint};

#[repr(C)]
pub struct PopplerRectangle {
    pub x1: c_double,
    pub y1: c_double,
    pub x2: c_double,
    pub y2: c_double,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum PopplerSelectionStyle {
    Glyph = 0,
    Word = 1,
    Line = 2,
}

pub enum PopplerDocument {}
pub enum PopplerPage {}

#[link(name = "poppler-glib")]
extern "C" {
    pub fn poppler_document_new_from_file(
        uri: *const c_char,
        password: *const c_char,
        error: *mut *mut glib::ffi::GError,
    ) -> *mut PopplerDocument;
    pub fn poppler_document_new_from_data(
        // Corrected from *mut to *const to match the C library signature
        data: *const c_char,
        length: c_int,
        password: *const c_char,
        error: *mut *mut glib::ffi::GError,
    ) -> *mut PopplerDocument;
    pub fn poppler_document_get_n_pages(document: *mut PopplerDocument) -> c_int;
    pub fn poppler_document_get_page(
        document: *mut PopplerDocument,
        index: c_int,
    ) -> *mut PopplerPage;

    pub fn poppler_document_get_title(document: *mut PopplerDocument) -> *mut c_char;
    pub fn poppler_document_get_metadata(document: *mut PopplerDocument) -> *mut c_char;
    pub fn poppler_document_get_pdf_version_string(document: *mut PopplerDocument) -> *mut c_char;
    pub fn poppler_document_get_permissions(document: *mut PopplerDocument) -> c_uint;

    pub fn poppler_page_get_size(
        page: *mut PopplerPage,
        width: *mut c_double,
        height: *mut c_double,
    );

    #[cfg(feature = "render")]
    pub fn poppler_page_render(page: *mut PopplerPage, cairo: *mut cairo::ffi::cairo_t);

    #[cfg(feature = "render")]
    pub fn poppler_page_render_for_printing(
        page: *mut PopplerPage,
        cairo: *mut cairo::ffi::cairo_t,
    );

    pub fn poppler_page_get_text(page: *mut PopplerPage) -> *mut c_char;

    // Text extraction over area / page
    pub fn poppler_page_get_selected_text(
        page: *mut PopplerPage,
        style: PopplerSelectionStyle,
        selection: *mut PopplerRectangle,
    ) -> *mut c_char;

    pub fn poppler_page_get_text_for_area(
        page: *mut PopplerPage,
        area: *mut PopplerRectangle,
    ) -> *mut c_char;

    pub fn poppler_page_get_text_layout(
        page: *mut PopplerPage,
        rectangles: *mut *mut PopplerRectangle,
        n_rectangles: *mut c_uint,
    ) -> glib::ffi::gboolean;

    // Helpful to capture full content bounds
    pub fn poppler_page_get_bounding_box(
        page: *mut PopplerPage,
        rect_out: *mut PopplerRectangle,
    ) -> glib::ffi::gboolean;
}


// Declarations for our custom C++ shim functions.
// The name "text_extractor" matches the library name we chose in build.rs.
#[link(name = "text_extractor")]
extern "C" {
    pub fn extract_text_layout_from_file(
        uri: *const c_char,
        password: *const c_char,
        page_index: c_int,
    ) -> *mut c_char;

    pub fn extract_text_layout_from_data(
        data: *const c_char,
        length: c_int,
        password: *const c_char,
        page_index: c_int,
    ) -> *mut c_char;
}
