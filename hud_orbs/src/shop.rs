//! Round 27 (B2): Ugo Kalahad's shop (Kalahad's Adventure Wear) in Wizard101's shop window, buying and selling
//! Elden Ring items for runes.
//!
//! Look: Wizard101's own ShoppingWindow.gui (the BUY / SELL tabs, the category buttons Hats .. Deck, the scroll with
//! the shop's name) with BuyWindow.gui or SellWindow.gui in its interactionWindow (0,146), all from
//! _Shared-WorldData.wad (tools/w101_book_layout.py --pages=ShoppingWindow,BuyWindow,SellWindow). The dye palettes,
//! crowns, rentals, jewels and backpack parts are hidden (nothing here uses them); Ugo sells only Hats, Robes,
//! Boots and Wands, so the other category buttons are greyed. Prices are in runes (Wizard101's gold x 25,
//! tools/w101_shop.py), the gold bag shows the runes held.
//!
//! Opening: Ugo has no dialogue. His talk event raises markers::SHOP_REQUEST (the street worker's, dialogue.tsv
//! "ugo_kalahad|idle"); the book opens this window instead of the dialogue box and answers the talk OK when it
//! closes (talk::DIALOG_OK). Walking away withdraws the request and closes the window.
//!
//! Trading: book/er/shop.tsv (tools/w101_shop.py): B rows (Wizard101 item -> its Elden Ring equivalent) and S rows
//! (Elden Ring head / chest / leg armour and unupgraded staves / seals at their sellValue). One event per row in the
//! mod's common.emevd (tools/emevd.py --shop) gives (2003[43]) or takes (2003[42] count, 2003[24] remove) the item
//! when the DLL raises REQUEST with the row's number in the INDEX flags, then turns DONE (or FAILED) on. Runes are
//! the DLL's: checked before a buy and taken on DONE; paid on a sale's DONE. Equipped items are never offered.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use hudhook::imgui::ImColor32;

use crate::game;
use crate::itemdata::ItemData;
use crate::layout::{Layout, TextStyle, View};
use crate::names::Names;

/// The layouts (book folder .lay files).
pub const WINDOW: &str = "ShoppingWindow";
pub const BUY: &str = "BuyWindow";
pub const SELL: &str = "SellWindow";
/// Wizard101's screen for these windows (HCENTER | VCENTER 800x600 on 1024x768) and where Buy/SellWindow sit in it.
pub const SCREEN: [f32; 2] = [1024.0, 768.0];
pub const SIZE: [f32; 2] = [800.0, 600.0];
pub const INNER_Y: f32 = 146.0;
pub const INNER_SIZE: [f32; 2] = [800.0, 453.0];
/// The shop's name on the scroll when dialogue.tsv has none (ugo_kalahad|idle page 0).
pub const DEFAULT_TITLE: &str = "Kalahad's Adventure Wear";

/// Event flags (tools/emevd.py SHOP_*): free flags 1042362300-329 of the mod's 1042362xxx block.
pub const REQUEST: u32 = 1042362300;
pub const SELLING: u32 = 1042362301;
pub const INDEX: u32 = 1042362302;
pub const INDEX_BITS: u32 = 10;
pub const DONE: u32 = 1042362312;
pub const FAILED: u32 = 1042362313;
/// Seconds to wait for the row's event before giving up (the events aren't in common.emevd).
const ANSWER_SECONDS: f32 = 3.0;
/// Rows per page (shoplist0-7).
pub const ROWS: usize = 8;

/// Ugo's categories: ShoppingWindow's category button and shop.tsv's category.
pub const CATEGORIES: [(&str, &str, &str); 4] = [
    ("ShopCategory_Hat", "hat", "Hats"),
    ("ShopCategory_Robe", "robe", "Robes"),
    ("ShopCategory_Shoes", "boots", "Boots"),
    ("ShopCategory_Weapon", "wand", "Wands"),
];
/// The category buttons Ugo doesn't use (greyed, as a Wizard101 shop without them).
const UNUSED: [&str; 6] = ["ShopCategory_Athame", "ShopCategory_Amulet", "ShopCategory_Ring", "ShopCategory_Pet",
    "ShopCategory_Mount", "ShopCategory_Deck"];
/// Parts of Buy/SellWindow nothing here uses (dyes, crowns, rentals, jewels, the backpack, the equipped fist).
const HIDDEN: [&str; 25] = ["patterns", "colorPalette", "secondaryColors", "labelColor", "labelTrim", "labelPattern",
    "overlay", "goldShopOverlay", "buyCrowns", "previewButton", "crownsBagIcon", "jewelsIcon", "SubscriberPrice",
    "sold", "RentalIcon", "fist", "spaceLeftBackground", "backpackIcon", "labelYourGold", "crowns", "pvp", "rental", "crafted",
    "inventorySpace",
    "lock"];

