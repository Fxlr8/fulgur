use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

const IDENTITY: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Debug, Serialize, PartialEq)]
pub struct InspectResult {
    pub pages: u32,
    pub metadata: Metadata,
    pub text_items: Vec<TextItem>,
    pub images: Vec<ImageItem>,
}

#[derive(Debug, Serialize, PartialEq, Default)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct TextItem {
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub text: String,
    pub font: String,
    pub font_size: f32,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ImageItem {
    pub page: u32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub format: String,
    pub width_px: u32,
    pub height_px: u32,
}

pub fn inspect(path: &Path) -> crate::Result<InspectResult> {
    let doc = lopdf::Document::load(path)
        .map_err(|e| crate::Error::Other(format!("Failed to load PDF: {e}")))?;

    let pages = doc.get_pages().len() as u32;
    let metadata = extract_metadata(&doc);
    let text_items = extract_text_items(&doc)?;
    let images = extract_image_items(&doc)?;

    Ok(InspectResult {
        pages,
        metadata,
        text_items,
        images,
    })
}

fn obj_as_name_str(obj: &lopdf::Object) -> Option<&str> {
    obj.as_name().ok().and_then(|b| std::str::from_utf8(b).ok())
}

fn extract_metadata(doc: &lopdf::Document) -> Metadata {
    let mut meta = Metadata::default();
    let info_id = match doc.trailer.get(b"Info") {
        Ok(obj) => match obj.as_reference() {
            Ok(id) => id,
            Err(_) => return meta,
        },
        Err(_) => return meta,
    };
    let info = match doc.get_object(info_id) {
        Ok(lopdf::Object::Dictionary(d)) => d,
        _ => return meta,
    };

    let get_str = |dict: &lopdf::Dictionary, key: &[u8]| -> Option<String> {
        dict.get(key)
            .ok()
            .and_then(|o| o.as_str().ok())
            .map(decode_pdf_string)
    };

    meta.title = get_str(info, b"Title");
    meta.author = get_str(info, b"Author");
    meta.creator = get_str(info, b"Creator");
    meta.created_at = get_str(info, b"CreationDate");
    meta.modified_at = get_str(info, b"ModDate");
    meta
}

