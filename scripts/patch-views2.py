import io, re

# ============ app.rs: detail_rx field + poll pump
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    pub detail_body: Option<Result<String, String>>,",
                  "    pub detail_body: Option<Result<String, String>>,\n    pub detail_rx: Option<Receiver<Result<String, String>>>,")
src = src.replace("            detail_body: None,", "            detail_body: None,\n            detail_rx: None,")

# poll pump for detail body
src = src.replace('''        let mut build_finished = false;''', '''        if let Some(rx) = self.detail_rx.as_ref() {
            match rx.try_recv() {
                Ok(res) => {
                    self.detail_body = Some(res);
                    self.detail_rx = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.detail_rx = None;
                }
            }
        }

        let mut build_finished = false;''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ views/mods.rs rework: auto-search, clickable rows, detail overlay, build in catalog
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# hit_row returns (add, install, open) — patch signature and calls
src = src.replace('''                            let (added, do_install) = hit_row(
                                ui,
                                skin,
                                hit,
                                icon.as_ref(),
                                busy || download_busy,
                            );
                            if added {
                                add_build = Some(i);
                            }
                            if do_install {
                                install_hit = Some(hit.clone());
                            }''', '''                            let (added, do_install, open) = hit_row(
                                ui,
                                skin,
                                hit,
                                icon.as_ref(),
                                busy || download_busy,
                            );
                            if added {
                                add_build = Some(i);
                            }
                            if do_install {
                                install_hit = Some(hit.clone());
                            }
                            if open {
                                open_detail = Some(hit.clone());
                            }''')

src = src.replace('''/// Returns (added_to_build, install_now).
fn hit_row(
    ui: &mut Ui,
    skin: &Skin,
    hit: &ModHit,
    icon: Option<&egui::TextureHandle>,
    busy: bool,
) -> (bool, bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::hover());''',
'''/// Returns (added_to_build, install_now, open_detail).
fn hit_row(
    ui: &mut Ui,
    skin: &Skin,
    hit: &ModHit,
    icon: Option<&egui::TextureHandle>,
    busy: bool,
) -> (bool, bool, bool) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 58.0), Sense::click());''')

src = src.replace('''    let mut add = false;
    let mut install = false;
    let mut child = ui.new_child(''', '''    let mut add = false;
    let mut install = false;
    let mut open = resp.clicked();
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), skin.hover().gamma_multiply(0.55));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let mut child = ui.new_child(''')

src = src.replace('''    (add, install)
}''', '''    (add, install, open)
}''')

# declare open_detail + auto-search inside view_mods
src = src.replace('''        let mut install_build = false;
        let mut clear_build = false;

        widgets::header(''', '''        let mut install_build = false;
        let mut clear_build = false;
        let mut open_detail: Option<ModHit> = None;

        // The catalog is never empty: popular content loads right away.
        if page == 0
            && self.search.results.is_empty()
            && !busy
            && !self.search.searched
        {
            search_now = true;
        }

        widgets::header(''')

# open detail handler + detail overlay rendering before handlers
src = src.replace('''        self.confirm_delete = confirm;

        if open_content {''', '''        self.confirm_delete = confirm;

        if let Some(hit) = open_detail {
            self.detail = Some(hit);
            self.detail_body = None;
        }
        if self.detail.is_some() && self.detail_body.is_none() && self.detail_rx.is_none() {
            if let Some(hit) = self.detail.clone() {
                let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
                self.detail_rx = Some(rx);
                std::thread::Builder::new()
                    .name("mod-detail".into())
                    .spawn(move || {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        let res = match rt {
                            Ok(rt) => rt.block_on(async {
                                let client = crate::minecraft::http_client().ok()?;
                                crate::mods::detail(&client, &hit).await
                            }),
                            Err(_) => Err("Не удалось запустить загрузчик".into()),
                        };
                        let _ = tx.send(res.map(|d| d.body));
                    })
                    .ok();
            }
        }

        if let Some(hit) = self.detail.clone() {
            draw_detail(self, ui, skin, hit, download_busy);
        }

        if open_content {''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("views/mods.rs wiring ok")
