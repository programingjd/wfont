import java.lang.Character.UnicodeBlock;

void main() throws Exception {
  final Class<?> clazz = UnicodeBlock.class;
  final Field startsField = clazz.getDeclaredField("blockStarts");
  startsField.setAccessible(true);
  final Field blocksField = clazz.getDeclaredField("blocks");
  blocksField.setAccessible(true);

  final int[] blockStarts = (int[]) startsField.get(null);
  final UnicodeBlock[] blocks = (UnicodeBlock[]) blocksField.get(null);

  final StringBuilder out = new StringBuilder();
  out.append("pub const UNASSIGNED: &str = \"UNASSIGNED\";\n");
  for (final UnicodeBlock block : blocks) {
    if (block == null) continue;
    final String blockName = block.toString();
    out.append("pub const ");
    out.append(blockName);
    out.append(": &str = \"");
    out.append(blockName.replace("_", " "));
    out.append("\";\n");
  }
  out.append("\n");
  out.append("pub const BLOCKS: [&'static str; ");
  out.append(blocks.length);
  out.append("] = [\n");
  for (final UnicodeBlock block : blocks) {
    final String blockName = block == null ? "UNASSIGNED" : block.toString();
    out.append("    ");
    out.append(blockName);
    out.append(",\n");
  }
  out.append("];\n\n");
  out.append("const BLOCK_STARTS: [u32; ");
  out.append(blocks.length);
  out.append("] = [\n");
  for (final int blockStart : blockStarts) {
    out.append("    ");
    out.append(blockStart);
    out.append(",\n");
  }
  out.append("];\n\n");

  out.append("pub fn block_index(codepoint: u32) -> usize {\n");
  out.append("    BLOCK_STARTS\n");
  out.append("        .binary_search(&codepoint)\n");
  out.append("        .unwrap_or_else(|index| index - 1)\n");
  out.append("}\n\n");
  out.append("pub struct Block {\n");
  out.append("    pub name: &'static str,\n");
  out.append("    pub start: u32,\n");
  out.append("    pub end: u32,\n");
  out.append("}\n\n");
  out.append("impl Block {\n");
  out.append("    pub fn at(index: usize) -> Self {\n");
  out.append("        Self {\n");
  out.append("            name: BLOCKS[index],\n");
  out.append("            start: BLOCK_STARTS[index],\n");
  out.append("            end: *BLOCK_STARTS.get(index + 1).unwrap_or(&0x10FFFF),\n");
  out.append("        }\n");
  out.append("    }\n");
  out.append("}\n");

  final String content = out.toString();
  Files.writeString(Path.of("src/unicode_blocks.rs"), content, StandardOpenOption.CREATE, StandardOpenOption.TRUNCATE_EXISTING);
  System.out.println(content);

  final StringBuilder out2 = new StringBuilder();
  out2.append("pub fn codepoint_name(codepoint: u32) -> Option<&'static str> {\n");
  out2.append("    match codepoint {\n");
  int count = 0;
  for (int i = 0; i < 0x10FFFF; i++) {
    if (Character.isISOControl(i)) continue;
    final int category = Character.getType(i);
    if (category == Character.PRIVATE_USE) continue;
    if (category == Character.UNASSIGNED) continue;
    if (category == Character.CONTROL) continue;
    if (category == Character.SURROGATE) continue;
    final String name = Character.getName(i);
    if (name == null) continue;
    if (name.isEmpty()) continue;
    final String upperHex = String.format("%X", i);
    if (name.endsWith("-" + upperHex) || name.endsWith(" " + upperHex)) continue;
    out2.append("        0x");
    out2.append(String.format("%08x", i));
    out2.append(" => Some(\"");
    out2.append(name);
    out2.append("\"),\n");
//    System.out.println(name);
//    }
    ++count;
  }
  out2.append("        _ => None,\n");
  out2.append("    }\n");
  out2.append("}\n\n");
  final String content2 = out2.toString();
  Files.writeString(Path.of("src/codepoint_names.rs"), content2, StandardOpenOption.CREATE, StandardOpenOption.TRUNCATE_EXISTING);
//  System.out.println(content);

  System.out.println("count: " + count);

}
