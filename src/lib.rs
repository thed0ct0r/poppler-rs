use std::ffi::CString;
use std::os::raw::{c_char, c_double, c_int, c_uint};
use std::path;

/// Re-exports `cairo` to provide types required for rendering.
#[cfg(feature = "render")]
pub use cairo;

mod ffi;
mod util;

/// A Poppler PDF document.
#[derive(Debug)]
pub struct PopplerDocument(*mut ffi::PopplerDocument);

/// Represents a page in a [`PopplerDocument`].
#[derive(Debug)]
pub struct PopplerPage(*mut ffi::PopplerPage);

impl PopplerDocument {
    /// Creates a new Poppler document.
    pub fn new_from_file<P: AsRef<path::Path>>(
        p: P,
        password: Option<&str>,
    ) -> Result<PopplerDocument, glib::error::Error> {
        let pw = Self::get_password(password)?;

        let path_cstring = util::path_to_glib_url(p)?;
        let doc = util::call_with_gerror(|err_ptr| unsafe {
            ffi::poppler_document_new_from_file(path_cstring.as_ptr(), pw.as_ptr(), err_ptr)
        })?;

        Ok(PopplerDocument(doc))
    }

    /// Creates a new Poppler document.
    pub fn new_from_data(
        data: &mut [u8],
        password: Option<&str>,
    ) -> Result<PopplerDocument, glib::error::Error> {
        if data.is_empty() {
            return Err(glib::error::Error::new(
                glib::FileError::Inval,
                "data is empty",
            ));
        }
        let pw = Self::get_password(password)?;

        let doc = util::call_with_gerror(|err_ptr| unsafe {
            ffi::poppler_document_new_from_data(
                data.as_mut_ptr() as *mut c_char,
                data.len() as c_int,
                pw.as_ptr(),
                err_ptr,
            )
        })?;

        Ok(PopplerDocument(doc))
    }

    /// Returns the document's title.
    pub fn get_title(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_title(self.0);
            util::take_c_owned_string(ptr)
        }
    }

    /// Returns the XML metadata string of the document.
    pub fn get_metadata(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_metadata(self.0);
            util::take_c_owned_string(ptr)
        }
    }

    /// Returns the PDF version of document as a string (e.g. `PDF-1.6`).
    pub fn get_pdf_version_string(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_document_get_pdf_version_string(self.0);
            util::take_c_owned_string(ptr)
        }
    }

    /// Returns the flags specifying which operations are permitted when the document is opened.
    pub fn get_permissions(&self) -> u32 {
        unsafe { ffi::poppler_document_get_permissions(self.0) as u32 }
    }

    /// Returns the number of pages in a loaded document.
    pub fn get_n_pages(&self) -> usize {
        // FIXME: what's the correct type here? can we assume a document
        //        has a positive number of pages?
        (unsafe { ffi::poppler_document_get_n_pages(self.0) }) as usize
    }

    /// Returns the page indexed at `index`.
    pub fn get_page(&self, index: usize) -> Option<PopplerPage> {
        match unsafe { ffi::poppler_document_get_page(self.0, index as c_int) } {
            ptr if ptr.is_null() => None,
            ptr => Some(PopplerPage(ptr)),
        }
    }

    pub fn pages(&self) -> PagesIter<'_> {
        PagesIter {
            total: self.get_n_pages(),
            index: 0,
            doc: self,
        }
    }


    /// Converts the provided password into a `CString`.
    fn get_password(password: Option<&str>) -> Result<CString, glib::error::Error> {
        Ok(CString::new(if password.is_none() {
            ""
        } else {
            password.expect("password.is_none() is false, but apparently it's lying.")
        })
            .map_err(|_| {
                glib::error::Error::new(
                    glib::FileError::Inval,
                    "Password invalid (possibly contains NUL characters)",
                )
            })?)
    }

    /// Returns the number of pages.
    ///
    /// This function is a convenience wrapper around [`get_n_pages`](Self::get_n_pages).
    pub fn len(&self) -> usize {
        self.get_n_pages()
    }

    /// Indicates whether this document has no pages.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for PopplerDocument {
    fn drop(&mut self) {
        unsafe {
            gobject_sys::g_object_unref(self.0 as *mut gobject_sys::GObject);
        }
    }
}

