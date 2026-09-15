//! Experimental gold-monitoring and shop-purchase logic.
//!
//! Status: engine only. It is deliberately **not** wired into the editor yet —
//! the shop flow still needs to be designed — so nothing in the UI exposes
//! [`EconomyConfig`]. The logic here is pure and unit tested so the eventual UI
//! can build on it without reworking the rules.

// Reserved for the shop flow that is still being designed: the engine is
// complete and unit tested, but no UI wires it up yet, so most items are only
// exercised from tests.
#![allow(dead_code, reason = "engine kept for the not-yet-designed shop flow")]

use serde::{Deserialize, Serialize};

use crate::model::SearchRegionSpec;

/// Reads a number out of a region using one small template per digit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoinReader {
    /// Template path per digit; index is the digit value.
    pub digit_templates: [Option<String>; 10],
    /// Where to look; `None` means the whole client area.
    pub region: Option<SearchRegionSpec>,
    /// Reference size the digits were captured at.
    pub reference_width: u32,
    pub reference_height: u32,
}

/// A matched digit box, in client coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigitBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub digit: u8,
}

impl DigitBox {
    pub fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }
}

/// Turns left-to-right digit hits into a number.
///
/// Hits are sorted by `x`; a horizontal gap wider than `gap_tolerance` ends the
/// number, so unrelated icons on the same row are ignored. Returns `None` when
/// no digit was read.
pub fn decode_amount(mut boxes: Vec<DigitBox>, gap_tolerance: u32) -> Option<u64> {
    boxes.retain(|hit| hit.digit <= 9);
    if boxes.is_empty() {
        return None;
    }
    boxes.sort_by_key(|hit| hit.x);
    let readable = boxes.first().expect("non-empty").x;
    let mut digits: Vec<u8> = Vec::new();
    let mut cursor = readable;
    for hit in boxes {
        if !digits.is_empty() && hit.x > cursor.saturating_add(gap_tolerance) {
            break;
        }
        // Digits may overlap slightly after scaling; keep the left-most reading.
        let overlaps_previous = !digits.is_empty() && hit.x < cursor;
        if overlaps_previous {
            continue;
        }
        cursor = hit.right();
        digits.push(hit.digit);
        if digits.len() >= 9 {
            break;
        }
    }
    let mut value: u64 = 0;
    for digit in digits {
        value = value.saturating_mul(10).saturating_add(u64::from(digit));
    }
    Some(value)
}

/// One purchasable shop entry. `priority` follows the same idea as the item
/// priority rules: higher first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopEntry {
    pub name: String,
    pub price: u64,
    pub priority: u8,
    pub enabled: bool,
}

/// Limits applied while planning purchases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchasePolicy {
    /// Gold that must remain after shopping.
    pub reserve_gold: u64,
    /// Upper bound of purchases per shop visit.
    pub max_purchases: u8,
    /// Skip entries that cannot be afforded right now instead of stopping.
    pub skip_unaffordable: bool,
}

impl Default for PurchasePolicy {
    fn default() -> Self {
        Self {
            reserve_gold: 0,
            max_purchases: 3,
            skip_unaffordable: true,
        }
    }
}

/// Greedy purchase plan: highest priority first, never spending below the
/// reserve and never exceeding the purchase limit. Returns indices into
/// `entries` in the order they should be bought.
pub fn plan_purchases(gold: u64, entries: &[ShopEntry], policy: PurchasePolicy) -> Vec<usize> {
    let mut order: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.enabled)
        .map(|(index, _)| index)
        .collect();
    order.sort_by(|left, right| {
        entries[*right]
            .priority
            .cmp(&entries[*left].priority)
            .then_with(|| entries[*left].price.cmp(&entries[*right].price))
    });

    let mut remaining = gold;
    let mut planned = Vec::new();
    for index in order {
        if planned.len() >= usize::from(policy.max_purchases) {
            break;
        }
        let entry = &entries[index];
        if remaining < entry.price.saturating_add(policy.reserve_gold) {
            if policy.skip_unaffordable {
                continue;
            }
            break;
        }
        remaining -= entry.price;
        planned.push(index);
    }
    planned
}

