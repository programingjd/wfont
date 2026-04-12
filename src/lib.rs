mod unicode_blocks;

use crate::unicode_blocks::{block_index, Block};
use skera::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::str::from_utf8;
use ttf2woff2::BrotliQuality;
use write_fonts::read::collections::IntSet;
use write_fonts::read::tables::cmap::CmapSubtable;
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

struct JsonString<'a>(&'a str);

impl<'a> Display for JsonString<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for c in self.0.chars() {
            match c {
                '\n' => write!(f, "\\n")?,
                '\r' => write!(f, "\\r")?,
                '\t' => write!(f, "\\t")?,
                c if c <= '\u{001F}' => write!(f, "\\u{{{:08X}}}", c as u32)?,
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
                    .inspect_err(|err| log_err(&format!("could not read family name: {err}")))
                    .unwrap()
                    .to_string();
                log_info(&format!("Font family name: {family_name}"));
                let family_name = JsonString(&family_name);
                metadata.push(format!("\"family_name\":\"{family_name}\""));
            } else {
                log_err("font family name not found");
            }
        }
        Err(err) => {
            log_err(&format!("could not read font name: {err}"));
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
                        "{{\"tag\":\"{tag}\",\"min\":{min},\"max\":{max},\"default\":{default}}}"
                    ));
                }
                if !meta.is_empty() {
                    metadata.push(format!("\"axes\":[{}]", meta.join(",")));
                }
            }
            Err(err) => {
                log_err(&format!("could not read font axes: {err}"));
            }
        }
    }

    let mut feature_set = BTreeSet::new();
    if let Ok(gsub) = font.gsub() {
        match gsub.feature_list() {
            Ok(features) => {
                for feature in features.feature_records() {
                    let tag = feature.feature_tag.to_string();
                    feature_set.insert(tag);
                }
            }
            Err(err) => {
                log_err(&format!("could not read font gsub features: {err}"));
            }
        }
    }
    if let Ok(gpos) = font.gpos() {
        match gpos.feature_list() {
            Ok(features) => {
                for feature in features.feature_records() {
                    let tag = feature.feature_tag.to_string();
                    feature_set.insert(tag);
                }
            }
            Err(err) => {
                log_err(&format!("could not read font gpos features: {err}"));
            }
        }
    }

    if !feature_set.is_empty() {
        let mut meta = vec![];
        for tag in feature_set.iter() {
            log_info(tag);
            let name = match tag.as_str() {
                it if ("ss01"..="ss20").contains(&it) => {
                    let raw = 255 + it[2..].parse::<u16>().unwrap();
                    let id = NameId::new(raw);
                    match font.name() {
                        Ok(name) => name.name_record().iter().find_map(|&it| {
                            if it.name_id() == id {
                                Some(Cow::Owned(
                                    it.string(name.string_data())
                                        .inspect_err(|err| {
                                            log_err(&format!(
                                                "could not read stylistic set name: {err}"
                                            ))
                                        })
                                        .unwrap()
                                        .to_string(),
                                ))
                            } else {
                                None
                            }
                        }),
                        _ => None,
                    }
                    .unwrap_or_else(|| feature_name(it))
                }
                it if ("cv01"..="cv99").contains(&it) => {
                    let raw = 285 + it[2..].parse::<u16>().unwrap();
                    let id = NameId::new(raw);
                    match font.name() {
                        Ok(name) => name.name_record().iter().find_map(|&it| {
                            if it.name_id() == id {
                                Some(Cow::Owned(
                                    it.string(name.string_data())
                                        .inspect_err(|err| {
                                            log_err(&format!(
                                                "could not read character variant name: {err}"
                                            ))
                                        })
                                        .unwrap()
                                        .to_string(),
                                ))
                            } else {
                                None
                            }
                        }),
                        _ => None,
                    }
                    .unwrap_or_else(|| feature_name(it))
                }
                it => feature_name(it),
            };
            let name = JsonString(&name);
            meta.push(format!("{{\"tag\":\"{tag}\",\"name\":\"{name}\"}}"));
        }
        metadata.push(format!("\"features\":[{}]", meta.join(",")));
    }

    let mut codepoints = BTreeSet::new();
    if let Ok(cmap) = font.cmap() {
        for record in cmap.encoding_records() {
            let subtable = record
                .subtable(cmap.offset_data())
                .inspect_err(|err| log_err(&format!("could not read subtable: {err}")))
                .unwrap();
            match subtable {
                CmapSubtable::Format4(table) => {
                    for (codepoint, _glyph_id) in table.iter() {
                        if let Some(c) = char::from_u32(codepoint)
                            && !c.is_control()
                        {
                            codepoints.insert(codepoint);
                        }
                    }
                }
                CmapSubtable::Format12(table) => {
                    for (codepoint, _glyph_id) in table.iter() {
                        if let Some(c) = char::from_u32(codepoint)
                            && !c.is_control()
                        {
                            codepoints.insert(codepoint);
                        }
                    }
                }
                CmapSubtable::Format13(table) => {
                    for (codepoint, _glyph_id) in table.iter() {
                        if let Some(c) = char::from_u32(codepoint)
                            && !c.is_control()
                        {
                            codepoints.insert(codepoint);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    metadata.push(format!("\"codepoint_count\":{}", codepoints.len()));

    let mut tables = BTreeMap::new();
    for codepoint in codepoints {
        let idx = block_index(codepoint);
        tables.entry(idx).or_insert_with(Vec::new).push(codepoint);
    }
    metadata.push(format!(
        "\"tables\":[{}]",
        tables.into_iter().map(|(block_index, codepoints)| {
            let Block { name, start, end } = Block::at(block_index);
            let codepoints = codepoints.iter().map(|&it| it.to_string()).collect::<Vec<_>>().join(",");
            let name = JsonString(&name);
            format!("{{\"name\":\"{name}\",\"start\":{start},\"end\":{end},\"codepoints\":[{codepoints}]}}")
        }).collect::<Vec<_>>().join(",")
    ));

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

pub fn feature_name(tag: &str) -> Cow<'static, str> {
    match tag {
        "aalt" => Cow::Borrowed("Access All Alternates"),
        "abvf" => Cow::Borrowed("Above-base Forms"),
        "abvm" => Cow::Borrowed("Above-base Mark Positioning"),
        "abvs" => Cow::Borrowed("Above-base Substitutions"),
        "afrc" => Cow::Borrowed("Alternative Fractions"),
        "akhn" => Cow::Borrowed("Akhand"),
        "apkn" => Cow::Borrowed("Kerning for Alternate Proportional Widths"),
        "blwf" => Cow::Borrowed("Below-base Forms"),
        "blwm" => Cow::Borrowed("Below-base Mark Positioning"),
        "blws" => Cow::Borrowed("Below-base Substitutions"),
        "calt" => Cow::Borrowed("Contextual Alternates"),
        "case" => Cow::Borrowed("Case-sensitive Forms"),
        "ccmp" => Cow::Borrowed("Glyph Composition / Decomposition"),
        "cfar" => Cow::Borrowed("Conjunct Form After Ro"),
        "chws" => Cow::Borrowed("Contextual Half-width Spacing"),
        "cjct" => Cow::Borrowed("Conjunct Forms"),
        "clig" => Cow::Borrowed("Contextual Ligatures"),
        "cpct" => Cow::Borrowed("Centered CJK Punctuation"),
        "cpsp" => Cow::Borrowed("Capital Spacing"),
        "cswh" => Cow::Borrowed("Contextual Swash"),
        "curs" => Cow::Borrowed("Cursive Positioning"),
        "c2pc" => Cow::Borrowed("Petite Capitals From Capitals"),
        "c2sc" => Cow::Borrowed("Small Capitals From Capitals"),
        "dist" => Cow::Borrowed("Distances"),
        "dlig" => Cow::Borrowed("Discretionary Ligatures"),
        "dnom" => Cow::Borrowed("Denominators"),
        "dtls" => Cow::Borrowed("Dotless Forms"),
        "expt" => Cow::Borrowed("Expert Forms"),
        "falt" => Cow::Borrowed("Final Glyph on Line Alternates"),
        "fin2" => Cow::Borrowed("Terminal Forms #2"),
        "fin3" => Cow::Borrowed("Terminal Forms #3"),
        "fina" => Cow::Borrowed("Terminal Forms"),
        "flac" => Cow::Borrowed("Flattened Accent Forms"),
        "frac" => Cow::Borrowed("Fractions"),
        "fwid" => Cow::Borrowed("Full Widths"),
        "half" => Cow::Borrowed("Half Forms"),
        "haln" => Cow::Borrowed("Halant Forms"),
        "halt" => Cow::Borrowed("Alternate Half Widths"),
        "hist" => Cow::Borrowed("Historical Forms"),
        "hkna" => Cow::Borrowed("Horizontal Kana Alternates"),
        "hlig" => Cow::Borrowed("Historical Ligatures"),
        "hngl" => Cow::Borrowed("Hangul"),
        "hojo" => Cow::Borrowed("Hojo Kanji Forms (JIS X 0212-1990 Kanji Forms)"),
        "hwid" => Cow::Borrowed("Half Widths"),
        "init" => Cow::Borrowed("Initial Forms"),
        "isol" => Cow::Borrowed("Isolated Forms"),
        "ital" => Cow::Borrowed("Italics"),
        "jalt" => Cow::Borrowed("Justification Alternates"),
        "jp78" => Cow::Borrowed("JIS78 Forms"),
        "jp83" => Cow::Borrowed("JIS83 Forms"),
        "jp90" => Cow::Borrowed("JIS90 Forms"),
        "jp04" => Cow::Borrowed("JIS2004 Forms"),
        "kern" => Cow::Borrowed("Kerning"),
        "lfbd" => Cow::Borrowed("Left Bounds"),
        "liga" => Cow::Borrowed("Standard Ligatures"),
        "ljmo" => Cow::Borrowed("Leading Jamo Forms"),
        "lnum" => Cow::Borrowed("Lining Figures"),
        "locl" => Cow::Borrowed("Localized Forms"),
        "ltra" => Cow::Borrowed("Left-to-right Alternates"),
        "ltrm" => Cow::Borrowed("Left-to-right Mirrored Forms"),
        "mark" => Cow::Borrowed("Mark Positioning"),
        "med2" => Cow::Borrowed("Medial Forms #2"),
        "medi" => Cow::Borrowed("Medial Forms"),
        "mgrk" => Cow::Borrowed("Mathematical Greek"),
        "mkmk" => Cow::Borrowed("Mark to Mark Positioning"),
        "mset" => Cow::Borrowed("Mark Positioning via Substitution"),
        "nalt" => Cow::Borrowed("Alternate Annotation Forms"),
        "nlck" => Cow::Borrowed("NLC Kanji Forms"),
        "nukt" => Cow::Borrowed("Nukta Forms"),
        "numr" => Cow::Borrowed("Numerators"),
        "onum" => Cow::Borrowed("Oldstyle Figures"),
        "opbd" => Cow::Borrowed("Optical Bounds"),
        "ordn" => Cow::Borrowed("Ordinals"),
        "ornm" => Cow::Borrowed("Ornaments"),
        "palt" => Cow::Borrowed("Proportional Alternate Widths"),
        "pcap" => Cow::Borrowed("Petite Capitals"),
        "pkna" => Cow::Borrowed("Proportional Kana"),
        "pnum" => Cow::Borrowed("Proportional Figures"),
        "pref" => Cow::Borrowed("Pre-base Forms"),
        "pres" => Cow::Borrowed("Pre-base Substitutions"),
        "pstf" => Cow::Borrowed("Post-base Forms"),
        "psts" => Cow::Borrowed("Post-base Substitutions"),
        "pwid" => Cow::Borrowed("Proportional Widths"),
        "qwid" => Cow::Borrowed("Quarter Widths"),
        "rand" => Cow::Borrowed("Randomize"),
        "rclt" => Cow::Borrowed("Required Contextual Alternates"),
        "rkrf" => Cow::Borrowed("Rakar Forms"),
        "rlig" => Cow::Borrowed("Required Ligatures"),
        "rphf" => Cow::Borrowed("Reph Form"),
        "rtbd" => Cow::Borrowed("Right Bounds"),
        "rtla" => Cow::Borrowed("Right-to-left Alternates"),
        "rtlm" => Cow::Borrowed("Right-to-left Mirrored Forms"),
        "ruby" => Cow::Borrowed("Ruby Notation Forms"),
        "rvrn" => Cow::Borrowed("Required Variation Alternates"),
        "salt" => Cow::Borrowed("Stylistic Alternates"),
        "sinf" => Cow::Borrowed("Scientific Inferiors"),
        "size" => Cow::Borrowed("Optical size"),
        "smcp" => Cow::Borrowed("Small Capitals"),
        "smpl" => Cow::Borrowed("Simplified Forms"),
        "ssty" => Cow::Borrowed("Math Script-style Alternates"),
        "stch" => Cow::Borrowed("Stretching Glyph Decomposition"),
        "subs" => Cow::Borrowed("Subscript"),
        "sups" => Cow::Borrowed("Superscript"),
        "swsh" => Cow::Borrowed("Swash"),
        "titl" => Cow::Borrowed("Titling"),
        "tjmo" => Cow::Borrowed("Trailing Jamo Forms"),
        "tnam" => Cow::Borrowed("Traditional Name Forms"),
        "tnum" => Cow::Borrowed("Tabular Figures"),
        "trad" => Cow::Borrowed("Traditional Forms"),
        "twid" => Cow::Borrowed("Third Widths"),
        "unic" => Cow::Borrowed("Unicase"),
        "valt" => Cow::Borrowed("Alternate Vertical Metrics"),
        "vapk" => Cow::Borrowed("Kerning for Alternate Proportional Vertical Metrics"),
        "vatu" => Cow::Borrowed("Vattu Variants"),
        "vchw" => Cow::Borrowed("Vertical Contextual Half-width Spacing"),
        "vert" => Cow::Borrowed("Vertical Alternates"),
        "vhal" => Cow::Borrowed("Alternate Vertical Half Metrics"),
        "vjmo" => Cow::Borrowed("Vowel Jamo Forms"),
        "vkna" => Cow::Borrowed("Vertical Kana Alternates"),
        "vkrn" => Cow::Borrowed("Vertical Kerning"),
        "vpal" => Cow::Borrowed("Proportional Alternate Vertical Metrics"),
        "vrt2" => Cow::Borrowed("Vertical Alternates and Rotation"),
        "vrtr" => Cow::Borrowed("Vertical Alternates for Rotation"),
        "zero" => Cow::Borrowed("Slashed Zero"),
        it if ("cv00"..="cv99").contains(&it) => {
            let n: u8 = it[2..].parse().unwrap();
            Cow::Owned(format!("Character Variant {}", n))
        }
        it if ("ss01"..="ss20").contains(&it) => {
            let n: u8 = it[2..].parse().unwrap();
            Cow::Owned(format!("Stylistic Set {}", n))
        }
        it => {
            log_err(&format!("unexpected feature name: {it}"));
            panic!("unexpected feature name");
        }
    }
}