impl PopplerPage {
    /// Gets the size of page at the current scale and rotation.
    pub fn get_size(&self) -> (f64, f64) {
        let mut width: f64 = 0.0;
        let mut height: f64 = 0.0;

        unsafe {
            ffi::poppler_page_get_size(
                self.0,
                &mut width as *mut f64 as *mut c_double,
                &mut height as *mut f64 as *mut c_double,
            )
        }

        (width, height)
    }

    /// Render the page to the given cairo context.
    ///
    /// This function is for rendering a page that will be displayed. If you want to render a
    /// page that will be printed use [`render_for_printing`](Self::render_for_printing) instead. Please see the
    /// Poppler documentation for that function for the differences between rendering to the screen
    /// and rendering to a printer.
    #[cfg(feature = "render")]
    pub fn render(&self, ctx: &cairo::Context) {
        let ctx_raw = ctx.to_raw_none();
        unsafe { ffi::poppler_page_render(self.0, ctx_raw) }
    }

    /// Render the page to the given cairo context for printing with POPPLER_PRINT_ALL flags selected.
    ///
    /// The difference between [`render`](Self::render) and this function is that some things get
    /// rendered differently between screens and printers:
    ///
    /// * PDF annotations get rendered according to their PopplerAnnotFlag value.
    ///   For example, POPPLER_ANNOT_FLAG_PRINT refers to whether an annotation is printed or not,
    ///   whereas POPPLER_ANNOT_FLAG_NO_VIEW refers to whether an annotation is invisible when displaying to the screen.
    /// * PDF supports "hairlines" of width 0.0, which often get rendered as having a width of 1
    ///   device pixel. When displaying on a screen, Cairo may render such lines wide so that
    ///   they are hard to see, and Poppler makes use of PDF's Stroke Adjust graphics parameter
    ///   to make the lines easier to see. However, when printing, Poppler is able to directly use a printer's pixel size instead.
    /// * Some advanced features in PDF may require an image to be rasterized before sending
    ///   off to a printer. This may produce raster images which exceed Cairo's limits.
    ///   The "printing" functions will detect this condition and try to down-scale the intermediate surfaces as appropriate.
    #[cfg(feature = "render")]
    pub fn render_for_printing(&self, ctx: &cairo::Context) {
        let ctx_raw = ctx.to_raw_none();
        unsafe { ffi::poppler_page_render_for_printing(self.0, ctx_raw) }
    }

    /// Retrieves the text of the page.
    pub fn get_text(&self) -> Option<String> {
        unsafe {
            let ptr = ffi::poppler_page_get_text(self.0);
            util::take_c_owned_string(ptr)
        }
    }

    // New: “complete” text extraction similar to pdftotext default behavior
    pub fn get_page_text(&self) -> Option<String> {
        unsafe {
            // Prefer content bounding box if available
            /*let mut bbox = ffi::PopplerRectangle { x1: 0.0, y1: 0.0, x2: 0.0, y2: 0.0 };
            let has_bbox = ffi::poppler_page_get_bounding_box(self.0, &mut bbox);

            if has_bbox == glib::ffi::GTRUE {
                let ptr = ffi::poppler_page_get_selected_text(
                    self.0,
                    ffi::PopplerSelectionStyle::Line,
                    &mut bbox,
                );

                if !ptr.is_null() {
                    return util::take_c_owned_string(ptr);
                }
            }*/

            // Fallback: full page rectangle
            let (w, h) = self.get_size();
            let mut full = ffi::PopplerRectangle { x1: 0.0, y1: 0.0, x2: w, y2: h };
            let ptr = ffi::poppler_page_get_selected_text(
                self.0,
                ffi::PopplerSelectionStyle::Line,
                &mut full,
            );
            util::take_c_owned_string(ptr)
        }
    }

    /// Low-level helper: returns the UTF-8 string of the page **and**
    /// a parallel vector with the bounding rectangle for *each*
    /// Unicode scalar in that string.  You can build your own layout
    /// algorithms on top of this if you wish.
    pub fn extract_text_layout(&self) -> Option<(String, Vec<ffi::PopplerRectangle>)>
    {
        unsafe {
            // 1. grab raw text (always free afterwards!)
            let text_ptr = ffi::poppler_page_get_text(self.0);
            if text_ptr.is_null() {
                return None;
            }
            let text = util::take_c_owned_string(text_ptr)?;

            // 2. grab rectangles (one per UTF-8 *scalar*)
            let mut rects_ptr: *mut ffi::PopplerRectangle = std::ptr::null_mut();
            let mut n_rects: c_uint = 0;
            let ok = ffi::poppler_page_get_text_layout(
                self.0,
                &mut rects_ptr,
                &mut n_rects,
            );
            if ok == glib::ffi::GFALSE || n_rects == 0 {
                // still return the plain text so callers can fall back
                return Some((text, Vec::new()));
            }

            let rects = util::take_c_owned_rects(
                rects_ptr,
                n_rects as usize,
            );

            Some((text, rects))
        }
    }

