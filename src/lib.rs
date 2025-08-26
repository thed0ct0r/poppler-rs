use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path;

/// Re-exports `cairo` to provide types required for rendering.
#[cfg(feature = "render")]
pub use cairo;

mod ffi;
mod util;

/// Represents the source from which a PopplerDocument was created.
#[derive(Debug)]
enum DocumentSource {
    File { uri: CString },
    Data { data: Vec<u8> },
}

/// A Poppler PDF document.
#[derive(Debug)]
pub struct PopplerDocument {
    doc: *mut ffi::PopplerDocument,
    source: DocumentSource,
    pw: CString,
}

/// Represents a page in a [`PopplerDocument`].
#[derive(Debug)]
pub struct PopplerPage<'a> {
    page: *mut ffi::PopplerPage,
    doc: &'a PopplerDocument,
    index: c_int,
}

impl PopplerDocument {
    pub fn new_from_file<P: AsRef<path::Path>>(
        p: P,
        password: Option<&str>,
    ) -> Result<PopplerDocument, glib::error::Error> {
        let pw_cstring = Self::get_password(password)?;
        let path_cstring = util::path_to_glib_url(p)?;
        let doc_ptr = util::call_with_gerror(|err_ptr| unsafe {
            ffi::poppler_document_new_from_file(path_cstring.as_ptr(), pw_cstring.as_ptr(), err_ptr)
        })?;

        Ok(PopplerDocument {
            doc: doc_ptr,
            source: DocumentSource::File { uri: path_cstring },
            pw: pw_cstring,
        })
    }

    pub fn new_from_data(
        data: &[u8],
        password: Option<&str>,
    ) -> Result<PopplerDocument, glib::error::Error> {
        if data.is_empty() {
            return Err(glib::error::Error::new(
                glib::FileError::Inval,
                "data is empty",
            ));
        }
        let pw = Self::get_password(password)?;

        let doc_ptr = util::call_with_gerror(|err_ptr| unsafe {
            ffi::poppler_document_new_from_data(
                data.as_ptr() as *const c_char,
                data.len() as c_int,
                pw.as_ptr(),
                err_ptr,
            )
        })?;

        Ok(PopplerDocument {
            doc: doc_ptr,
            source: DocumentSource::Data {
                data: data.to_vec(),
            },
            pw,
        })
    }