/// One row of shop.tsv.
#[derive(Clone, Debug)]
pub struct Line {
    /// The row's event number.
    pub n: u32,
    /// Index into CATEGORIES.
    pub cat: usize,
    /// The Wizard101 item's name (buy rows).
    pub name: String,
    pub runes: u32,
    /// Elden Ring item ID with category bits.
    pub item: u32,
}

#[derive(Default)]
pub struct Stock {
    pub buy: Vec<Line>,
    pub sell: HashMap<u32, Line>,
}

impl Stock {
    pub fn is_empty(&self) -> bool {
        self.buy.is_empty()
    }
}

/// Reads book/er/shop.tsv.
pub fn load(path: &Path) -> Stock {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let cat = |c: &str| CATEGORIES.iter().position(|k| k.1 == c);
    let mut stock = Stock::default();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.parse::<u32>().ok());
        match (f.first().copied(), f.get(2).and_then(|c| cat(c))) {
            (Some("B"), Some(c)) if f.len() >= 7 => {
                if let (Some(n), Some(runes), Some(item)) = (num(1), num(5), num(6)) {
                    stock.buy.push(Line { n, cat: c, name: f[3].to_string(), runes, item });
                }
            }
            (Some("S"), Some(c)) if f.len() >= 5 => {
                if let (Some(n), Some(item), Some(runes)) = (num(1), num(3), num(4)) {
                    stock.sell.insert(item, Line { n, cat: c, name: String::new(), runes, item });
                }
            }
            _ => {}
        }
    }
    tracing::info!("shop: {} to buy, {} sellable items ({})", stock.buy.len(), stock.sell.len(), path.display());
    stock
}

/// A buy or sale sent to its event.
#[derive(Clone, Debug)]
pub struct Trade {
    pub line: Line,
    pub sell: bool,
    pub sent: Instant,
}

/// The window on screen.
pub struct Shop {
    /// The talk request it answers (talk.rs index); None in the preview.
    pub request: Option<usize>,
    pub title: String,
    pub selling: bool,
    pub cat: usize,
    pub page: usize,
    /// The highlighted row (index into `list`).
    pub chosen: usize,
    pub opened: Instant,
    /// What the player holds that the shop buys (item ID, count), not equipped; and the runes held.
    pub held: Vec<(u32, u32)>,
    pub runes: u32,
    pub read_at: Option<Instant>,
    /// A line under the BUY / SELL button for a while (bought, sold, not enough runes).
    pub note: Option<(String, Instant)>,
}

impl Shop {
    pub fn new(request: Option<usize>, title: String) -> Shop {
        Shop { request, title, selling: false, cat: 0, page: 0, chosen: 0, opened: Instant::now(), held: Vec::new(),
               runes: 0, read_at: None, note: None }
    }

    /// The rows of the open tab and category.
    pub fn list(&self, stock: &Stock) -> Vec<(Line, u32)> {
        if self.selling {
            self.held.iter().filter_map(|(id, n)| stock.sell.get(id).filter(|l| l.cat == self.cat).map(|l| (l.clone(), *n)))
                .collect()
        } else {
            stock.buy.iter().filter(|l| l.cat == self.cat).map(|l| (l.clone(), 1)).collect()
        }
    }

    /// The player's runes and sellable items from the character sheet (not equipped: weapons slots 0-5, armour
    /// 12-15).
    pub fn read(&mut self, sheet: &crate::sheet::Sheet, stock: &Stock) {
        self.runes = sheet.runes;
        let equipped: Vec<u32> = sheet.equipment.iter().enumerate()
            .filter(|(i, id)| (*i < 6 || (12..16).contains(i)) && **id >= 0)
            .map(|(i, id)| if i < 6 { *id as u32 } else { 0x1000_0000 | *id as u32 })
            .collect();
        let mut held: Vec<(u32, u32)> = Vec::new();
        for (id, n) in &sheet.items {
            if !stock.sell.contains_key(id) || equipped.contains(id) || *n == 0 {
                continue;
            }
            match held.iter_mut().find(|h| h.0 == *id) {
                Some(h) => h.1 += n,
                None => held.push((*id, *n)),
            }
        }
        held.sort_by_key(|h| (stock.sell[&h.0].cat, h.0));
        self.held = held;
        self.read_at = Some(Instant::now());
    }

