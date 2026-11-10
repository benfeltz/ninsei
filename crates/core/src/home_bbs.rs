//! Calling a BBS from home: boards you've found go in the phonebook, and dialing one opens it
//! over the desktop with no network run. You pay a phone bill per call, spend the bank's
//! credits, and take jobs for networks in your phonebook. Listings change after each run.

use crate::bbs::{JobPost, Line};
use crate::binary;
use crate::data::{BbsDef, Data, Drop};
use crate::game::{StockFile, roll_stock};
use crate::hub::{KnownBoard, PendingJob, Save, StashItem, UpgradeDef};
use crate::rng::Rng;
use crate::tuning::{
    BBS_JOBS, CONTRACT_GHOST_BONUS_PCT, CONTRACT_GHOST_MARGIN, CONTRACT_GHOST_PCT,
    CONTRACT_PER_LAYER, CONTRACT_REWARD_MAX, CONTRACT_REWARD_MIN,
};

/// A board on the line from home: its listing and jobs as of today, and what you can spend.
/// Rebuild it with [`Save::home_call`] after every purchase.
#[derive(Clone, Debug)]
pub struct HomeCall {
    /// Index into `Data::boards`.
    pub board: usize,
    def: BbsDef,
    stock: Vec<Vec<StockFile>>,
    jobs: Vec<(JobPost, PendingJob)>,
    bank: u32,
    stash_free: u32,
    tools: Vec<String>,
    /// Games you have a cracked copy of (ids in `hub/games.ron`).
    games: Vec<String>,
    /// Tunes you own (ids in `data/music/`).
    tunes: Vec<String>,
    /// Textfiles from this board already on your disk (text ids).
    texts: Vec<String>,
    tool_defs: Vec<crate::data::ToolDef>,
    /// Each game's id and its crack's label, for "you already have".
    cracks: Vec<(String, String)>,
    /// Each tune's id and its title, for "you already have".
    tune_titles: Vec<(String, String)>,
    /// The board's news of new ICE versions, as it stands today.
    news: Option<crate::data::BulletinDef>,
}

impl Line for HomeCall {
    fn board(&self) -> &BbsDef {
        &self.def
    }

    fn news(&self) -> Option<&crate::data::BulletinDef> {
        self.news.as_ref()
    }

    fn stock(&self) -> &[Vec<StockFile>] {
        &self.stock
    }

    fn jobs(&self) -> Vec<JobPost> {
        self.jobs.iter().map(|(p, _)| p.clone()).collect()
    }

    fn credits(&self) -> u32 {
        self.bank
    }

    fn room(&self) -> (&'static str, u32) {
        ("Stash room", self.stash_free)
    }

    fn has_tool(&self, id: &str) -> bool {
        self.tools.iter().any(|t| t == id)
    }

    fn has_text(&self, id: &str) -> bool {
        self.texts.iter().any(|t| t == id)
    }

    fn has_crack(&self, id: &str) -> bool {
        self.games.iter().any(|g| g == id)
    }

    fn has_tune(&self, id: &str) -> bool {
        self.tunes.iter().any(|t| t == id)
    }

    fn buy_blocker(&self, area: usize, file: usize) -> Option<String> {
        let s = self.stock.get(area).and_then(|a| a.get(file))?;
        let f = &self.def.areas[area].files[s.def];
        if s.bought {
            return Some("you already have that one.".into());
        }
        if matches!(
            f.item,
            Drop::Ram(_) | Drop::Cpu(_) | Drop::Hdd(_) | Drop::Repair(_)
        ) {
            return Some("deck mods go on a live deck. download it in a run.".into());
        }
        if matches!(f.item, Drop::Warez(_)) && self.stash_free == 0 {
            return Some("your stash is full.".into());
        }
        if let Some(why) = crate::game::tool_blocker(
            &self.tool_defs,
            &self.cracks,
            &self.tune_titles,
            &f.item,
            self,
        ) {
            return Some(why);
        }
        if self.bank < f.price {
            return Some(format!(
                "insufficient credits: {} needs {}¢, the bank has {}¢.",
                f.file, f.price, self.bank
            ));
        }
        None
    }
}

fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Save {
    pub fn knows_board(&self, id: &str) -> bool {
        self.boards.iter().any(|b| b.id == id)
    }

    /// Adds a board's number to the phonebook. False if it was already there.
    pub fn learn_board(&mut self, id: &str, source: &str) -> bool {
        if self.knows_board(id) {
            return false;
        }
        self.boards.push(KnownBoard {
            id: id.into(),
            source: source.into(),
        });
        true
    }

    /// The boards you know, as indices into `Data::boards`, in the order you found them.
    pub fn known_boards(&self, data: &Data) -> Vec<usize> {
        self.boards
            .iter()
            .filter_map(|k| data.board(&k.id))
            .collect()
    }

    /// A known board by its number, or by its position in the phonebook listing (boards are
    /// numbered after the networks).
    pub fn lookup_board(&self, data: &Data, what: &str) -> Option<usize> {
        let known = self.known_boards(data);
        if let Ok(n) = what.parse::<usize>() {
            return n
                .checked_sub(self.phonebook.len() + 1)
                .and_then(|i| known.get(i).copied());
        }
        known.into_iter().find(|&b| data.boards[b].phone == what)
    }

    /// The RNG for a board's listings since the last run: same save, board and run count,
    /// same answer.
    fn home_rng(&self, data: &Data, board: usize, salt: u64) -> Rng {
        let id = fnv(&data.boards[board].id);
        Rng::new(self.key ^ id.rotate_left(13) ^ u64::from(self.runs).wrapping_mul(0x9E37) ^ salt)
    }

    /// Calls a known board: charges the phone bill and returns the line.
    pub fn call_board(
        &mut self,
        data: &Data,
        defs: &[UpgradeDef],
        board: usize,
    ) -> Result<HomeCall, String> {
        let def = data.boards.get(board).ok_or("no such board.")?;
        if !self.knows_board(&def.id) {
            return Err("you don't have that number.".into());
        }
        if self.bank < def.call_cost {
            return Err(format!(
                "the phone bill for {} is {}¢. you have {}¢.",
                def.name, def.call_cost, self.bank
            ));
        }
        self.bank -= def.call_cost;
        Ok(self.home_call(data, defs, board))
    }

    /// The line to a board as it stands now. Free: [`Save::call_board`] is what charges.
    pub fn home_call(&self, data: &Data, defs: &[UpgradeDef], board: usize) -> HomeCall {
        let def = data.boards[board].clone();
        let mut stock = roll_stock(&def, &mut self.home_rng(data, board, 0x5700C));
        for &(ref b, area, file) in &self.home_bought {
            if *b == def.id
                && let Some(s) = stock.get_mut(area).and_then(|a| a.get_mut(file))
            {
                s.bought = true;
            }
        }
        let stash_free = self.stash_cap(defs).saturating_sub(self.stash.len() as u32);
        HomeCall {
            board,
            jobs: self.home_jobs(data, board),
            def,
            stock,
            bank: self.bank,
            stash_free,
            tools: self.tools.clone(),
            games: self.cracks(),
            tunes: self.tunes.clone(),
            texts: self
                .texts
                .iter()
                .filter(|k| k.board == data.boards[board].id)
                .map(|k| k.id.clone())
                .collect(),
            tool_defs: data.tools.clone(),
            cracks: data.crack_labels(),
            tune_titles: data.tune_titles(),
            // Only a board that hears news reads the versions in force.
            news: data.boards[board].ice_news.as_ref().and_then(|_| {
                crate::bbs::news_bulletin(&data.boards[board], data, &self.shipped(data))
            }),
        }
    }

    /// Jobs a board posts at home: contracts on networks already in your phonebook.
    fn home_jobs(&self, data: &Data, board: usize) -> Vec<(JobPost, PendingJob)> {
        if self.phonebook.is_empty() || data.contracts.is_empty() || !data.boards[board].jobs {
            return Vec::new();
        }
        let id = &data.boards[board].id;
        let mut rng = self.home_rng(data, board, 0x10B5);
        let mut pool: Vec<usize> = (0..self.phonebook.len()).collect();
        let mut out = Vec::new();
        for i in 0..BBS_JOBS.min(pool.len()) {
            let e = &self.phonebook[pool.swap_remove(rng.below(pool.len()))];
            let t = &data.contracts[rng.below(data.contracts.len())];
            let pay = data.networks.tier(e.tier).map_or(100, |t| t.pay_pct);
            // No node yet: price it as a mid-network target.
            let base = rng.range(CONTRACT_REWARD_MIN, CONTRACT_REWARD_MAX) + CONTRACT_PER_LAYER * 3;
            let reward = base * pay / 100;
            let ghost = rng.chance(CONTRACT_GHOST_PCT).then(|| {
                (
                    CONTRACT_GHOST_MARGIN,
                    reward * CONTRACT_GHOST_BONUS_PCT / 100,
                )
            });
            let taken = self.home_taken.iter().any(|(b, j)| b == id && *j == i);
            let pending = PendingJob {
                entry: e.id,
                client: t.client.clone(),
                brief: t.brief.clone(),
                reward,
                ghost,
            };
            let post = JobPost {
                client: t.client.clone(),
                reward,
                target: e.name.clone(),
                detail: e.number.clone(),
                brief: t.brief.replace("{host}", "a box on their net"),
                ghost,
                taken,
            };
            out.push((post, pending));
        }
        out
    }

    /// Downloads a file from a board at home with banked credits. Warez land in the stash,
    /// textfiles teach their range, fragments go in the drawer. Deck mods need a run.
    pub fn home_buy(
        &mut self,
        data: &Data,
        defs: &[UpgradeDef],
        board: usize,
        area: usize,
        file: usize,
    ) -> Result<String, String> {
        let call = self.home_call(data, defs, board);
        if let Some(why) = call.buy_blocker(area, file) {
            return Err(why);
        }
        let s = call.stock[area][file];
        let f = call.def.areas[area].files[s.def].clone();
        let what = match &f.item {
            Drop::Warez(id) => {
                let w = data.warez_ix(id).ok_or("corrupt download.")?;
                let bin = binary::encode(&data.warez[w], self.key);
                binary::check(&bin, self.key).map_err(|_| "corrupt download.".to_string())?;
                let item = self.next_item;
                self.next_item += 1;
                self.stash.push(StashItem {
                    id: item,
                    origin: id.clone(),
                    hex: binary::to_hex(&bin),
                });
                format!("{} is in your stash", data.warez[w].name)
            }
            Drop::Hint(range) => {
                self.learn_range(range, &format!("a textfile on {}", call.def.name));
                format!("new numbers to wardial: {range}")
            }
            Drop::Fragment(n) => {
                self.fragments += n;
                format!("+{n} optimizer fragment(s)")
            }
            Drop::Text(id) => {
                self.keep_text(&call.def.id, id);
                format!("{} is on your disk", f.file)
            }
            Drop::Tool(id) => {
                self.learn_tool(id);
                let name = data.tool(id).map_or(id.as_str(), |t| t.name.as_str());
                format!("{name} is on your desktop")
            }
            Drop::Tune(id) => {
                self.learn_tune(id);
                let title = data.tune(id).map_or(id.as_str(), |t| t.title.as_str());
                format!("{title} is on your disk")
            }
            Drop::Game(id) => {
                self.learn_game(id, crate::hub::GameCopy::Cracked);
                let name = data.game_label(id, crate::hub::GameCopy::Cracked);
                format!("{name} is on your disk")
            }
            _ => return Err("deck mods go on a live deck. download it in a run.".into()),
        };
        self.bank -= f.price;
        self.home_bought.push((call.def.id.clone(), area, file));
        let cost = if f.price == 0 {
            "free".to_string()
        } else {
            format!("-{}¢", f.price)
        };
        Ok(format!("downloaded {} ({cost}): {what}.", f.file))
    }

    /// Reads a bulletin or post on a board at home: what it does, if anything, lands on the
    /// save. Returns the line to print: empty if it was already noted.
    pub fn home_read(
        &mut self,
        data: &Data,
        board: usize,
        r: crate::game::Reading,
    ) -> Result<String, String> {
        let def = data.boards.get(board).ok_or("no such board.")?;
        let effect = r.effect(def).ok_or("nothing to note.")?.clone();
        Ok(self
            .read_effect(data, &effect, &def.name)
            .unwrap_or_default())
    }

    /// Takes a job from a board at home. It waits on its network until you dial it.
    pub fn home_take(&mut self, data: &Data, board: usize, job: usize) -> Result<String, String> {
        let jobs = self.home_jobs(data, board);
        let (post, pending) = jobs.get(job).ok_or("no such job.")?;
        if post.taken {
            return Err("that job's taken.".into());
        }
        let line = format!(
            "job taken: {} for {}, {}¢. dial {} to do it.",
            post.target, post.client, post.reward, post.detail
        );
        self.home_taken.push((data.boards[board].id.clone(), job));
        self.pending.push(pending.clone());
        Ok(line)
    }
}
