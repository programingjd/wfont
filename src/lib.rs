use skera::*;
use std::fmt::{Display, Formatter};
use std::str::from_utf8;
use ttf2woff2::BrotliQuality;
use write_fonts::read::collections::IntSet;
use write_fonts::read::tables::{ebdt, eblc, feat, svg};
use write_fonts::read::{FontRef, TableProvider, TopLevelTable};
use write_fonts::types::{GlyphId, NameId, Tag};
use wuff::decompress_woff2;

#[link(wasm_import_module = "js")]
unsafe extern "C" {
    fn println(ptr: usize, len: usize);
    fn eprintln(ptr: usize, len: usize);
}

/// # Safety
/// wasm export
#[unsafe(no_mangle)]
pub unsafe extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut vec = Vec::<u8>::with_capacity(len);
    let ptr = vec.as_mut_ptr();
    core::mem::forget(vec);
    ptr
}

struct JsonString(String);

impl Display for JsonString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for c in self.0.chars() {
            match c {
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                c if c <= '\u{001F}' => write!(f, "\\u{:04X}", c as u32)?,
                '"' => write!(f, "\\\"")?,
                '\\' => write!(f, "\\\\")?,
                _ => write!(f, "{c}")?,
            }
        }
        Ok(())
    }
}

/// # Safety
/// wasm export
#[unsafe(no_mangle)]
pub unsafe extern "C" fn subset(
    ptr1: usize,
    len1: usize,
    ptr2: usize,
    len2: usize,
    woff2: bool,
) -> Box<[u8; 8]> {
    // # Safety
    // the pointer and length should come from the result of alloc
    let bytes = unsafe { Vec::from_raw_parts(ptr1 as *mut u8, len1, len1) };
    let bytes = if bytes.starts_with(b"wOF2") {
        decompress_woff2(&bytes)
            .inspect_err(|err| log_err(&format!("{err}")))
            .unwrap()
    } else {
        bytes
    };

    // # Safety
    // the pointer and length should come from the result of alloc
    let text = unsafe { Vec::from_raw_parts(ptr2 as *mut u8, len2, len2) };
    let text = from_utf8(&text)
        .inspect_err(|err| log_err(&format!("{err}")))
        .unwrap();

    let font = FontRef::new(&bytes)
        .inspect_err(|err| log_err(&format!("{err}")))
        .unwrap();

    let gids = IntSet::<GlyphId>::empty();
    let mut unicodes = IntSet::<u32>::empty();
    unicodes.extend_unsorted(text.chars().map(|c| c as u32));

    let default_drop_tables = [
        // Layout disabled by default
        MORX,
        MORT,
        KERX,
        KERN,
        // Copied from fontTools
        JSTF,
        DSIG,
        ebdt::Ebdt::TAG,
        eblc::Eblc::TAG,
        EBSC,
        svg::Svg::TAG,
        PCLT,
        LTSH,
        // Graphite tables
        feat::Feat::TAG,
        GLAT,
        GLOC,
        SILF,
        SILL,
    ];

    let subset_flags = SubsetFlags::default();
    let drop_tables: IntSet<Tag> = default_drop_tables.iter().copied().collect();
    let mut name_ids = IntSet::<NameId>::empty();
    name_ids.insert(NameId::FAMILY_NAME);
    // name_ids.insert_range(NameId::from(0)..=NameId::from(6));
    let mut name_languages = IntSet::<u16>::empty();
    name_languages.insert(0x0409);
    let mut layout_scripts = IntSet::<Tag>::empty();
    layout_scripts.invert();
    let mut layout_features = IntSet::<Tag>::empty();
    layout_features.extend(DEFAULT_LAYOUT_FEATURES.iter().copied());
    let plan = Plan::new(
        &gids,
        &unicodes,
        &font,
        subset_flags,
        &drop_tables,
        &layout_scripts,
        &layout_features,
        &name_ids,
        &name_languages,
    );
    let output = subset_font(&font, &plan)
        .inspect_err(|err| log_err(&format!("{err}")))
        .unwrap();
    log_info("ttf font created");
    let output = if woff2 {
        let output = ttf2woff2::encode(&output, BrotliQuality::default())
            .inspect_err(|err| log_err(&format!("{err}")))
            .unwrap();
        log_info("woff2 font created");
        output
    } else {
        output
    };
    let output = output.into_boxed_slice();
    let len = output.len();
    let ptr = Box::into_raw(output) as *mut u8;
    let mut output = Vec::with_capacity(8);
    output.extend_from_slice(&(ptr as u32).to_le_bytes());
    output.extend_from_slice(&(len as u32).to_le_bytes());
    // # Safety
    // convert to pointer and length for wasm
    unsafe { Box::from_raw(Box::into_raw(output.into_boxed_slice()) as *mut [u8; 8]) }
}

/// # Safety
/// wasm export
#[unsafe(no_mangle)]
pub unsafe extern "C" fn metadata(ptr: usize, len: usize) -> Box<[u8; 8]> {
    // # Safety
    // the pointer and length should come from the result of alloc
    let bytes = unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) };
    let bytes = if bytes.starts_with(b"wOF2") {
        decompress_woff2(&bytes)
            .inspect_err(|err| log_err(&format!("{err}")))
            .unwrap()
    } else {
        bytes
    };

    let font = FontRef::new(&bytes)
        .inspect_err(|err| log_err(&format!("{err}")))
        .unwrap();

    let mut metadata = Vec::new();

    match font.name() {
        Ok(name) => {
            let records = name.name_record();
            let family_name = records.iter().find(|it| it.name_id == NameId::FAMILY_NAME);
            if let Some(family_name) = family_name {
                let family_name = family_name
                    .string(name.string_data())
                    .inspect_err(|err| log_err(&format!("{err}")))
                    .unwrap()
                    .to_string();
                log_info(&format!("Font family name: {family_name}"));
                let family_name = JsonString(family_name);
                metadata.push(format!("\"family_name\":\"{family_name}\""));
            } else {
                log_err("Font family name not found");
            }
        }
        Err(err) => {
            log_err(&format!("Error reading font name: {err}"));
        }
    }

    if let Ok(fvar) = font.fvar() {
        match fvar.axes() {
            Ok(axes) => {
                let mut meta = vec![];
                for axis in axes {
                    let tag = axis.axis_tag.to_string();
                    let min = axis.min_value.get().to_f32();
                    let max = axis.max_value.get().to_f32();
                    let default = axis.default_value.get().to_f32();
                    meta.push(format!(
                        "{{\"name\":\"{tag}\",\"min\":{min},\"max\":{max},\"default\":{default}}}"
                    ));
                }
                if !meta.is_empty() {
                    metadata.push(format!("\"axes\":[{}]", meta.join(",")));
                }
            }
            Err(err) => {
                log_err(&format!("Error reading font axes: {err}"));
            }
        }
    }

    let mut output = Vec::with_capacity(8);
    let metadata = format!("{{{}}}", metadata.join(","))
        .into_boxed_str()
        .into_boxed_bytes();
    let len = metadata.len();
    let ptr = Box::into_raw(metadata) as *mut u8;
    output.extend_from_slice(&(ptr as u32).to_le_bytes());
    output.extend_from_slice(&(len as u32).to_le_bytes());
    // # Safety
    // convert to pointer and length for wasm
    unsafe { Box::from_raw(Box::into_raw(output.into_boxed_slice()) as *mut [u8; 8]) }
}

fn log_err(message: &str) {
    unsafe { eprintln(message.as_ptr() as usize, message.len()) };
}

fn log_info(message: &str) {
    unsafe { println(message.as_ptr() as usize, message.len()) };
}