    fn clamp(&mut self, rows: usize) {
        let pages = rows.div_ceil(ROWS).max(1);
        self.page = self.page.min(pages - 1);
        self.chosen = self.chosen.min(rows.saturating_sub(1));
    }
}

/// What a click or key in the window does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Close,
    Tab(bool),
    Category(usize),
    Row(usize),
    Page(i32),
    Trade,
}

/// The window's placement on a `screen`: (scale, top left of ShoppingWindow).
pub fn placement(screen: [f32; 2]) -> (f32, [f32; 2]) {
    let s = (screen[0] / SCREEN[0]).min(screen[1] / SCREEN[1]);
    (s, [(screen[0] - SIZE[0] * s) / 2.0, (screen[1] - SIZE[1] * s) / 2.0])
}

fn yellow() -> ImColor32 {
    ImColor32::from_rgba(255, 255, 0, 255)
}

/// The ShoppingWindow view: the tab and category chosen, the unused categories greyed, the shop's name.
pub fn top_view(l: &Layout, shop: &Shop) -> View {
    let mut v = View::default();
    let checked = |v: &mut View, name: &str| {
        if let Some(i) = l.find(name) {
            v.checked.insert(i);
        }
    };
    checked(&mut v, if shop.selling { "sellTab" } else { "buyTab" });
    checked(&mut v, CATEGORIES[shop.cat].0);
    for name in UNUSED {
        if let Some(i) = l.find(name) {
            v.greyed.insert(i);
        }
    }
    for (node, _, label) in CATEGORIES {
        if let Some(i) = l.find(node) {
            v.plain_tip(l.nodes[i].rect, vec![label.to_string()]);
        }
    }
    v.set(l.find("shopTitleText"), format!("<center>{}", shop.title));
    v
}

/// Round 28 (B4): Elden Ring's own rune symbol (01_common.tpf SB_FE_01 MENU_FL_Runes, the icon beside the HUD's rune
/// count, 52x50), deployed to book/er/menu; it replaces Wizard101's gold bag (goldBagIcon) beside the runes held.
#[allow(dead_code)]
pub const ER_RUNE: &str = "@er/menu/MENU_FL_Runes.png";
/// Round 29 (S1): the middle of the rune amount's digits (totalCoinsControlText 368,412..507,443), window pixels.
const RUNES_MIDDLE: f32 = 427.5;
/// Round 28 (B3): the part of Background_Buying with Wizard101's backpack-space bar (the blue backpack badge and the
/// red box, 120..288 x 298..358 of the 800x453 window), covered with Background_Selling's plain parchment at the same
/// spot, so only the BUY button and its own background stay.
const BACKPACK_BAR: [f32; 4] = [119.0, 294.0, 292.0, 362.0];
const PARCHMENT: &str = "GUI/Shopping/Background_Selling.png";
/// Round 28 (B2): the Sell window's cost-box font (SellWindow.gui labelCost / priceControlText: WizardFancy 0.5006),
/// used on the Buy window too (BuyWindow.gui's 0.67 was much too big for "Cost 1000").
const COST_SCALE: f32 = 0.5006;

