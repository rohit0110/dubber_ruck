use crate::budget::Used;
use crate::config::{Config, Window};
use chrono::{NaiveDateTime, NaiveTime};

#[derive(Debug, Clone, PartialEq)]
pub enum Skip {
    OutsideActiveHours,
    QuietHours,
    CalendarBusy,
    OverBudget,
    TooSoon,
    QuizOpen,
    NotThisTick,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Ask,
    Skip(Skip),
}

/// Everything `decide` needs besides the config, so tests can fake it.
pub struct Inputs {
    pub now: NaiveDateTime,
    pub used_today: Used,
    pub last_asked: Option<NaiveDateTime>,
    pub quiz_open: bool,
    pub calendar_busy: bool,
    /// Uniform random in [0, 1).
    pub roll: f64,
}

fn parse(t: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(t, "%H:%M").ok()
}

fn in_window(w: &Window, t: NaiveTime) -> bool {
    match (parse(&w.from), parse(&w.to)) {
        (Some(a), Some(b)) if a <= b => t >= a && t < b,
        (Some(a), Some(b)) => t >= a || t < b, // wraps past midnight
        _ => false,
    }
}

/// Gates in order of cost; the first "no" wins, so the common idle case does no I/O beyond what was already read.
pub fn decide(cfg: &Config, i: &Inputs) -> Decision {
    let t = i.now.time();
    if !in_window(&cfg.active, t) {
        return Decision::Skip(Skip::OutsideActiveHours);
    }
    if cfg.quiet.iter().any(|w| in_window(w, t)) {
        return Decision::Skip(Skip::QuietHours);
    }
    if i.quiz_open {
        return Decision::Skip(Skip::QuizOpen);
    }
    if i.calendar_busy {
        return Decision::Skip(Skip::CalendarBusy);
    }
    if i.used_today.tokens >= cfg.daily_token_cap || i.used_today.cost_usd >= cfg.daily_cost_cap_usd {
        return Decision::Skip(Skip::OverBudget);
    }
    if let Some(last) = i.last_asked {
        if (i.now - last).num_minutes() < cfg.min_gap_minutes as i64 {
            return Decision::Skip(Skip::TooSoon);
        }
    }
    // Spread questions randomly: one tick asks with probability = questions per tick.
    let p = (cfg.questions_per_hour * cfg.tick_minutes as f64 / 60.0).clamp(0.0, 1.0);
    if i.roll >= p {
        return Decision::Skip(Skip::NotThisTick);
    }
    Decision::Ask
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 30).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    fn open(h: u32, m: u32) -> Inputs {
        Inputs { now: at(h, m), used_today: Used::default(), last_asked: None, quiz_open: false, calendar_busy: false, roll: 0.0 }
    }

    #[test]
    fn asks_when_every_gate_is_open() {
        assert_eq!(decide(&Config::default(), &open(10, 0)), Decision::Ask);
    }

    #[test]
    fn each_gate_blocks() {
        let c = Config::default();
        assert_eq!(decide(&c, &open(8, 0)), Decision::Skip(Skip::OutsideActiveHours));
        assert_eq!(decide(&c, &open(18, 0)), Decision::Skip(Skip::OutsideActiveHours));
        assert_eq!(decide(&c, &open(13, 0)), Decision::Skip(Skip::QuietHours));
        let mut i = open(10, 0);
        i.quiz_open = true;
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::QuizOpen));
        let mut i = open(10, 0);
        i.calendar_busy = true;
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::CalendarBusy));
        let mut i = open(10, 0);
        i.used_today = Used { tokens: c.daily_token_cap, cost_usd: 0.0 };
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::OverBudget));
        let mut i = open(10, 0);
        i.used_today = Used { tokens: 0, cost_usd: c.daily_cost_cap_usd };
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::OverBudget));
        let mut i = open(10, 0);
        i.last_asked = Some(at(9, 50));
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::TooSoon));
        i.last_asked = Some(at(9, 30));
        assert_eq!(decide(&c, &i), Decision::Ask);
        let mut i = open(10, 0);
        i.roll = 0.99;
        assert_eq!(decide(&c, &i), Decision::Skip(Skip::NotThisTick));
    }

    #[test]
    fn quiet_wins_over_active_and_windows_can_wrap_midnight() {
        let mut c = Config::default();
        c.active = Window { from: "22:00".into(), to: "02:00".into() };
        c.quiet = vec![];
        assert_eq!(decide(&c, &open(23, 0)), Decision::Ask);
        assert_eq!(decide(&c, &open(1, 0)), Decision::Ask);
        assert_eq!(decide(&c, &open(3, 0)), Decision::Skip(Skip::OutsideActiveHours));
    }

    #[test]
    fn bad_window_never_opens() {
        let mut c = Config::default();
        c.active = Window { from: "nope".into(), to: "18:00".into() };
        assert_eq!(decide(&c, &open(10, 0)), Decision::Skip(Skip::OutsideActiveHours));
    }
}
