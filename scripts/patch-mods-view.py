import io, re
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

m = re.search(r"impl Launcher \{\n    pub fn view_mods.*?\n    \}\n\}\n", src, re.S)
assert m, "view_mods block not found"
tail = src[m.end():]  # hit_row / mod_row stay

new_view = open("scripts/mods-view-body.txt", encoding="utf-8").read()
src = src[:m.start()] + new_view + tail
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("mods view ok")