/// The Buy or Sell window's view: the page of rows, the chosen item's picture, name, price, the runes held and the
/// page number. Round 28: Elden Ring names everywhere (B1), Buy rows show only the centred name (B6), Sell rows the
/// name and price without a rune (B8), the item tooltip is the book's (B9).
pub fn page_view(l: &Layout, shop: &mut Shop, stock: &Stock, names: &Names, items: &ItemData) -> View {
    let mut v = View::default();
    for (i, n) in l.nodes.iter().enumerate() {
        if HIDDEN.iter().any(|h| n.name == *h || (n.name.starts_with(h) && n.name[h.len()..].parse::<u32>().is_ok())) {
            v.show.insert(i, false);
        }
    }
    if !shop.selling {
        let uv = [BACKPACK_BAR[0] / INNER_SIZE[0], BACKPACK_BAR[1] / INNER_SIZE[1], BACKPACK_BAR[2] / INNER_SIZE[0],
                  BACKPACK_BAR[3] / INNER_SIZE[1]];
        v.patches.push((0, BACKPACK_BAR, PARCHMENT.to_string(), uv));
    }
    let list = shop.list(stock);
    shop.clamp(list.len());
    let pages = list.len().div_ceil(ROWS).max(1);
    let row_style = TextStyle::new("WizardFancy", 0.5, yellow());
    let price_style = TextStyle::new("WizardFancy", 0.5, yellow()).right();
    for r in 0..ROWS {
        let Some(node) = l.find(&format!("shoplist{r}")) else { continue };
        let at = shop.page * ROWS + r;
        let Some((line, count)) = list.get(at) else {
            v.show.insert(node, false);
            continue;
        };
        let rect = l.nodes[node].rect;
        if at == shop.chosen {
            v.checked.insert(node);
        }
        let name = names.item(line.item);
        let name = if *count > 1 { format!("{name} ({count})") } else { name };
        if shop.selling {
            v.add([rect[0] + 12.0, rect[1], rect[2] - 70.0, rect[3]], name, row_style.clone());
            v.add([rect[2] - 70.0, rect[1], rect[2] - 10.0, rect[3]], line.runes.to_string(), price_style.clone());
        } else {
            v.add([rect[0] + 8.0, rect[1], rect[2] - 8.0, rect[3]], format!("<center>{name}"),
                  row_style.clone().centred());
        }
        v.tip(rect, items.tooltip(line.item, *count, names, false));
    }
    let chosen = list.get(shop.chosen);
    if let Some((line, _)) = chosen {
        v.set(l.find("labelItemHighlighted"), format!("<center>{}", names.item(line.item)));
        v.set(l.find("priceControlText"), format!("<center>{}", line.runes));
        if let Some(i) = l.find("pictureDisplay") {
            let r = l.nodes[i].rect;
            let (w, h) = (r[2] - r[0], r[3] - r[1]);
            let side = w.min(h) * 0.8;
            let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
            if let Some(icon) = items.icon(line.item) {
                v.image([cx - side / 2.0, cy - side / 2.0, cx + side / 2.0, cy + side / 2.0], icon);
            }
        }
        // The price inside the cost box (its right end at x ~289 in Background_Buying, ~280 in Background_Selling),
        // in the Sell window's font on both (B2). Round 29 (S1, Ryan): no rune symbol in the cost box any more.
        if let Some(i) = l.find("priceControlText") {
            let r = l.nodes[i].rect;
            let right = r[2].min(288.0) - 2.0;
            let left = if shop.selling { r[0] } else { 200.0 };
            v.rect.insert(i, [left, r[1], right, r[3]]);
        }
        for name in ["labelCost", "priceControlText"] {
            if let Some(i) = l.find(name) {
                if let Some(mut style) = l.nodes[i].text_style.clone() {
                    style.scale = COST_SCALE;
                    style.middle = true;
                    v.style.insert(i, style);
                }
            }
        }
    } else {
        v.set(l.find("labelItemHighlighted"), if shop.selling { "<center>Nothing to sell" } else { "" });
        v.set(l.find("priceControlText"), "");
        v.set(l.find("labelCost"), "");
    }
    // Round 28 (B4): Elden Ring's rune symbol in place of Wizard101's gold bag. Round 29 (S1, Ryan: "it looks
    // see-through; it must be solid with its background"): Elden Ring's bare rune is half transparent (961 of its
    // 2600 pixels), so it is now the solid gold rune piece of the quest cards' reward (Wizard101's gold disc and ring
    // round Elden Ring's rune, the same family as the gold bag's Art_Quest_Max_Gold), at the bag's 50 square, moved
    // down from the bag's spot (390..440) to sit centred on the rune amount's line (RUNES_MIDDLE).
    if let Some(i) = l.find("goldBagIcon") {
        v.no_back.insert(i);
        let r = l.nodes[i].rect;
        let (cx, half) = ((r[0] + r[2]) / 2.0, (r[3] - r[1]) / 2.0);
        v.image([cx - half, RUNES_MIDDLE - half, cx + half, RUNES_MIDDLE + half], crate::pages::REWARD_RUNES);
    }
    let action = l.find(if shop.selling { "sellAction" } else { "buyAction" });
    let can = chosen.is_some_and(|(line, _)| shop.selling || shop.runes >= line.runes);
    if let Some(a) = action.filter(|_| !can) {
        v.greyed.insert(a);
    }
    v.set(l.find("totalCoinsControlText"), shop.runes.to_string());
    v.set(l.find("labelScrollPage"), format!("<center>{}/{}", shop.page + 1, pages));
    for (name, back) in [("leftscroll", shop.page == 0), ("rightscroll", shop.page + 1 >= pages)] {
        if let Some(i) = l.find(name).filter(|_| back) {
            // Round 28 (B7): Wizard101's disabled frame of Button_Left/Right is already grey; drawn solid.
            v.greyed.insert(i);
            v.opaque.insert(i);
        }
    }
    if let Some((note, at)) = &shop.note {
        if at.elapsed().as_secs_f32() < 4.0 {
            if let Some(i) = action {
                let r = l.nodes[i].rect;
                v.add([r[0] - 80.0, r[3] + 2.0, r[2] + 80.0, r[3] + 30.0], format!("<center>{note}"),
                      TextStyle::new("WizardFancy", 0.45, yellow()).centred());
            }
        }
    }
    v
}

