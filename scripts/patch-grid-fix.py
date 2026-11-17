import io

p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) remove the view-mode toggle chips (grid only) — revert the wrapper to a plain block
src = src.replace('''                        widgets::section_label(ui, skin, label);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if widgets::chip(ui, skin, "▦ Сетка", self.mods_layout == 0).clicked() {
                                self.mods_layout = 0;
                            }
                            if widgets::chip(ui, skin, "☰ Список", self.mods_layout == 1).clicked() {
                                self.mods_layout = 1;
                            }
                        });
                        ui.add_space(6.0);
                        let results = self.search.results.clone();
                        if self.mods_layout == 0 {''', '''                        widgets::section_label(ui, skin, label);
                        ui.add_space(6.0);
                        let results = self.search.results.clone();
                        {''')

# 2) search: chips wrap, field on its own row — nothing wider than the viewport
old_search = '''            ui.horizontal(|ui| {
                let row_w = ui.available_width();
                let field_w = (row_w - 260.0).max(160.0);
                if widgets::chip(ui, skin, "Modrinth", source == 1).clicked() {
                    self.search.source = 1;
                }
                if widgets::chip(ui, skin, "CurseForge", source == 0).clicked() {
                    self.search.source = 0;
                }
                if widgets::chip(ui, skin, "↓ скачивания", self.search.sort == 0).clicked() {
                    self.search.sort = 0;
                }
                if widgets::chip(ui, skin, "♥ популярность", self.search.sort == 1).clicked() {
                    self.search.sort = 1;
                }
                if widgets::chip(ui, skin, "↑ обновления", self.search.sort == 2).clicked() {
                    self.search.sort = 2;
                }
                if widgets::chip(ui, skin, "★ новые", self.search.sort == 3).clicked() {
                    self.search.sort = 3;
                }
                let resp = search_field(
                    ui,
                    skin,
                    "mod-search",
                    &mut self.search.query,
                    "Название",
                    field_w,
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    search_now = true;
                }
                let label = if busy { "Ищу…" } else { "Найти" };
                if widgets::button(ui, skin, label, Btn::Primary, !busy).clicked() {
                    search_now = true;
                }
            });
            ui.add_space(4.0);'''
new_search = '''            // Chips wrap; the field sits on its own row — nothing may
            // overflow the viewport width, or the grid shifts off-screen.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                if widgets::chip(ui, skin, "Modrinth", source == 1).clicked() {
                    self.search.source = 1;
                }
                if widgets::chip(ui, skin, "CurseForge", source == 0).clicked() {
                    self.search.source = 0;
                }
                if widgets::chip(ui, skin, "↓ скачивания", self.search.sort == 0).clicked() {
                    self.search.sort = 0;
                }
                if widgets::chip(ui, skin, "♥ популярность", self.search.sort == 1).clicked() {
                    self.search.sort = 1;
                }
                if widgets::chip(ui, skin, "↑ обновления", self.search.sort == 2).clicked() {
                    self.search.sort = 2;
                }
                if widgets::chip(ui, skin, "★ новые", self.search.sort == 3).clicked() {
                    self.search.sort = 3;
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let field_w = (ui.available_width() - 90.0).max(160.0);
                let resp = search_field(
                    ui,
                    skin,
                    "mod-search",
                    &mut self.search.query,
                    "Название",
                    field_w,
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    search_now = true;
                }
                let label = if busy { "Ищу…" } else { "Найти" };
                if widgets::button(ui, skin, label, Btn::Primary, !busy).clicked() {
                    search_now = true;
                }
            });
            ui.add_space(4.0);'''
assert old_search in src, "search row not found"
src = src.replace(old_search, new_search, 1)

# 3) grid: hard cap the wrap width to the viewport
src = src.replace('''                        if self.mods_layout == 0 {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);''', '''                        {
                            let grid_w = ui.available_width();
                            ui.horizontal_wrapped(|ui| {
                                ui.set_max_width(grid_w);
                                ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);''')

# 4) drop the now-unused list renderer
import re
m = re.search(r"/// Табличный вид: строка с маленькой иконкой.*?\nfn list_row\(.*?\n\}\n", src, re.S)
if m:
    src = src[:m.start()] + src[m.end():]

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("grid fix ok")
