// This version is updated to match the modern C++ API of Poppler,
// resolving specific type mismatches identified during compilation.

#include <PDFDoc.h>
#include <PDFDocFactory.h>
#include <TextOutputDev.h>
#include <goo/GooString.h>
#include <glib.h>
#include <memory>
#include <optional>

// Headers for creating a document from a memory buffer
#include <Object.h>
#include <Stream.h>

// Include the poppler version header to get version macros
#include <poppler-version.h>

extern "C" {
    /**
     * @brief Extracts text from a page of a PDF file, preserving physical layout.
     *
     * @param uri The file URI (e.g., "file:///path/to/doc.pdf").
     * @param password The user password for the PDF, or nullptr if none.
     * @param page_index The 0-based index of the page to extract.
     * @return A C-string containing the extracted text, allocated with g_strdup.
     * The caller is responsible for freeing this string with g_free.
     * Returns nullptr on failure.
     */
    char* extract_text_layout_from_file(const char* uri, const char* password, int page_index) {
        if (!uri) {
            return nullptr;
        }

        try {
            std::string path_str(uri);
            if (path_str.rfind("file://", 0) == 0) {
                path_str.erase(0, 7);
            }

            GooString file_name(path_str.c_str());
            std::optional<GooString> user_pw;
            if (password && password[0] != '\0') {
                user_pw.emplace(password);
            }

            // The factory returns a std::unique_ptr, which correctly manages the memory.
            std::unique_ptr<PDFDoc> doc = PDFDocFactory().createPDFDoc(file_name, user_pw, {});

            if (!doc || !doc->isOk()) {
                return nullptr;
            }

            int page_num = page_index + 1;
            if (page_num <= 0 || page_num > doc->getNumPages()) {
                return nullptr;
            }

            TextOutputDev text_dev(nullptr, true, 2.0, false, false);
            if (!text_dev.isOk()) {
                return nullptr;
            }

            doc->displayPage(&text_dev, page_num, 72.0, 72.0, 0, false, true, false);

            double page_w = doc->getPageMediaWidth(page_num);
            double page_h = doc->getPageMediaHeight(page_num);

            GooString text = text_dev.getText(0, 0, page_w, page_h);
            char *result = nullptr;
            if (text.c_str()) {
                result = g_strdup(text.c_str());
            }

            return result;
        } catch (...) {
            return nullptr;
        }
    }

    /**
     * @brief Extracts text from a page of a PDF from a memory buffer, preserving layout.
     *
     * @param data Pointer to the start of the PDF data buffer.
     * @param length The length of the data buffer.
     * @param password The user password for the PDF, or nullptr if none.
     * @param page_index The 0-based index of the page to extract.
     * @return A C-string with the extracted text, allocated with g_strdup.
     * The caller must free it with g_free. Returns nullptr on failure.
     */
    char* extract_text_layout_from_data(const char* data, int length, const char* password, int page_index) {
        if (!data || length <= 0) {
            return nullptr;
        }

        try {
            std::optional<GooString> user_pw;
            if (password && password[0] != '\0') {
                user_pw.emplace(password);
            }
            
            // Create a null object for the MemStream dictionary.
            // The API for this has changed multiple times in poppler's history.
            // Workaround for inconsistent poppler packaging in Debian Trixie.
            // Version 25.03.0 reports a new version number but contains old headers.
            #if (POPPLER_VERSION_MAJOR == 25 && POPPLER_VERSION_MINOR == 3)
                // SPECIAL CASE: Trixie's 25.03 uses the 22.04-24.03 API.
                BaseStream *stream = new MemStream((char*)data, 0, length, Object(objNull));
            #elif (POPPLER_VERSION_MAJOR > 24) || (POPPLER_VERSION_MAJOR == 24 && POPPLER_VERSION_MINOR >= 4)
                // Newest API (24.04+): Object::null() was re-introduced.
                BaseStream *stream = new MemStream((char*)data, 0, length, Object::null());
            #elif (POPPLER_VERSION_MAJOR > 22) || (POPPLER_VERSION_MAJOR == 22 && POPPLER_VERSION_MINOR >= 4)
                // API from 22.04 to 24.03: Use Object(Object::nullObj).
                BaseStream *stream = new MemStream((char*)data, 0, length, Object(objNull));
            #elif (POPPLER_VERSION_MAJOR > 22) || (POPPLER_VERSION_MAJOR == 22 && POPPLER_VERSION_MINOR >= 1)
                // API from 22.01 to 22.03: Use the default constructor.
                BaseStream *stream = new MemStream((char*)data, 0, length, Object());
            #else
                // Old API (< 22.01): Use the original Object::null().
                BaseStream *stream = new MemStream((char*)data, 0, length, Object::null());
            #endif

            // The PDFDoc constructor for streams also takes std::optional for passwords.
            std::unique_ptr<PDFDoc> doc(new PDFDoc(stream, user_pw, {}));

            if (!doc || !doc->isOk()) {
                return nullptr;
            }

            int page_num = page_index + 1;
            if (page_num <= 0 || page_num > doc->getNumPages()) {
                return nullptr;
            }

            TextOutputDev text_dev(nullptr, true, 2.0, false, false);
            if (!text_dev.isOk()) {
                return nullptr;
            }

            doc->displayPage(&text_dev, page_num, 72.0, 72.0, 0, false, true, false);

            double page_w = doc->getPageMediaWidth(page_num);
            double page_h = doc->getPageMediaHeight(page_num);

            GooString text = text_dev.getText(0, 0, page_w, page_h);

            char *result = nullptr;
            if (text.c_str()) {
                result = g_strdup(text.c_str());
            }

            // The unique_ptr for doc will handle deletion, which also deletes the stream.
            return result;
        } catch (...) {
            return nullptr;
        }
    }
}