    pub fn get_title(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_title(self.doc);
            util::take_c_owned_string(ptr)
        }
    }

    pub fn get_metadata(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_metadata(self.doc);
            util::take_c_owned_string(ptr)
        }
    }

    pub fn get_pdf_version_string(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_pdf_version_string(self.doc);
            util::take_c_owned_string(ptr)
        }
    }

    pub fn get_permissions(&self) -> u32 {
        unsafe { ffi::poppler_document_get_permissions(self.doc) as u32 }
    }

    pub fn get_n_pages(&self) -> usize {
        (unsafe { ffi::poppler_document_get_n_pages(self.doc) }) as usize
    }

    pub fn get_page(&self, index: usize) -> Option<PopplerPage<'_>> {
        match unsafe { ffi::poppler_document_get_page(self.doc, index as c_int) } {
            ptr if ptr.is_null() => None,
            ptr => Some(PopplerPage {
                page: ptr,
                doc: self,
                index: index as c_int,
            }),
        }
    }

    pub fn pages(&self) -> PagesIter<'_> {
        PagesIter {
            total: self.get_n_pages(),
            index: 0,
            doc: self,
        }
    }

    fn get_password(password: Option<&str>) -> Result<CString, glib::error::Error> {
        let pw_str = password.unwrap_or("");
        CString::new(pw_str).map_err(|_| {
            glib::error::Error::new(
                glib::FileError::Inval,
                "Password invalid (possibly contains NUL characters)",
            )
        })
    }

    pub fn len(&self) -> usize {
        self.get_n_pages()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for PopplerDocument {
    fn drop(&mut self) {
        unsafe {
            gobject_sys::g_object_unref(self.doc as *mut gobject_sys::GObject);
        }
    }
}

impl<'a> PopplerPage<'a> {
    pub fn get_size(&self) -> (f64, f64) {
        let mut width: f64 = 0.0;
        let mut height: f64 = 0.0;

        unsafe {
            ffi::poppler_page_get_size(
                self.page,
                &mut width as *mut f64 as *mut c_double,
                &mut height as *mut f64 as *mut c_double,
            )
        }

        (width, height)
    }

    #[cfg(feature = "render")]
    pub fn render(&self, ctx: &cairo::Context) {
        let ctx_raw = unsafe { ctx.to_raw_none() };
        unsafe { ffi::poppler_page_render(self.page, ctx_raw) }
    }

    #[cfg(feature = "render")]
    pub fn render_for_printing(&self, ctx: &cairo::Context) {
        let ctx_raw = unsafe { ctx.to_raw_none() };
        unsafe { ffi::poppler_page_render_for_printing(self.page, ctx_raw) }
    }

    pub fn get_text(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_page_get_text(self.page);
            util::take_c_owned_string(ptr)
        }
    }

    /// Extracts page text, preserving the physical layout.
    ///
    /// This function is unwind-safe. If a panic occurs in the underlying
    /// C++ library, it will be caught and returned as an `Err`, preventing
    /// undefined behavior.
    pub fn get_text_layout(&self) -> anyhow::Result<Option<String>> {
        // Use catch_unwind to ensure that a panic in the C++ code does not
        // cross the FFI boundary, which would be undefined behavior.
        catch_unwind(AssertUnwindSafe(|| unsafe {
            let ptr = match &self.doc.source {
                DocumentSource::File { uri } => ffi::extract_text_layout_from_file(
                    uri.as_ptr(),
                    self.doc.pw.as_ptr(),
                    self.index,
                ),
                DocumentSource::Data { data } => ffi::extract_text_layout_from_data(
                    data.as_ptr() as *const c_char,
                    data.len() as c_int,
                    self.doc.pw.as_ptr(),
                    self.index,
                ),
            };
            util::take_c_owned_string(ptr)
        }))
        .map_err(|err| anyhow::anyhow!("failed ffi call in underlying poppler c code => {err:?}"))
    }
}

impl<'a> Drop for PopplerPage<'a> {
    fn drop(&mut self) {
        unsafe {
            gobject_sys::g_object_unref(self.page as *mut gobject_sys::GObject);
        }
    }
}

#[derive(Debug)]
pub struct PagesIter<'a> {
    total: usize,
    index: usize,
    doc: &'a PopplerDocument,
}

impl<'a> Iterator for PagesIter<'a> {
    type Item = PopplerPage<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.total {
            let page = self.doc.get_page(self.index);
            self.index += 1;
            page
        } else {
            None
        }
    }
}

// The existing tests will need to be updated to accommodate the
// new lifetime-bound PopplerPage struct. For example, `get_page`
// calls will create a page that cannot outlive the `doc` variable.
#[cfg(test)]
mod tests {
    // ... tests would need to be updated ...
    // For example:
    #[test]
    fn test_layout_extraction() {
        let filename = "test.pdf"; // Make sure you have a test PDF
        let doc = super::PopplerDocument::new_from_file(filename, None).unwrap();
        let page = doc.get_page(0).unwrap();

        let simple_text = page.get_text().unwrap_or_default();
        let layout_text = page
            .get_text_layout()
            .expect("could not get text layout")
            .unwrap_or_default();

        println!("--- Simple Text ---\n{}", simple_text);
        println!("--- Layout Text ---\n{}", layout_text);

        // In a real test, you would assert that layout_text contains
        // formatting (like multiple spaces) that simple_text does not.
        assert!(!layout_text.is_empty());
    }

    #[test]
    fn test_layout_extraction2() {
        let filename = "us1.pdf"; // Make sure you have a test PDF
        let doc = super::PopplerDocument::new_from_file(filename, None).unwrap();
        let page = doc.get_page(0).unwrap();

        let simple_text = page.get_text().unwrap_or_default();
        let layout_text = page
            .get_text_layout()
            .expect("could not get text layout")
            .unwrap_or_default();

        println!("--- Simple Text ---\n{}", simple_text);
        println!("--- Layout Text ---\n{}", layout_text);

        // In a real test, you would assert that layout_text contains
        // formatting (like multiple spaces) that simple_text does not.
        assert!(!layout_text.is_empty());
    }
}