    /// High-level, “pdftotext-like” extraction that
    ///  * groups characters into visual lines,
    ///  * inserts spaces when glyph gaps are large enough,
    ///  * keeps right-to-left scripts in correct reading order.
    ///
    /// The heuristics are a **straight port** of the ones used in
    /// poppler’s original TextOutputDev.
    pub fn get_text_full(&self) -> Option<String> {
        let (text, rects) = self.extract_text_layout()?;
        if rects.is_empty() {
            // rare – but GLib sometimes cannot give us rectangles
            // (e.g. for documents that are *only* form XObjects)
            return Some(text);
        }

        // ------- 1. zip text + rectangles into one vector ----------
        let mut glyphs: Vec<(char, ffi::PopplerRectangle)> = text
            .chars()
            .zip(rects)
            .collect();

        // sort *visually* – poppler page coords start at (0,0) in the
        // lower left corner, so bigger y == higher on page
        glyphs.sort_by(|(_, a), (_, b)| {
            let ay = (a.y1 + a.y2) / 2.0;
            let by = (b.y1 + b.y2) / 2.0;

            by.partial_cmp(&ay).unwrap_or(std::cmp::Ordering::Equal)
        });

        // average character height – used as the line merge threshold
        let sum_height = glyphs
            .iter()
            .map(|(_, r)| (r.y2 - r.y1).abs())
            .sum::<f64>();
        let avg_height = sum_height / glyphs.len() as f64;

        let mut result = String::new();
        let mut current_line: Vec<(char, ffi::PopplerRectangle)> = Vec::new();
        let mut current_base_y: f64 = f64::NAN;

        let flush_line = |line: &mut Vec<(char, ffi::PopplerRectangle)>, out: &mut String| {
            if line.is_empty() {
                return;
            }

            // Decide text direction – a very light-weight heuristic:
            let rtl_count = line
                .iter()
                .filter(|(c, _)| {
                    matches!(
                        unicode_bidi::bidi_class(*c),
                        unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL
                    )
                })
                .count();
            let ltr = rtl_count * 2 < line.len(); // majority rule

            // sort X
            line.sort_by(|(_, a), (_, b)| {
                if ltr {
                    a.x1.partial_cmp(&b.x1).unwrap()
                } else {
                    b.x2.partial_cmp(&a.x2).unwrap()
                }
            });

            // build the line – insert spaces for large gaps
            let mut prev: Option<&ffi::PopplerRectangle> = None;
            for (ch, rect) in line.iter() {
                if let Some(prev_rect) = prev {
                    let gap = if ltr {
                        rect.x1 - prev_rect.x2
                    } else {
                        prev_rect.x1 - rect.x2
                    };
                    if gap > (prev_rect.x2 - prev_rect.x1) * 0.5 {
                        out.push(' ');
                    }
                }
                out.push(*ch);
                prev = Some(rect);
            }
            out.push('\n');
            line.clear();
        };

        // --------- 2. line breaking -------------------------------
        for (ch, rect) in glyphs.into_iter() {
            let mid_y = (rect.y1 + rect.y2) / 2.0;
            if current_line.is_empty() {
                current_base_y = mid_y;
            }
            // new line if the vertical distance is more than half an
            // average glyph height
            if (current_base_y - mid_y).abs() > avg_height * 0.5 {
                flush_line(&mut current_line, &mut result);
                current_base_y = mid_y;
            }
            current_line.push((ch, rect));
        }
        // flush the last line
        flush_line(&mut current_line, &mut result);

        Some(result)
    }