fn extract_text_items(doc: &lopdf::Document) -> crate::Result<Vec<TextItem>> {
    use lopdf::content::Operation;
    let mut items = Vec::new();

    for (&page_num, &page_id) in &doc.get_pages() {
        let content_bytes = match doc.get_page_content(page_id) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let content = match lopdf::content::Content::decode(&content_bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let identity = IDENTITY;
        // Graphics state stack: (CTM, font_name, font_size).
        // Tf is part of the graphics state (PDF §8.4.5 Table 52), so q/Q save/restore it.
        let mut gs_stack: Vec<([f32; 6], String, f32)> =
            vec![(identity, "unknown".to_string(), 12.0)];
        // Text matrix linear components (scale/rotation); updated by Tm, reset by BT.
        let mut tm_a: f32 = 1.0;
        let mut tm_b: f32 = 0.0;
        let mut tm_c: f32 = 0.0;
        let mut tm_d: f32 = 1.0;
        // Text line matrix translation in user space; updated by Tm/Td/TD/T*, reset by BT.
        let mut tlm_e: f32 = 0.0;
        let mut tlm_f: f32 = 0.0;
        // Current text origin in page space.
        let mut tx: f32 = 0.0;
        let mut ty: f32 = 0.0;
        let mut font_name = String::from("unknown");
        let mut font_size: f32 = 12.0;
        let mut text_leading: f32 = 0.0;

        for Operation { operator, operands } in &content.operations {
            match operator.as_str() {
                "q" => {
                    let top = gs_stack.last().expect("gs_stack non-empty").clone();
                    gs_stack.push(top);
                }
                "Q" if gs_stack.len() > 1 => {
                    gs_stack.pop();
                    let (_, ref saved_font, saved_size) =
                        *gs_stack.last().expect("gs_stack non-empty after Q");
                    font_name = saved_font.clone();
                    font_size = saved_size;
                }
                "cm" if operands.len() == 6 => {
                    let new_m = [
                        obj_to_f32(&operands[0]),
                        obj_to_f32(&operands[1]),
                        obj_to_f32(&operands[2]),
                        obj_to_f32(&operands[3]),
                        obj_to_f32(&operands[4]),
                        obj_to_f32(&operands[5]),
                    ];
                    if let Some(gs) = gs_stack.last_mut() {
                        gs.0 = concat_matrix(&gs.0, &new_m);
                    }
                }
                "Tf" => {
                    if let (Some(name_obj), Some(size)) = (operands.first(), operands.get(1)) {
                        font_name = obj_as_name_str(name_obj).unwrap_or("unknown").to_string();
                        font_size = obj_to_f32(size);
                        if let Some(gs) = gs_stack.last_mut() {
                            gs.1 = font_name.clone();
                            gs.2 = font_size;
                        }
                    }
                }
                "TL" if !operands.is_empty() => {
                    text_leading = obj_to_f32(&operands[0]);
                }
                // BT resets the text matrix and text line matrix to identity (PDF §9.4.1).
                // tx/ty are derived from tlm + CTM: text origin is the CTM translation.
                "BT" => {
                    tm_a = 1.0;
                    tm_b = 0.0;
                    tm_c = 0.0;
                    tm_d = 1.0;
                    tlm_e = 0.0;
                    tlm_f = 0.0;
                    let ctm = gs_stack.last().map(|gs| &gs.0).unwrap_or(&identity);
                    tx = ctm[4];
                    ty = ctm[5];
                }
                "Tm" if operands.len() >= 6 => {
                    tm_a = obj_to_f32(&operands[0]);
                    tm_b = obj_to_f32(&operands[1]);
                    tm_c = obj_to_f32(&operands[2]);
                    tm_d = obj_to_f32(&operands[3]);
                    tlm_e = obj_to_f32(&operands[4]);
                    tlm_f = obj_to_f32(&operands[5]);
                    let ctm = gs_stack.last().map(|gs| &gs.0).unwrap_or(&identity);
                    tx = ctm[0] * tlm_e + ctm[2] * tlm_f + ctm[4];
                    ty = ctm[1] * tlm_e + ctm[3] * tlm_f + ctm[5];
                }
                // Td/TD advances the text line matrix in text space (PDF §9.4.2).
                // The offset (dx, dy) is in text coordinates; multiply through the
                // linear part of the text matrix to get user-space displacement.
                // TD also sets the text leading to -dy (PDF §9.4.2).
                "Td" | "TD" if operands.len() >= 2 => {
                    let dx = obj_to_f32(&operands[0]);
                    let dy = obj_to_f32(&operands[1]);
                    if operator == "TD" {
                        text_leading = -dy;
                    }
                    tlm_e += dx * tm_a + dy * tm_c;
                    tlm_f += dx * tm_b + dy * tm_d;
                    let ctm = gs_stack.last().map(|gs| &gs.0).unwrap_or(&identity);
                    tx = ctm[0] * tlm_e + ctm[2] * tlm_f + ctm[4];
                    ty = ctm[1] * tlm_e + ctm[3] * tlm_f + ctm[5];
                }
                // T* ≡ Td 0 -text_leading (PDF §9.4.2).
                "T*" => {
                    tlm_e += (-text_leading) * tm_c;
                    tlm_f += (-text_leading) * tm_d;
                    let ctm = gs_stack.last().map(|gs| &gs.0).unwrap_or(&identity);
                    tx = ctm[0] * tlm_e + ctm[2] * tlm_f + ctm[4];
                    ty = ctm[1] * tlm_e + ctm[3] * tlm_f + ctm[5];
                }
                "Tj" => {
                    if let Some(text_obj) = operands.first() {
                        if let Ok(bytes) = text_obj.as_str() {
                            let text = decode_pdf_string(bytes);
                            if !text.trim().is_empty() {
                                let w = estimate_width(&text, font_size);
                                items.push(TextItem {
                                    page: page_num,
                                    x: tx,
                                    y: ty,
                                    width: w,
                                    height: font_size,
                                    text,
                                    font: font_name.clone(),
                                    font_size,
                                });
                                tx += w;
                            }
                        }
                    }
                }
                "TJ" => {
                    if let Some(array_obj) = operands.first() {
                        if let Ok(array) = array_obj.as_array() {
                            let mut combined = String::new();
                            for elem in array {
                                if let Ok(bytes) = elem.as_str() {
                                    combined.push_str(&decode_pdf_string(bytes));
                                }
                            }
                            if !combined.trim().is_empty() {
                                let w = estimate_width(&combined, font_size);
                                items.push(TextItem {
                                    page: page_num,
                                    x: tx,
                                    y: ty,
                                    width: w,
                                    height: font_size,
                                    text: combined,
                                    font: font_name.clone(),
                                    font_size,
                                });
                                tx += w;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(items)
}

fn resolve_page_resources(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Option<lopdf::Dictionary> {
    // Walk the page's Parent chain to find an inherited Resources dictionary.
    // Track visited object IDs so malformed inputs with cyclic /Parent references
    // (e.g. A -> B -> A) cannot spin this loop indefinitely.
    let mut visited = HashSet::new();
    let mut current_id = page_id;
    loop {
        if !visited.insert(current_id) {
            return None;
        }
        let dict = match doc.get_object(current_id) {
            Ok(lopdf::Object::Dictionary(d)) => d.clone(),
            _ => return None,
        };
        if let Ok(res) = dict.get(b"Resources") {
            if let Ok((_, lopdf::Object::Dictionary(resources))) = doc.dereference(res) {
                return Some(resources.clone());
            }
        }
        match dict.get(b"Parent").and_then(|p| p.as_reference()) {
            Ok(parent_id) => current_id = parent_id,
            Err(_) => return None,
        }
    }
}

fn transform_point(m: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

fn extract_image_items(doc: &lopdf::Document) -> crate::Result<Vec<ImageItem>> {
    let mut items = Vec::new();

    for (&page_num, &page_id) in &doc.get_pages() {
        // Step 1: XObject から画像情報を一時 map に収集
        // Resources は親 /Pages ノードから継承される場合があるため、継承チェーンを辿る。
        // key = XObject name, value = (format, width_px, height_px)
        let mut image_xobjects: std::collections::BTreeMap<String, (String, u32, u32)> =
            std::collections::BTreeMap::new();

        if let Some(resources) = resolve_page_resources(doc, page_id) {
            if let Ok(xo) = resources.get(b"XObject") {
                if let Ok((_, lopdf::Object::Dictionary(xobjects))) = doc.dereference(xo) {
                    for (name, obj_ref) in xobjects.iter() {
                        if let Ok((_, lopdf::Object::Stream(xobj))) = doc.dereference(obj_ref) {
                            let subtype = xobj
                                .dict
                                .get(b"Subtype")
                                .ok()
                                .and_then(|o| obj_as_name_str(o))
                                .unwrap_or_default();
                            if subtype == "Image" {
                                let fmt = detect_image_format(&xobj.dict);
                                let w_px = xobj
                                    .dict
                                    .get(b"Width")
                                    .ok()
                                    .and_then(|o| o.as_i64().ok())
                                    .unwrap_or(0) as u32;
                                let h_px = xobj
                                    .dict
                                    .get(b"Height")
                                    .ok()
                                    .and_then(|o| o.as_i64().ok())
                                    .unwrap_or(0) as u32;
                                let name_str = String::from_utf8_lossy(name).into_owned();
                                image_xobjects.insert(name_str, (fmt, w_px, h_px));
                            }
                        }
                    }
                }
            }
        }

        if image_xobjects.is_empty() {
            continue;
        }

        // Step 2: content stream から Do オペレータで位置を取得し、突き合わせて push
        let content_bytes = match doc.get_page_content(page_id) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let content = match lopdf::content::Content::decode(&content_bytes) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let identity = IDENTITY;
        let mut ctm_stack: Vec<[f32; 6]> = vec![identity];
        for op in &content.operations {
            match op.operator.as_str() {
                "q" => {
                    let top = *ctm_stack.last().unwrap_or(&identity);
                    ctm_stack.push(top);
                }
                "Q" if ctm_stack.len() > 1 => {
                    ctm_stack.pop();
                }
                "cm" if op.operands.len() == 6 => {
                    let new_m = [
                        obj_to_f32(&op.operands[0]),
                        obj_to_f32(&op.operands[1]),
                        obj_to_f32(&op.operands[2]),
                        obj_to_f32(&op.operands[3]),
                        obj_to_f32(&op.operands[4]),
                        obj_to_f32(&op.operands[5]),
                    ];
                    let current = *ctm_stack.last().unwrap_or(&identity);
                    *ctm_stack.last_mut().unwrap() = concat_matrix(&current, &new_m);
                }
                "Do" => {
                    if let Some(name_obj) = op.operands.first() {
                        if let Some(name) = obj_as_name_str(name_obj) {
                            if let Some((fmt, w_px, h_px)) = image_xobjects.get(name) {
                                let ctm = ctm_stack.last().unwrap_or(&identity);
                                // PDF images occupy the unit square [0,1]x[0,1].
                                // Transform all 4 corners through the CTM and take
                                // the axis-aligned bounding box so rotated/sheared
                                // images produce correct width/height.
                                let corners = [
                                    transform_point(ctm, 0.0, 0.0),
                                    transform_point(ctm, 1.0, 0.0),
                                    transform_point(ctm, 0.0, 1.0),
                                    transform_point(ctm, 1.0, 1.0),
                                ];
                                let min_x = corners
                                    .iter()
                                    .map(|(x, _)| *x)
                                    .fold(f32::INFINITY, f32::min);
                                let max_x = corners
                                    .iter()
                                    .map(|(x, _)| *x)
                                    .fold(f32::NEG_INFINITY, f32::max);
                                let min_y = corners
                                    .iter()
                                    .map(|(_, y)| *y)
                                    .fold(f32::INFINITY, f32::min);
                                let max_y = corners
                                    .iter()
                                    .map(|(_, y)| *y)
                                    .fold(f32::NEG_INFINITY, f32::max);
                                items.push(ImageItem {
                                    page: page_num,
                                    x: min_x,
                                    y: min_y,
                                    width: max_x - min_x,
                                    height: max_y - min_y,
                                    format: fmt.clone(),
                                    width_px: *w_px,
                                    height_px: *h_px,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(items)
}

fn obj_to_f32(obj: &lopdf::Object) -> f32 {
    match obj {
        lopdf::Object::Integer(i) => *i as f32,
        lopdf::Object::Real(f) => *f,
        _ => 0.0,
    }
}

/// Concatenate two PDF transformation matrices.
///
/// PDF transformation matrices use the row-vector convention:
/// ```text
/// a c e
/// b d f
/// 0 0 1
/// ```
/// This function computes `M_result = M_new × M_current`.
fn concat_matrix(current: &[f32; 6], new: &[f32; 6]) -> [f32; 6] {
    let (a, b, c, d, e, f) = (new[0], new[1], new[2], new[3], new[4], new[5]);
    let (a2, b2, c2, d2, e2, f2) = (
        current[0], current[1], current[2], current[3], current[4], current[5],
    );
    [
        a * a2 + b * c2,
        a * b2 + b * d2,
        c * a2 + d * c2,
        c * b2 + d * d2,
        e * a2 + f * c2 + e2,
        e * b2 + f * d2 + f2,
    ]
}

/// Decode a PDF string to a Rust String.
///
/// Handles UTF-16 BE (BOM `\xFE\xFF`) strings. For all other strings,
/// falls back to treating each byte as a Latin-1 code point.
///
/// Note: fulgur-generated PDFs use CID fonts where text in the content
/// stream consists of glyph IDs, not Unicode code points. The decoded
/// text for such PDFs will appear as raw byte sequences, not readable text.
/// Full Unicode reconstruction requires ToUnicode CMap parsing, which is
/// not yet implemented.
fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let chars: Vec<u16> = bytes[2..]
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&chars);
    }
    bytes.iter().map(|&b| b as char).collect()
}

fn estimate_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.5
}

fn detect_image_format(dict: &lopdf::Dictionary) -> String {
    if let Ok(filter) = dict.get(b"Filter") {
        let name = match filter {
            lopdf::Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
            lopdf::Object::Array(arr) => arr
                .last()
                .and_then(|o| obj_as_name_str(o))
                .unwrap_or("")
                .to_string(),
            _ => String::new(),
        };
        match name.as_str() {
            "DCTDecode" => return "jpeg".to_string(),
            "JPXDecode" => return "jp2".to_string(),
            "CCITTFaxDecode" => return "tiff".to_string(),
            "FlateDecode" => return "flate".to_string(),
            _ => {}
        }
    }
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_test_pdf(html: &str) -> Vec<u8> {
        crate::engine::Engine::builder()
            .build()
            .render(html)
            .unwrap()
    }

    fn inspect_bytes(bytes: &[u8]) -> InspectResult {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), bytes).unwrap();
        inspect(tmp.path()).unwrap()
    }

    #[test]
    fn inspect_page_count() {
        let pdf = render_test_pdf("<html><body><p>Hello</p></body></html>");
        let result = inspect_bytes(&pdf);
        assert_eq!(result.pages, 1);
    }

    #[test]
    fn inspect_metadata_title() {
        let pdf = crate::engine::Engine::builder()
            .title("Test Title".to_string())
            .build()
            .render("<html><body><p>Hi</p></body></html>")
            .unwrap();
        let result = inspect_bytes(&pdf);
        assert_eq!(result.metadata.title.as_deref(), Some("Test Title"));
    }

    #[test]
    fn inspect_text_items_non_empty() {
        let pdf = render_test_pdf("<html><body><p>Hello World</p></body></html>");
        let result = inspect_bytes(&pdf);
        assert!(!result.text_items.is_empty(), "expected text items");
    }

    #[test]
    fn inspect_text_item_fields() {
        let pdf = render_test_pdf("<html><body><p>Hello</p></body></html>");
        let result = inspect_bytes(&pdf);
        let item = result
            .text_items
            .first()
            .expect("text items should not be empty");
        assert!(item.page >= 1);
        assert!(item.font_size > 0.0);
        assert!(!item.text.is_empty());
    }

    #[test]
    fn inspect_result_serializes_to_json() {
        let pdf = render_test_pdf("<html><body><p>Test</p></body></html>");
        let result = inspect_bytes(&pdf);
        let json = serde_json::to_string_pretty(&result).unwrap();
        assert!(json.contains("\"pages\""));
        assert!(json.contains("\"metadata\""));
        assert!(json.contains("\"text_items\""));
        assert!(json.contains("\"images\""));
    }

    #[test]
    fn inspect_error_on_nonexistent_file() {
        let result = inspect(std::path::Path::new("/nonexistent/path/to.pdf"));
        assert!(result.is_err(), "expected error for nonexistent file");
    }

    #[test]
    fn inspect_multi_page_pdf() {
        // Force two pages by making content taller than a single A4 page
        let html = "<html><body>\
            <p style='margin-bottom:2000pt'>Page one</p>\
            <p>Page two</p>\
            </body></html>";
        let pdf = render_test_pdf(html);
        let result = inspect_bytes(&pdf);
        assert!(result.pages >= 2, "expected at least 2 pages");
    }

    #[test]
    fn inspect_metadata_all_fields() {
        let pdf = crate::engine::Engine::builder()
            .title("My Title".to_string())
            .authors(vec!["Alice".to_string()])
            .creator("TestApp".to_string())
            .build()
            .render("<html><body><p>x</p></body></html>")
            .unwrap();
        let result = inspect_bytes(&pdf);
        assert_eq!(result.metadata.title.as_deref(), Some("My Title"));
        assert_eq!(result.metadata.author.as_deref(), Some("Alice"));
        assert_eq!(result.metadata.creator.as_deref(), Some("TestApp"));
    }

    #[test]
    fn inspect_image_embedded() {
        // Generate a valid 4x4 red PNG via the image crate (already a dev-dep)
        let img = image::RgbImage::from_fn(4, 4, |_, _| image::Rgb([255u8, 0, 0]));
        let mut png_bytes = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        let mut bundle = crate::asset::AssetBundle::new();
        bundle.add_image("test.png", png_bytes);
        let pdf = crate::engine::Engine::builder()
            .assets(bundle)
            .build()
            .render(r#"<html><body><img src="test.png" width="50" height="50"></body></html>"#)
            .unwrap();
        let result = inspect_bytes(&pdf);
        assert!(!result.images.is_empty(), "expected at least one image");
        let img = &result.images[0];
        assert_eq!(img.page, 1);
        assert!(img.width > 0.0, "image width should be positive");
        assert!(img.height > 0.0, "image height should be positive");
    }

    // --- pure function unit tests ---

    #[test]
    fn decode_pdf_string_latin1() {
        let bytes = b"Hello";
        assert_eq!(decode_pdf_string(bytes), "Hello");
    }

    #[test]
    fn decode_pdf_string_utf16be() {
        // UTF-16 BE BOM + "Hi" (U+0048, U+0069)
        let bytes = &[0xFE, 0xFF, 0x00, 0x48, 0x00, 0x69];
        assert_eq!(decode_pdf_string(bytes), "Hi");
    }

    #[test]
    fn decode_pdf_string_utf16be_odd_trailing_byte_ignored() {
        // BOM + one complete pair + one orphan byte
        let bytes = &[0xFE, 0xFF, 0x00, 0x41, 0xFF];
        let s = decode_pdf_string(bytes);
        assert_eq!(s, "A"); // orphan byte filtered by chunks(2) + len==2
    }

    #[test]
    fn detect_image_format_jpeg() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Name(b"DCTDecode".to_vec()));
        assert_eq!(detect_image_format(&dict), "jpeg");
    }

    #[test]
    fn detect_image_format_flate() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Name(b"FlateDecode".to_vec()));
        assert_eq!(detect_image_format(&dict), "flate");
    }

    #[test]
    fn detect_image_format_jp2() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Name(b"JPXDecode".to_vec()));
        assert_eq!(detect_image_format(&dict), "jp2");
    }

    #[test]
    fn detect_image_format_tiff() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Name(b"CCITTFaxDecode".to_vec()));
        assert_eq!(detect_image_format(&dict), "tiff");
    }

    #[test]
    fn detect_image_format_unknown() {
        let dict = lopdf::Dictionary::new(); // no Filter key
        assert_eq!(detect_image_format(&dict), "unknown");
    }

    #[test]
    fn detect_image_format_array_filter() {
        // Array filter — last entry wins
        let mut dict = lopdf::Dictionary::new();
        dict.set(
            b"Filter",
            lopdf::Object::Array(vec![
                lopdf::Object::Name(b"ASCII85Decode".to_vec()),
                lopdf::Object::Name(b"DCTDecode".to_vec()),
            ]),
        );
        assert_eq!(detect_image_format(&dict), "jpeg");
    }

    #[test]
    fn concat_matrix_identity() {
        let id = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let m = [1.0f32, 0.0, 0.0, 1.0, 10.0, 20.0];
        let result = concat_matrix(&id, &m);
        // id × m = m
        assert!((result[4] - 10.0).abs() < 1e-4);
        assert!((result[5] - 20.0).abs() < 1e-4);
    }

    #[test]
    fn concat_matrix_translation() {
        let a = [1.0f32, 0.0, 0.0, 1.0, 5.0, 10.0];
        let b = [1.0f32, 0.0, 0.0, 1.0, 3.0, 4.0];
        let result = concat_matrix(&a, &b);
        // Translations add: e = 3+5=8, f = 4+10=14
        assert!((result[4] - 8.0).abs() < 1e-4);
        assert!((result[5] - 14.0).abs() < 1e-4);
    }

    // --- obj_to_f32 edge case ---

    #[test]
    fn obj_to_f32_returns_zero_for_non_numeric_object() {
        // The `_ => 0.0` arm is hit when the Object is neither Integer nor Real
        // (e.g., a Name or Null). This covers the fallback branch.
        assert_eq!(obj_to_f32(&lopdf::Object::Null), 0.0);
        assert_eq!(obj_to_f32(&lopdf::Object::Boolean(true)), 0.0);
        assert_eq!(obj_to_f32(&lopdf::Object::Name(b"F1".to_vec())), 0.0);
        // The covered variants
        assert_eq!(obj_to_f32(&lopdf::Object::Integer(5)), 5.0);
        assert!((obj_to_f32(&lopdf::Object::Real(2.5)) - 2.5).abs() < 1e-4);
    }

    // --- detect_image_format edge cases ---

    #[test]
    fn detect_image_format_unrecognized_name_filter() {
        // Filter is a Name but not one of the four recognized values.
        // Hits the `_ => {}` arm in the inner match, then falls through to "unknown".
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Name(b"ASCII85Decode".to_vec()));
        assert_eq!(detect_image_format(&dict), "unknown");
    }

    #[test]
    fn detect_image_format_non_name_non_array_filter_object() {
        // Filter is neither a Name nor an Array (e.g., an Integer).
        // Hits the `_ => String::new()` arm in the outer match.
        let mut dict = lopdf::Dictionary::new();
        dict.set(b"Filter", lopdf::Object::Integer(1));
        assert_eq!(detect_image_format(&dict), "unknown");
    }

    // --- lopdf-constructed PDF: TL / Td / TD / T* text-positioning operators ---

    /// Build a minimal lopdf-native PDF whose content stream uses the
    /// TL / Td / TD / T* text-positioning operators.  fulgur-generated PDFs
    /// use Tm/Tj for all text positioning, so these operator paths in
    /// `extract_text_items` are only reachable via synthetically crafted PDFs.
    fn make_pdf_with_text_positioning_ops() -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::{Document, Object, Stream, dictionary};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Courier",
                },
            },
        });

        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new(
                    "Tf",
                    vec![Object::Name(b"F1".to_vec()), Object::Integer(12)],
                ),
                // TL sets text leading to 14 pt
                Operation::new("TL", vec![Object::Real(14.0)]),
                // Td moves text position
                Operation::new("Td", vec![Object::Real(100.0), Object::Real(700.0)]),
                Operation::new("Tj", vec![Object::string_literal("Line1")]),
                // TD moves and also resets leading to abs(-14) = 14
                Operation::new("TD", vec![Object::Real(0.0), Object::Real(-14.0)]),
                Operation::new("Tj", vec![Object::string_literal("Line2")]),
                // T* advances by the current text leading (set via TL / TD)
                Operation::new("T*", vec![]),
                Operation::new("Tj", vec![Object::string_literal("Line3")]),
                Operation::new("ET", vec![]),
            ],
        };

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(595),
                Object::Integer(842),
            ],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn inspect_text_operators_tl_td_td_tstar_produce_text_items() {
        let pdf = make_pdf_with_text_positioning_ops();
        let result = inspect_bytes(&pdf);
        // Three Tj calls → three text items (Line1 / Line2 / Line3)
        assert_eq!(
            result.text_items.len(),
            3,
            "expected 3 text items from Td/TD/T* positioned text"
        );
        let texts: Vec<&str> = result.text_items.iter().map(|i| i.text.as_str()).collect();
        assert!(texts.contains(&"Line1"), "missing Line1");
        assert!(texts.contains(&"Line2"), "missing Line2");
        assert!(texts.contains(&"Line3"), "missing Line3");
    }

    #[test]
    fn inspect_text_td_advances_position() {
        // After `Td 100 700`, tx should be ~100 + advance; verify the first
        // item's x is near 100 (within the default identity CTM).
        let pdf = make_pdf_with_text_positioning_ops();
        let result = inspect_bytes(&pdf);
        let first = result.text_items.first().expect("expected text items");
        assert!(
            (first.x - 100.0).abs() < 1e-4,
            "expected text x to be exactly 100.0, got {}",
            first.x
        );
    }

    #[test]
    fn inspect_text_td_updates_leading_via_td_operator() {
        // TD 0 -14 sets text_leading = 14 (abs(-(-14))), then T* moves by that
        // amount. The third item (after T*) should have a y offset different
        // from the second item (after TD).
        let pdf = make_pdf_with_text_positioning_ops();
        let result = inspect_bytes(&pdf);
        assert!(result.text_items.len() >= 3, "need at least 3 text items");
        let y1 = result.text_items[1].y;
        let y2 = result.text_items[2].y;
        // y2 (after T*) should differ from y1 (after TD) by ~14 pt in either
        // direction (depending on the coordinate convention in use).
        let diff = (y2 - y1).abs();
        assert!(
            (diff - 14.0).abs() < 1e-4,
            "expected T* to advance y by exactly 14.0, got {diff}"
        );
    }

    // --- metadata absent ---

    #[test]
    fn inspect_metadata_returns_all_none_when_no_info_dict() {
        // A PDF without an Info entry in the trailer exercises the
        // `Err(_) => return meta` branch (line 80 in extract_metadata).
        use lopdf::content::{Content, Operation};
        use lopdf::{Document, Object, Stream, dictionary};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new(
                    "Tf",
                    vec![Object::Name(b"F1".to_vec()), Object::Integer(12)],
                ),
                Operation::new(
                    "Tm",
                    vec![
                        Object::Real(1.0),
                        Object::Real(0.0),
                        Object::Real(0.0),
                        Object::Real(1.0),
                        Object::Real(72.0),
                        Object::Real(720.0),
                    ],
                ),
                Operation::new("Tj", vec![Object::string_literal("hi")]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => dictionary! {
                    "Type" => "Font",
                    "Subtype" => "Type1",
                    "BaseFont" => "Courier",
                },
            },
        });
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(595),
                Object::Integer(842),
            ],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(page_id)],
                "Count" => Object::Integer(1),
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        // No trailer "Info" key → exercises the Err branch in extract_metadata
        doc.trailer.set("Root", Object::Reference(catalog_id));

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();

        let result = inspect_bytes(&buf);
        assert_eq!(result.metadata.title, None);
        assert_eq!(result.metadata.author, None);
        assert_eq!(result.metadata.creator, None);
        assert_eq!(result.metadata.created_at, None);
        assert_eq!(result.metadata.modified_at, None);
    }

    /// Cyclic /Parent chain (A -> B -> A, no /Resources anywhere) must terminate.
    /// Without a visited-set cap, `resolve_page_resources` alternates between the two
    /// parent dictionaries forever.
    #[test]
    fn resolve_page_resources_stops_on_multi_node_parent_cycle() {
        use lopdf::{Document, Object, dictionary};

        let mut doc = Document::new();
        // Page 1 -> Pages 2 -> Pages 3 -> Pages 2 (cycle, none have /Resources).
        doc.objects.insert(
            (1, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Page",
                "Parent" => Object::Reference((2, 0)),
            }),
        );
        doc.objects.insert(
            (2, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Parent" => Object::Reference((3, 0)),
            }),
        );
        doc.objects.insert(
            (3, 0),
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Parent" => Object::Reference((2, 0)),
            }),
        );

        assert_eq!(resolve_page_resources(&doc, (1, 0)), None);
    }
}
