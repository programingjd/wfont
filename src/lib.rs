use skera::*;
use write_fonts::read::collections::IntSet;
use write_fonts::read::tables::{ebdt, eblc, feat, svg};
use write_fonts::read::{FontRef, TopLevelTable};
use write_fonts::types::{GlyphId, NameId, Tag};

#[link(wasm_import_module = "js")]
unsafe extern "C" {
    fn println(ptr: usize, len: usize);
    fn eprintln(ptr: usize, len: usize);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn subset() {
    let bytes = include_bytes!("../barlow.woff2");
    let font = FontRef::new(bytes)
        .inspect_err(|err| log_err(&format!("{err}")))
        .unwrap();

    let gids = IntSet::<GlyphId>::all();
    let mut unicodes = IntSet::<u32>::empty();
    let text = "Jerome";
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
    name_ids.insert_range(NameId::from(0)..=NameId::from(6));
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
    todo!()
}

fn log_err(message: &str) {
    unsafe { eprintln(message.as_ptr() as usize, message.len()) };
}