    /// Returns every character on the page together with an optional
    /// bounding rectangle (newlines and other non-printable chars have
    /// `None`).  Always succeeds if Poppler could retrieve *any* text.
    fn glyph_stream(
        &self,
    ) -> Option<(String, Vec<Option<ffi::PopplerRectangle>>)> {
        unsafe {
            // --- raw UTF-8 text -------------------------------------------------
            let txt_ptr = ffi::poppler_page_get_text(self.0);
            let text = util::take_c_owned_string(txt_ptr)?;

            // --- rectangles (one per *glyph*, no entries for '\n') --------------
            let mut rect_ptr: *mut ffi::PopplerRectangle = std::ptr::null_mut();
            let mut n: c_uint = 0;
            ffi::poppler_page_get_text_layout(self.0, &mut rect_ptr, &mut n);

            let rects = util::take_c_owned_rects(rect_ptr, n as usize);

            // map glyph rectangles into the character stream  -------------------
            let mut mapped: Vec<Option<ffi::PopplerRectangle>> =
                Vec::with_capacity(text.chars().count());

            let mut r_it = rects.into_iter();
            for ch in text.chars() {
                if ch == '\n' || ch == '\r' {
                    mapped.push(None);
                } else {
                    mapped.push(r_it.next());
                }
            }
            Some((text, mapped))
        }
    }

    /// *Exact* visual layout (equivalent to `pdftotext -layout`).
    ///
    /// • preserves line-breaks inserted by Poppler  
    /// • inserts the correct number of blank spaces by measuring glyph gaps  
    /// • keeps the original character order (no extra bidi processing)
    pub fn get_text_layout_exact(&self) -> Option<String> {
        let (raw_text, glyphs) = self.glyph_stream()?;

        // mean glyph width – used as space unit
        let mut w_sum = 0.0;
        let mut w_cnt = 0usize;

        for r in glyphs.iter().flatten() {
            w_sum += (r.x2 - r.x1).abs();
            w_cnt += 1;
        }

        let avg_w = if w_cnt == 0 { 5.0 } else { w_sum / w_cnt as f64 };

        // build output ----------------------------------------------------------
        let mut out = String::with_capacity(raw_text.len() + 32);
        let mut prev_rect: Option<ffi::PopplerRectangle> = None;

        for (ch, rect_opt) in raw_text.chars().zip(glyphs.into_iter()) {
            match ch {
                '\n' | '\r' => {
                    out.push('\n');
                    prev_rect = None;
                }

                _ => {
                    // spacing: only when both current AND previous have rects
                    if let (Some(prev), Some(rect)) = (prev_rect, rect_opt) {
                        let gap = (rect.x1 - prev.x2).abs();

                        // number of spaces needed to approximate the gap
                        let mut n_spaces = ((gap / avg_w) + 0.5 /* round */) as usize;

                        // Poppler already emits real space glyphs – do not
                        // double-insert when the current character *is* one.
                        if ch == ' ' { n_spaces = n_spaces.saturating_sub(1); }

                        for _ in 0..n_spaces {
                            out.push(' ');
                        }
                    }

                    out.push(ch);
                    prev_rect = rect_opt;
                }
            }
        }

        Some(out)
    }
}

#[derive(Debug)]
pub struct PagesIter<'a> {
    total: usize,
    index: usize,
    doc: &'a PopplerDocument,
}

