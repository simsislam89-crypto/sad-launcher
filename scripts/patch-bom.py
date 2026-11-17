import io
p = "src/config.rs"
src = io.open(p, encoding="utf-8").read()
old = 'pub fn load() -> Config {\n    let path = config_path();\n    match std::fs::read_to_string(&path) {\n        Ok(text) => serde_json::from_str::<Config>(&text).unwrap_or_default(),\n        Err(_) => Config::default(),\n    }\n}'
new_lines = [
    'pub fn load() -> Config {',
    '    let path = config_path();',
    '    match std::fs::read_to_string(&path) {',
    '        // Some editors write a BOM; serde rejects it \u2014 strip before parsing.',
    "        Ok(text) => {",
    "            let text = text.trim_start_matches('\\u{feff}');",
    '            serde_json::from_str::<Config>(text).unwrap_or_default()',
    '        }',
    '        Err(_) => Config::default(),',
    '    }',
    '}',
]
new = "\n".join(new_lines)
if old in src:
    src = src.replace(old, new)
    io.open(p, "w", encoding="utf-8", newline="\n").write(src)
    print("BOM patch applied")
else:
    print("already patched" if "trim_start_matches" in src else "NOT FOUND")
