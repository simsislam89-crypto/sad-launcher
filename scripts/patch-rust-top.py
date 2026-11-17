import io
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()

start_marker = '                ui.add_space(16.0);\n                widgets::section_label(ui, skin, "RUST-ИЗДАНИЕ (ЭКСПЕРИМЕНТ)");'
end_marker = "ui.add_space(12.0);\n                }\n"

i = src.find(start_marker)
assert i != -1, "rust card start not found"
j = src.find(end_marker, i)
assert j != -1, "rust card end not found"
j += len(end_marker)
card = src[i:j]
src = src[:i] + src[j:]

anchor = '''            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");'''
assert anchor in src
src = src.replace(anchor, '''            .show(ui, |ui| {
                ui.set_width(ui.available_width());

''' + card.strip() + '''

                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");''', 1)

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("rust card top ok")