/// The act of a click on node `name` of ShoppingWindow (`top`) or Buy/SellWindow.
pub fn click(name: &str, top: bool) -> Option<Act> {
    if top {
        return match name {
            "buyTab" => Some(Act::Tab(false)),
            "sellTab" => Some(Act::Tab(true)),
            _ => CATEGORIES.iter().position(|c| c.0 == name).map(Act::Category),
        };
    }
    match name {
        "exit" => Some(Act::Close),
        "leftscroll" => Some(Act::Page(-1)),
        "rightscroll" => Some(Act::Page(1)),
        "buyAction" | "sellAction" => Some(Act::Trade),
        _ => name.strip_prefix("shoplist").and_then(|r| r.parse::<usize>().ok()).map(Act::Row),
    }
}

/// Applies a tab / category / row / page act (Close and Trade are the book's).
pub fn apply(shop: &mut Shop, act: Act, stock: &Stock) {
    match act {
        Act::Tab(sell) if sell != shop.selling => {
            shop.selling = sell;
            shop.page = 0;
            shop.chosen = 0;
        }
        Act::Category(c) if c != shop.cat => {
            shop.cat = c;
            shop.page = 0;
            shop.chosen = 0;
        }
        Act::Row(r) => shop.chosen = shop.page * ROWS + r,
        Act::Page(d) => {
            let rows = shop.list(stock).len();
            let pages = rows.div_ceil(ROWS).max(1) as i32;
            let page = (shop.page as i32 + d).clamp(0, pages - 1) as usize;
            if page != shop.page {
                shop.page = page;
                shop.chosen = page * ROWS;
            }
        }
        _ => {}
    }
    shop.clamp(shop.list(stock).len());
}

/// Sends the chosen row to its event; the trade to wait for, or why not.
pub fn request(shop: &Shop, stock: &Stock, names: &Names) -> Result<Trade, String> {
    let list = shop.list(stock);
    let Some((line, _)) = list.get(shop.chosen) else { return Err("Choose an item first".into()) };
    if game::event_flag(REQUEST) == Some(true) {
        return Err("One moment...".into());
    }
    if !shop.selling && shop.runes < line.runes {
        return Err(format!("You need {} runes", line.runes));
    }
    for f in [DONE, FAILED] {
        game::set_event_flag(f, false);
    }
    // The number first, the request last: the event only looks at the number once REQUEST is on.
    game::set_event_flag(SELLING, shop.selling);
    for k in 0..INDEX_BITS {
        game::set_event_flag(INDEX + k, line.n >> k & 1 == 1);
    }
    game::set_event_flag(REQUEST, true);
    tracing::info!("shop: {} {} (row {}, {} runes)", if shop.selling { "sell" } else { "buy" },
                   names.item(line.item), line.n, line.runes);
    Ok(Trade { line: line.clone(), sell: shop.selling, sent: Instant::now() })
}

/// How a trade sent to its event went: None while waiting; Some(Ok(text)) done (runes taken or paid), Some(Err)
/// failed or no answer.
pub fn poll(trade: &Trade, game: Option<&game::Game>, names: &Names) -> Option<Result<String, String>> {
    let on = |f: u32| game::event_flag(f) == Some(true);
    let name = names.item(trade.line.item);
    if on(DONE) {
        game::set_event_flag(DONE, false);
        let runes = trade.line.runes;
        let paid = match game {
            Some(g) if trade.sell => g.add_runes(runes).is_some(),
            Some(g) => g.spend_runes(runes).is_some(),
            None => false,
        };
        tracing::info!("shop: {} {name}: done, {runes} runes {} ({})", if trade.sell { "sold" } else { "bought" },
                       if trade.sell { "paid" } else { "taken" }, if paid { "ok" } else { "FAILED" });
        return Some(Ok(if trade.sell { format!("Sold {name}") } else { format!("Bought {name}") }));
    }
    if on(FAILED) {
        game::set_event_flag(FAILED, false);
        tracing::warn!("shop: {name}: the event found nothing to {}", if trade.sell { "sell" } else { "buy" });
        return Some(Err(format!("You don't have {name}")));
    }
    if trade.sent.elapsed().as_secs_f32() > ANSWER_SECONDS {
        game::set_event_flag(REQUEST, false);
        tracing::warn!("shop: no answer to row {} (the shop events aren't in common.emevd?)", trade.line.n);
        return Some(Err("The shop didn't answer: its events aren't installed".into()));
    }
    None
}
