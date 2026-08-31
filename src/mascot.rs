use eframe::egui;

/// Chibi Morimens emoji embedded into the binary for UI decoration.
///
/// These are official 《忘却前夜》(Morimens) battle emojis, sourced from the
/// community wiki mirror for personal fan use only — do not redistribute
/// commercially. Replace or remove the `assets/emoji/` files if the project
/// is ever published beyond personal use.
pub struct Mascots {
    pub keeper_hi: egui::TextureHandle,
    pub ramona_point: egui::TextureHandle,
    pub erika_ok: egui::TextureHandle,
    pub wanda_work: egui::TextureHandle,
    pub turu_sleep: egui::TextureHandle,
    pub kekesi_cry: egui::TextureHandle,
    pub luotan_easy: egui::TextureHandle,
    pub ogier_salute: egui::TextureHandle,
    pub agrippa_watch: egui::TextureHandle,
    pub keeper_me: egui::TextureHandle,
    pub celeste_pray: egui::TextureHandle,
    pub dexter_cheers: egui::TextureHandle,
    pub ramona_pro: egui::TextureHandle,
    pub brown_dunno: egui::TextureHandle,
    pub hilo_vanish: egui::TextureHandle,
    pub kekesi_grudge: egui::TextureHandle,
    pub wincor_run: egui::TextureHandle,
}

const ENTRIES: &[(&str, &[u8])] = &[
    ("keeper-hi", include_bytes!("../assets/emoji/keeper-hi.png")),
    (
        "ramona-point",
        include_bytes!("../assets/emoji/ramona-point.png"),
    ),
    ("erika-ok", include_bytes!("../assets/emoji/erika-ok.png")),
    (
        "wanda-work",
        include_bytes!("../assets/emoji/wanda-work.png"),
    ),
    (
        "turu-sleep",
        include_bytes!("../assets/emoji/turu-sleep.png"),
    ),
    (
        "kekesi-cry",
        include_bytes!("../assets/emoji/kekesi-cry.png"),
    ),
    (
        "luotan-easy",
        include_bytes!("../assets/emoji/luotan-easy.png"),
    ),
    (
        "ogier-salute",
        include_bytes!("../assets/emoji/ogier-salute.png"),
    ),
    (
        "agrippa-watch",
        include_bytes!("../assets/emoji/agrippa-watch.png"),
    ),
    ("keeper-me", include_bytes!("../assets/emoji/keeper-me.png")),
    (
        "celeste-pray",
        include_bytes!("../assets/emoji/celeste-pray.png"),
    ),
    (
        "dexter-cheers",
        include_bytes!("../assets/emoji/dexter-cheers.png"),
    ),
    (
        "ramona-pro",
        include_bytes!("../assets/emoji/ramona-pro.png"),
    ),
    (
        "brown-dunno",
        include_bytes!("../assets/emoji/brown-dunno.png"),
    ),
    (
        "hilo-vanish",
        include_bytes!("../assets/emoji/hilo-vanish.png"),
    ),
    (
        "kekesi-grudge",
        include_bytes!("../assets/emoji/kekesi-grudge.png"),
    ),
    (
        "wincor-run",
        include_bytes!("../assets/emoji/wincor-run.png"),
    ),
];

impl Mascots {
    pub fn new(ctx: &egui::Context) -> Self {
        let mut textures = std::collections::HashMap::new();
        for (name, bytes) in ENTRIES {
            textures.insert(*name, load(ctx, name, bytes));
        }
        let mut take = |name: &str| textures.remove(name).expect("表情纹理缺失");
        Self {
            keeper_hi: take("keeper-hi"),
            ramona_point: take("ramona-point"),
            erika_ok: take("erika-ok"),
            wanda_work: take("wanda-work"),
            turu_sleep: take("turu-sleep"),
            kekesi_cry: take("kekesi-cry"),
            luotan_easy: take("luotan-easy"),
            ogier_salute: take("ogier-salute"),
            agrippa_watch: take("agrippa-watch"),
            keeper_me: take("keeper-me"),
            celeste_pray: take("celeste-pray"),
            dexter_cheers: take("dexter-cheers"),
            ramona_pro: take("ramona-pro"),
            brown_dunno: take("brown-dunno"),
            hilo_vanish: take("hilo-vanish"),
            kekesi_grudge: take("kekesi-grudge"),
            wincor_run: take("wincor-run"),
        }
    }
}

fn load(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let image = decode(name, bytes).into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    ctx.load_texture(
        format!("mascot-{name}"),
        color_image,
        egui::TextureOptions::LINEAR,
    )
}

fn decode(name: &str, bytes: &[u8]) -> image::DynamicImage {
    image::load_from_memory(bytes)
        .unwrap_or_else(|error| panic!("内嵌表情 {name} 解码失败：{error}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn embedded_emojis_decode_with_enabled_image_formats() {
        for (name, bytes) in super::ENTRIES {
            super::decode(name, bytes);
        }
    }
}
