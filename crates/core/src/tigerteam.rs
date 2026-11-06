//! Tiger Team Pro, the corporate audit suite leaked from CorpLink: everything NiGHTCOMM does,
//! plus what's *behind* a number, written up the way a security team would: a network profile,
//! findings ranked by severity, and an exposure rating. Worked out here; the app only draws.

use crate::data::Data;
use crate::hub::{Entry, Save, Severity};
use crate::nightcomm::Tab;

/// Tiger Team Pro's sections, in key order (`1` to `5`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    /// One target's report.
    #[default]
    Assessment,
    /// Every target in one table.
    Engagements,
    /// The wardialer.
    Recon,
    /// The ranges you know.
    Address,
    /// The modem terminal.
    Console,
}

impl Page {
    pub const ALL: [Page; 5] = [
        Page::Assessment,
        Page::Engagements,
        Page::Recon,
        Page::Address,
        Page::Console,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Assessment => "Assessment",
            Page::Engagements => "Engagements",
            Page::Recon => "Reconnaissance",
            Page::Address => "Address Space",
            Page::Console => "Console",
        }
    }

    /// A short name for a tight menu bar.
    pub fn short(self) -> &'static str {
        match self {
            Page::Assessment => "Assess",
            Page::Engagements => "Engage",
            Page::Recon => "Recon",
            Page::Address => "Address",
            Page::Console => "Console",
        }
    }

    /// The section a digit key picks (`'1'` is the first).
    pub fn from_key(c: char) -> Option<Page> {
        let i = c.to_digit(10)?.checked_sub(1)? as usize;
        Page::ALL.get(i).copied()
    }

    /// The NiGHTCOMM page this section is, for the ones Tiger Team Pro shares.
    pub fn shared(self) -> Option<Tab> {
        match self {
            Page::Recon => Some(Tab::Wardial),
            Page::Address => Some(Tab::Ranges),
            Page::Console => Some(Tab::Term),
            _ => None,
        }
    }
}

/// How hard a target hits back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Exposure {
    Low,
    Medium,
    High,
}

impl Exposure {
    pub fn label(self) -> &'static str {
        match self {
            Exposure::Low => "LOW",
            Exposure::Medium => "MED",
            Exposure::High => "HIGH",
        }
    }
}

/// Exposure points at or above these read MED and HIGH.
const MEDIUM_AT: u32 = 5;
const HIGH_AT: u32 = 12;

/// One finding on a target, numbered for the report (`F-01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub text: String,
    pub severity: Severity,
}

/// A target as Tiger Team Pro writes it up.
#[derive(Clone, Debug)]
pub struct Profile<'a> {
    pub entry: &'a Entry,
    /// LAN, CORP or MEGACORP.
    pub tier: String,
    /// Layers, entry and exit included.
    pub layers: usize,
    pub subnets: usize,
    /// ICE strength on top of the base (tier and heat).
    pub ice_bonus: u32,
    pub trace_pct: u32,
    pub pay_pct: u32,
    /// The bulletin board on site, if there is one.
    pub board: Option<&'a str>,
    /// Findings, worst first.
    pub findings: Vec<Finding>,
    pub exposure: Exposure,
}

/// Writes up one phonebook entry.
pub fn profile<'a>(data: &'a Data, e: &'a Entry) -> Profile<'a> {
    let (p, m) = Save::site_params(data, e);
    let nets = &data.networks;
    let mut found: Vec<(Severity, String)> = e
        .traits
        .iter()
        .filter_map(|t| nets.trait_def(t))
        .map(|d| (d.severity, d.finding.clone()))
        .collect();
    // Worst first; ties keep the network's own order.
    found.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    let findings: Vec<Finding> = found
        .into_iter()
        .enumerate()
        .map(|(i, (severity, text))| Finding {
            id: format!("F-{:02}", i + 1),
            text,
            severity,
        })
        .collect();
    let points = m.ice_bonus
        + m.trace_pct.saturating_sub(100) / 5
        + findings
            .iter()
            .map(|f| match f.severity {
                Severity::High => 3,
                Severity::Medium => 1,
                _ => 0,
            })
            .sum::<u32>();
    let exposure = if points >= HIGH_AT {
        Exposure::High
    } else if points >= MEDIUM_AT {
        Exposure::Medium
    } else {
        Exposure::Low
    };
    let board = p
        .bbs
        .then(|| data.boards[Save::site_board(data, e)].name.as_str());
    Profile {
        entry: e,
        tier: nets
            .tier(e.tier)
            .map_or_else(|| format!("T{}", e.tier), |t| t.label.clone()),
        layers: p.mid_layers + 2,
        subnets: p.subnets,
        ice_bonus: m.ice_bonus,
        trace_pct: m.trace_pct,
        pay_pct: m.pay_pct,
        board,
        findings,
        exposure,
    }
}

/// How the Engagements table orders targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Order {
    /// Best paying first.
    #[default]
    Value,
    /// Most exposed first.
    Exposure,
    /// Hottest first.
    Alert,
    Tier,
    Name,
}

impl Order {
    pub const ALL: [Order; 5] = [
        Order::Value,
        Order::Exposure,
        Order::Alert,
        Order::Tier,
        Order::Name,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Order::Value => "value",
            Order::Exposure => "exposure",
            Order::Alert => "alert",
            Order::Tier => "tier",
            Order::Name => "name",
        }
    }

    pub fn next(self) -> Order {
        let i = Order::ALL.iter().position(|o| *o == self).unwrap_or(0);
        Order::ALL[(i + 1) % Order::ALL.len()]
    }
}

/// Every target, written up, in `order`. Ties keep the phonebook's order.
pub fn engagements<'a>(save: &'a Save, data: &'a Data, order: Order) -> Vec<Profile<'a>> {
    let mut all: Vec<Profile> = save.phonebook.iter().map(|e| profile(data, e)).collect();
    match order {
        Order::Value => all.sort_by_key(|p| std::cmp::Reverse(p.pay_pct)),
        Order::Exposure => all.sort_by_key(|p| std::cmp::Reverse(p.exposure)),
        Order::Alert => all.sort_by_key(|p| std::cmp::Reverse(p.entry.heat)),
        Order::Tier => all.sort_by_key(|p| std::cmp::Reverse(p.entry.tier)),
        Order::Name => all.sort_by_key(|p| p.entry.name.to_lowercase()),
    }
    all
}

/// The Assessment section's target list: most exposed first, then best paying.
pub fn targets<'a>(save: &'a Save, data: &'a Data) -> Vec<Profile<'a>> {
    let mut all = engagements(save, data, Order::Value);
    all.sort_by_key(|p| std::cmp::Reverse(p.exposure));
    all
}