/// Whether a fresh reading differs enough from the previous one to act on.
pub fn gold_changed(previous: Option<u64>, current: u64, threshold: u64) -> bool {
    match previous {
        None => true,
        Some(previous) => previous.abs_diff(current) >= threshold,
    }
}

/// Configuration placeholder for the future shop flow. Not exposed in the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EconomyConfig {
    pub enabled: bool,
    pub coin: CoinReader,
    /// Shop entries with their prices, in the user's own order.
    pub shop: Vec<ShopEntry>,
    pub policy: PurchasePolicy,
    /// How often the coin region is monitored.
    pub poll_secs: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digit(x: u32, digit: u8) -> DigitBox {
        DigitBox {
            x,
            y: 40,
            width: 12,
            height: 16,
            digit,
        }
    }

    #[test]
    fn decodes_a_number_from_left_to_right() {
        let hits = vec![digit(80, 3), digit(40, 1), digit(60, 2)];
        assert_eq!(decode_amount(hits, 8), Some(123));
    }

    #[test]
    fn stops_at_a_large_gap_and_ignores_empty_readings() {
        let hits = vec![digit(10, 9), digit(24, 8), digit(90, 7)];
        assert_eq!(decode_amount(hits, 10), Some(98));
        assert_eq!(decode_amount(Vec::new(), 10), None);
        // Out-of-range digits are dropped instead of corrupting the number.
        let hits = vec![
            digit(10, 1),
            DigitBox {
                digit: 42,
                ..digit(30, 0)
            },
        ];
        assert_eq!(decode_amount(hits, 30), Some(1));
    }

    fn entry(name: &str, price: u64, priority: u8, enabled: bool) -> ShopEntry {
        ShopEntry {
            name: name.to_owned(),
            price,
            priority,
            enabled,
        }
    }

    #[test]
    fn purchase_plan_follows_priority_then_price() {
        let entries = vec![
            entry("药水", 100, 1, true),
            entry("钥匙", 300, 3, true),
            entry("卷轴", 200, 3, true),
            entry("停用项", 10, 9, false),
        ];
        let policy = PurchasePolicy {
            reserve_gold: 0,
            max_purchases: 3,
            skip_unaffordable: true,
        };
        assert_eq!(plan_purchases(1000, &entries, policy), vec![2, 1, 0]);
    }

    #[test]
    fn purchase_plan_respects_reserve_and_limits() {
        let entries = vec![
            entry("贵", 500, 5, true),
            entry("中", 200, 4, true),
            entry("便宜", 50, 3, true),
        ];
        let policy = PurchasePolicy {
            reserve_gold: 300,
            max_purchases: 2,
            skip_unaffordable: true,
        };
        // 600 gold: the 500 entry would break the reserve, so it is skipped.
        assert_eq!(plan_purchases(600, &entries, policy), vec![1, 2]);
        // Stopping instead of skipping changes the outcome.
        let stopping = PurchasePolicy {
            skip_unaffordable: false,
            ..policy
        };
        assert_eq!(plan_purchases(600, &entries, stopping), Vec::<usize>::new());
        // The limit caps the plan even when everything is affordable.
        let generous = PurchasePolicy {
            reserve_gold: 0,
            max_purchases: 1,
            skip_unaffordable: true,
        };
        assert_eq!(plan_purchases(10_000, &entries, generous), vec![0]);
    }

    #[test]
    fn gold_change_detection() {
        assert!(gold_changed(None, 0, 1));
        assert!(!gold_changed(Some(100), 100, 5));
        assert!(!gold_changed(Some(100), 104, 5));
        assert!(gold_changed(Some(100), 105, 5));
        assert!(gold_changed(Some(100), 80, 5));
    }
}