impl<'a> Iterator for PagesIter<'a> {
    type Item = PopplerPage;

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

impl Drop for PopplerPage {
    fn drop(&mut self) {
        unsafe {
            gobject_sys::g_object_unref(self.0 as *mut gobject_sys::GObject);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PopplerDocument;
    use crate::PopplerPage;
    #[cfg(feature = "render")]
    use cairo::Context;
    #[cfg(feature = "render")]
    use cairo::Format;
    #[cfg(feature = "render")]
    use cairo::ImageSurface;
    use std::{fs::File, io::Read};

    #[test]
    fn test1() {
        let filename = "test.pdf";
        let doc = PopplerDocument::new_from_file(filename, None).unwrap();
        let num_pages = doc.get_n_pages();

        println!("Document has {} page(s)", num_pages);

        #[cfg(feature = "render")]
        let surface = cairo::PdfSurface::new(420.0, 595.0, "output.pdf").unwrap();
        #[cfg(feature = "render")]
        let ctx = Context::new(&surface).unwrap();

        // FIXME: move iterator to poppler
        for page_num in 0..num_pages {
            let page = doc.get_page(page_num).unwrap();
            let (w, h) = page.get_size();
            println!("page {} has size {}, {}", page_num, w, h);
            #[cfg(feature = "render")]
            (|page: &PopplerPage, ctx: &Context| {
                surface.set_size(w, h).unwrap();
                ctx.save().unwrap();
                page.render(ctx);
            })(&page, &ctx);

            println!("Text: {:?}", page.get_text().unwrap_or(""));
            #[cfg(feature = "render")]
            (|ctx: &Context| {
                ctx.restore().unwrap();
                ctx.show_page().unwrap();
            })(&ctx);
        }
        //surface.write_to_png("file.png");
        #[cfg(feature = "render")]
        surface.finish();
    }

    #[test]
    fn test2_from_file() {
        let path = "test.pdf";
        let doc: PopplerDocument = PopplerDocument::new_from_file(path, Some("upw")).unwrap();
        let num_pages = doc.get_n_pages();
        let title = doc.get_title().unwrap();
        let metadata = doc.get_metadata();
        let version_string = doc.get_pdf_version_string();
        let permissions = doc.get_permissions();
        let page: PopplerPage = doc.get_page(0).unwrap();
        let (w, h) = page.get_size();

        println!(
            "Document {} has {} page(s) and is {}x{}",
            title, num_pages, w, h
        );
        println!(
            "Version: {:?}, Permissions: {:x?}",
            version_string, permissions
        );

        assert!(metadata.is_some());
        assert_eq!(version_string, Some("PDF-1.3".to_string()));
        assert_eq!(permissions, 0xff);

        assert_eq!(title, "This is a test PDF file");

        #[cfg(feature = "render")]
        let surface = ImageSurface::create(Format::ARgb32, w as i32, h as i32).unwrap();
        #[cfg(feature = "render")]
        let ctx = Context::new(&surface).unwrap();

        #[cfg(feature = "render")]
        (|page: &PopplerPage, ctx: &Context| {
            ctx.save().unwrap();
            page.render(ctx);
            ctx.restore().unwrap();
            ctx.show_page().unwrap();
        })(&page, &ctx);

        #[cfg(feature = "render")]
        let mut f: File = File::create("out.png").unwrap();
        #[cfg(feature = "render")]
        surface.write_to_png(&mut f).expect("Unable to write PNG");
    }
    #[test]
    fn test2_from_data() {
        let path = "test.pdf";
        let mut file = File::open(path).unwrap();
        let mut data: Vec<u8> = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let doc: PopplerDocument =
            PopplerDocument::new_from_data(&mut data[..], Some("upw")).unwrap();
        let num_pages = doc.get_n_pages();
        let title = doc.get_title().unwrap();
        let metadata = doc.get_metadata();
        let version_string = doc.get_pdf_version_string();
        let permissions = doc.get_permissions();
        let page: PopplerPage = doc.get_page(0).unwrap();
        let (w, h) = page.get_size();

        println!(
            "Document {} has {} page(s) and is {}x{}",
            title, num_pages, w, h
        );
        println!(
            "Version: {:?}, Permissions: {:x?}",
            version_string, permissions
        );

        assert!(metadata.is_some());
        assert_eq!(version_string, Some("PDF-1.3".to_string()));
        assert_eq!(permissions, 0xff);
    }

    #[test]
    fn test3() {
        let mut data = vec![];

        assert!(PopplerDocument::new_from_data(&mut data[..], Some("upw")).is_err());
    }

    #[test]
    fn test4() {
        let path = "test.pdf";
        let doc: PopplerDocument = PopplerDocument::new_from_file(path, None).unwrap();
        let total = doc.get_n_pages();

        let mut count = 0;
        let src_w = 595f64;
        let src_h = 842f64;

        for (index, p) in doc.pages().enumerate() {
            let (w, h) = p.get_size();
            assert_eq!(w, src_w);
            assert_eq!(h, src_h);

            println!("page {}/{} -- {}x{}", index + 1, total, w, h);
            count += 1;
        }

        assert_eq!(count, 1);

        #[cfg(feature = "render")]
        let mut count = 0;

        #[cfg(feature = "render")]
        for (index, p) in doc.pages().enumerate() {
            let (w, h) = p.get_size();

            assert_eq!(w, src_w);
            assert_eq!(h, src_h);

            let surface = ImageSurface::create(Format::ARgb32, w as i32, h as i32).expect("failed to create image surface");
            let context = Context::new(&surface).expect("failed to create cairo context");
            (|page: &PopplerPage, ctx: &Context| {
                ctx.save().unwrap();
                page.render(ctx);
                ctx.restore().unwrap();
                ctx.show_page().unwrap();
            })(&p, &context);
            let mut f: File = File::create(format!("out{}.png", index)).unwrap();
            surface.write_to_png(&mut f).expect("Unable to write PNG");

            count += 1;
        }

        #[cfg(feature = "render")]
        assert_eq!(count, 1)
    }
}

