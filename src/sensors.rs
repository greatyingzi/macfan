//! What each family of SMC temperature sensors actually measures.
//!
//! "The temperature" is not one number. An M2 exposes 126 readable sensors, and
//! they measure different places at very different scales: a CPU hot spot at
//! 92 °C and a battery cell at 36 °C are both true at the same moment, and their
//! average means nothing. This table groups them so a reading can be read in
//! context.
//!
//! The grouping and the names come from a load experiment on an M2 (13" MBP,
//! 2026-09): each family was loaded on purpose and watched, so a group is only
//! claimed when its sensors moved for the right reason.
//!
//! | family      | keys              | response measured                                  |
//! |-------------|-------------------|----------------------------------------------------|
//! | CPU cores   | `Tp0*`, `Tp1*`, `Te0*` | +24…29 °C under 8 CPU threads                   |
//! | CPU die     | `TCMz`, `TCMb`    | SMC's own die max / die average, same ramp          |
//! | GPU         | `Tg0*`, `Tg1*`    | +11…12 °C under a WebGL shader burn (CPU only +2)   |
//! | Heatsink    | `Th0*`, `Th1*`, `Th2*` | +7.5 °C under GPU load, slower and smoother     |
//! | Power delivery | `TPD*`, `TRD*` | +6 °C under sustained I/O and GPU load             |
//! | Battery     | `TB*T`            | 36.4 °C, flat under every load (a real, inert probe)|
//! | Charger     | `TCHP`            | rises when charging                                |
//! | Memory      | `TMVR`, `TVM*`    | memory rail / memory VR, 44–52 °C                  |
//! | Wireless    | `TW0*`            | +0.7 °C, small but real                            |
//! | SSD         | `Ts0*`, `Tsx*`, `TH0*` | **not reliable as a disk temperature**        |
//!
//! The SSD family deserves the warning: those keys are labelled SSD/NAND in the
//! community key databases, but on this machine 90 s of sustained writes with
//! `fsync` moved them by 3.7 °C — the same amount the die sensors moved, i.e.
//! they track board heat rather than flash activity. macOS exposes no NVMe
//! temperature either (`system_profiler SPNVMeDataType` is empty, there are no
//! temperature properties on the controller in `ioreg`, and `smartctl` cannot
//! talk to Apple's storage). So an SSD reading here is a proximity reading, and
//! is labelled as such rather than presented as a disk temperature.

use crate::smc::{Error, Smc};

/// A family of sensors that measures one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// CPU cores, clusters and fabric.
    Cpu,
    /// The SMC's own die-wide aggregates.
    CpuDie,
    /// GPU cores.
    Gpu,
    /// Heatsink / board mass.
    Heatsink,
    /// SSD or NAND proximity (see the module note: not a disk temperature).
    Ssd,
    /// Memory and its voltage regulator.
    Memory,
    /// Battery cells.
    Battery,
    /// Charger.
    Charger,
    /// Wireless module.
    Wireless,
    /// Power delivery ICs and RF delivery.
    PowerDelivery,
    /// System controller and I/O.
    System,
    /// The SMC's own derived values (`TVS*`, `TVD*`, `TVA*`, `TAO`): they repeat
    /// other readings rather than measuring a place of their own, so they are
    /// listed on request but never summarised or alarmed about — `TVD0` alone
    /// otherwise made "System" look like it was about to melt.
    Virtual,
    /// Anything else that reads a plausible value.
    Other,
}

/// Everything known about one group: how to recognise it, what to call it, and
/// where "warm" turns into "hot" for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    /// Stable identifier for scripts.
    pub id: &'static str,
    /// Human name, in English (the CLI keeps the classic tool's language).
    pub label: &'static str,
    /// [warm, hot] thresholds in °C.
    pub warm: f64,
    /// Threshold above which the reading is worth alarming about.
    pub hot: f64,
    /// Worth showing by default: a person watching a laptop cares about these.
    pub primary: bool,
}

/// How a reading compares with its group's thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Below the warm threshold.
    Fine,
    /// At or above warm, below hot.
    Warm,
    /// At or above hot.
    Hot,
}

/// The table, in display order.
pub const SPECS: &[(Group, Spec)] = &[
    (
        Group::Cpu,
        Spec {
            id: "cpu",
            label: "CPU",
            warm: 85.0,
            hot: 100.0,
            primary: true,
        },
    ),
    (
        Group::CpuDie,
        Spec {
            id: "cpudie",
            label: "CPU die",
            warm: 80.0,
            hot: 95.0,
            primary: false,
        },
    ),
    (
        Group::Gpu,
        Spec {
            id: "gpu",
            label: "GPU",
            warm: 75.0,
            hot: 90.0,
            primary: true,
        },
    ),
    (
        Group::Heatsink,
        Spec {
            id: "heatsink",
            label: "Heatsink",
            warm: 65.0,
            hot: 80.0,
            primary: true,
        },
    ),
    (
        Group::Ssd,
        Spec {
            id: "ssd",
            label: "SSD (proximity)",
            warm: 60.0,
            hot: 75.0,
            primary: false,
        },
    ),
    (
        Group::Memory,
        Spec {
            id: "memory",
            label: "Memory",
            warm: 55.0,
            hot: 70.0,
            primary: false,
        },
    ),
    (
        Group::Battery,
        Spec {
            id: "battery",
            label: "Battery",
            warm: 40.0,
            hot: 45.0,
            primary: true,
        },
    ),
    (
        Group::Charger,
        Spec {
            id: "charger",
            label: "Charger",
            warm: 45.0,
            hot: 55.0,
            primary: false,
        },
    ),
    (
        Group::Wireless,
        Spec {
            id: "wireless",
            label: "Wireless",
            warm: 50.0,
            hot: 60.0,
            primary: false,
        },
    ),
    (
        Group::PowerDelivery,
        Spec {
            id: "power",
            label: "Power delivery",
            warm: 70.0,
            hot: 85.0,
            primary: false,
        },
    ),
    (
        Group::System,
        Spec {
            id: "system",
            label: "System",
            warm: 70.0,
            hot: 85.0,
            primary: false,
        },
    ),
    (
        Group::Other,
        Spec {
            id: "other",
            label: "Other",
            warm: 70.0,
            hot: 85.0,
            primary: false,
        },
    ),
];

/// The spec for a group.
pub fn spec(group: Group) -> Spec {
    SPECS
        .iter()
        .find(|(candidate, _)| *candidate == group)
        .map(|(_, spec)| *spec)
        .expect("every group has a spec")
}

/// Where a group stands relative to its own thresholds.
pub fn verdict(group: Group, celsius: f64) -> Verdict {
    let spec = spec(group);
    if celsius >= spec.hot {
        Verdict::Hot
    } else if celsius >= spec.warm {
        Verdict::Warm
    } else {
        Verdict::Fine
    }
}

/// Exact keys that do not follow their family's prefix rule.
const EXACT: &[(&str, Group)] = &[
    ("TCMz", Group::CpuDie),
    ("TCMb", Group::CpuDie),
    ("TCHP", Group::Charger),
    ("TSCD", Group::System),
    ("TIOP", Group::System),
    ("TMVR", Group::Memory),
    ("TW0P", Group::Wireless),
    ("TAO", Group::Other),
];

/// Classify a sensor key.
pub fn group_of(key: &str) -> Group {
    if let Some((_, group)) = EXACT.iter().find(|(name, _)| *name == key) {
        return *group;
    }
    let bytes = key.as_bytes();
    if bytes.first() != Some(&b'T') {
        return Group::Other;
    }
    match bytes.get(1) {
        // CPU cores, efficiency cores, cluster aggregates and fabric.
        Some(b'p') | Some(b'e') => match bytes.get(2) {
            Some(b'0') | Some(b'1') | Some(b'2') => Group::Cpu,
            _ => Group::Other,
        },
        // GPU cores.
        Some(b'g') => Group::Gpu,
        // Heatsink and board mass.
        Some(b'h') => Group::Heatsink,
        // SSD / NAND proximity.
        Some(b's') | Some(b'H') | Some(b'N') => Group::Ssd,
        // Memory rail and memory VR (`Tm0p`, `Tm1p` on Intel Macs).
        Some(b'm') => Group::Memory,
        // Battery cells.
        Some(b'B') => Group::Battery,
        // Power delivery and RF delivery.
        Some(b'P') | Some(b'R') => Group::PowerDelivery,
        // Wireless.
        Some(b'W') => Group::Wireless,
        // `TVM*` is the memory rail, while `TVS*`/`TVD*`/`TVA*` are the SMC's
        // *virtual* sensors: die-like readings that must not be filed as memory,
        // where they once put a 75 °C "memory temperature" in the CLI.
        Some(b'V') => match bytes.get(2) {
            Some(b'M') => Group::Memory,
            _ => Group::Virtual,
        },
        // System controller, I/O.
        Some(b'S') | Some(b'I') => Group::System,
        _ => Group::Other,
    }
}

/// A human name for a sensor key, as far as it can be told from the family and
/// the key's own shape. Deliberately conservative: no invented precision.
pub fn describe(key: &str) -> String {
    let group = spec(group_of(key)).label;
    let tail = &key[2..];
    match group_of(key) {
        Group::Cpu => match key.as_bytes().get(1) {
            Some(b'e') => format!("CPU efficiency core ({tail})"),
            _ => format!("CPU core / cluster ({tail})"),
        },
        // The SMC's own die-wide aggregates, with the names its documentation
        // and the community key databases agree on.
        Group::CpuDie => match key {
            "TCMz" => "CPU die maximum".to_string(),
            "TCMb" => "CPU die average".to_string(),
            _ => format!("CPU die ({tail})"),
        },
        Group::Gpu => format!("GPU core ({tail})"),
        Group::Heatsink => format!("Heatsink ({tail})"),
        Group::Ssd => format!("SSD / NAND proximity ({tail})"),
        Group::Memory => format!("Memory ({tail})"),
        Group::Battery => format!("Battery cell ({tail})"),
        Group::Charger => "Charger".to_string(),
        Group::Wireless => format!("Wireless ({tail})"),
        Group::PowerDelivery => format!("Power delivery ({tail})"),
        Group::System => format!("System / I/O ({tail})"),
        Group::Virtual => format!("derived value ({tail})"),
        Group::Other => format!("{group} ({tail})"),
    }
}

/// One sensor reading.
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    /// SMC key.
    pub key: String,
    /// Temperature in °C.
    pub celsius: f64,
    /// Encoding the sensor uses: `"flt"` on Apple Silicon, `"sp78"` on Intel.
    pub kind: &'static str,
}

/// Aggregate of one group.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// Number of sensors included.
    pub count: usize,
    /// Highest reading.
    pub max: f64,
    /// Mean reading.
    pub mean: f64,
}

impl Stats {
    fn of(values: &[f64]) -> Option<Stats> {
        if values.is_empty() {
            return None;
        }
        Some(Stats {
            count: values.len(),
            max: values.iter().copied().fold(f64::MIN, f64::max),
            mean: values.iter().sum::<f64>() / values.len() as f64,
        })
    }
    /// How the group's hot spot stands.
    pub fn verdict(&self, group: Group) -> Verdict {
        verdict(group, self.max)
    }
}

/// Aggregate one group out of a set of readings. Sensors that read zero are
/// skipped whatever their key says: they are unwired, and counting them would
/// drag a mean down for no reason.
pub fn stats_for(readings: &[Reading], group: Group) -> Option<Stats> {
    let values: Vec<f64> = readings
        .iter()
        .filter(|r| group_of(&r.key) == group && r.celsius > 0.0)
        .map(|r| r.celsius)
        .collect();
    Stats::of(&values)
}

/// Every group that has at least one readable sensor, in display order.
///
/// Derived values are left out: they are not measurements of a place, and
/// including them invites false alarms (see [`Group::Virtual`]).
pub fn summary(readings: &[Reading]) -> Vec<(Group, Stats)> {
    SPECS
        .iter()
        .filter(|(group, _)| *group != Group::Virtual)
        .filter_map(|(group, _)| stats_for(readings, *group).map(|stats| (*group, stats)))
        .collect()
}

/// Keys belonging to a group. Discovered once: the SMC's key list does not
/// change while the machine is up, and reading all 126 keys on every refresh
/// would be wasteful when only one group is wanted.
pub fn keys_in(smc: &Smc, group: Group) -> Result<Vec<String>, Error> {
    Ok(smc
        .all_keys()?
        .into_iter()
        .filter(|key| group_of(key) == group)
        .collect())
}

/// Aggregate a group by reading only the keys it is made of.
pub fn stats_of(smc: &Smc, keys: &[String]) -> Option<Stats> {
    let values: Vec<f64> = keys
        .iter()
        .filter_map(|key| {
            let value = smc.read(key).ok()?;
            let (celsius, _) =
                crate::decode::temperature(&value.data_type, value.data_size, value.payload())?;
            (celsius > 0.0).then_some(celsius)
        })
        .collect();
    Stats::of(&values)
}

/// An exponential moving average that uses the real time between samples.
///
/// Readings move a few degrees between two reads — measured ±3…5 °C on the CPU
/// hot spot — so anything shown to a person wants smoothing; using the measured
/// interval rather than a fixed step keeps the window honest when the refresh
/// rate changes.
#[derive(Debug, Clone)]
pub struct Smoothed {
    tau: f64,
    value: Option<f64>,
    peak: Option<f64>,
}

impl Smoothed {
    /// A smoother with the given time constant, in seconds.
    pub fn new(tau_seconds: f64) -> Self {
        Smoothed {
            tau: tau_seconds.max(0.001),
            value: None,
            peak: None,
        }
    }

    /// Feed a sample taken `dt_seconds` after the previous one.
    pub fn push(&mut self, sample: f64, dt_seconds: f64) -> f64 {
        let alpha = 1.0 - (-dt_seconds / self.tau).exp();
        let value = match self.value {
            Some(previous) => previous + alpha * (sample - previous),
            None => sample,
        };
        self.value = Some(value);
        self.peak = Some(match self.peak {
            Some(peak) => peak.max(sample),
            None => sample,
        });
        value
    }

    /// The smoothed value, once something has been fed in.
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// The highest raw sample seen since the last reset.
    pub fn peak(&self) -> Option<f64> {
        self.peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(key: &str, celsius: f64) -> Reading {
        Reading {
            key: key.to_string(),
            celsius,
            kind: "flt",
        }
    }

    #[test]
    fn each_family_lands_in_its_group() {
        for (key, group) in [
            ("Tp01", Group::Cpu),
            ("Tp0n", Group::Cpu),
            ("Tp1a", Group::Cpu),
            ("Te06", Group::Cpu),
            ("TCMz", Group::CpuDie),
            ("TCMb", Group::CpuDie),
            ("Tg0n", Group::Gpu),
            ("Th0A", Group::Heatsink),
            ("Ts0Y", Group::Ssd),
            ("Ts0P", Group::Ssd),
            ("TH0a", Group::Ssd),
            ("TMVR", Group::Memory),
            ("TVM0", Group::Memory),
            ("TB0T", Group::Battery),
            ("TCHP", Group::Charger),
            ("TW0P", Group::Wireless),
            ("TPDX", Group::PowerDelivery),
            ("TSCD", Group::System),
            ("TIOP", Group::System),
            ("F0Ac", Group::Other),
            ("", Group::Other),
        ] {
            assert_eq!(group_of(key), group, "{key}");
        }
    }

    #[test]
    fn virtual_sensors_are_not_memory() {
        // TVS*/TVD*/TVA* sit next to TVM* in the key space and read die-like
        // values, but they are the SMC's virtual sensors: filing them under
        // Memory once put a 75 °C "memory temperature" in the CLI.
        assert_eq!(group_of("TVS0"), Group::Virtual);
        assert_eq!(group_of("TVSx"), Group::Virtual);
        assert_eq!(group_of("TVD0"), Group::Virtual);
        assert_eq!(group_of("TVA0"), Group::Virtual);
        assert_eq!(group_of("TVM0"), Group::Memory);
        assert_eq!(group_of("TVM1"), Group::Memory);
        assert_eq!(group_of("TMVR"), Group::Memory);
    }

    #[test]
    fn a_flat_battery_is_fine_while_a_cpu_at_the_same_number_is_hot() {
        // The thresholds are per group: 45 °C is alarming for a battery and
        // unremarkable for a CPU rail.
        assert_eq!(verdict(Group::Battery, 44.9), Verdict::Warm);
        assert_eq!(verdict(Group::Battery, 30.0), Verdict::Fine);
        assert_eq!(verdict(Group::Cpu, 44.9), Verdict::Fine);
        assert_eq!(verdict(Group::Cpu, 100.0), Verdict::Hot);
    }

    #[test]
    fn stats_skip_zero_readings_and_stay_inside_one_group() {
        let readings = vec![
            reading("Tp01", 70.0),
            reading("Tp02", 90.0),
            reading("Tp03", 0.0),
            reading("TAO", 0.0),
            reading("TB0T", 35.0),
        ];
        let cpu = stats_for(&readings, Group::Cpu).expect("cpu");
        assert_eq!(cpu.count, 2);
        assert_eq!(cpu.max, 90.0);
        assert_eq!(cpu.mean, 80.0);
        assert_eq!(stats_for(&readings, Group::Gpu), None);
        let battery = stats_for(&readings, Group::Battery).expect("battery");
        assert_eq!(battery.max, 35.0);
        assert_eq!(battery.verdict(Group::Battery), Verdict::Fine);
    }

    #[test]
    fn the_summary_is_in_display_order_and_skips_absent_groups() {
        let readings = vec![reading("Tp01", 70.0), reading("TB0T", 35.0)];
        let groups: Vec<Group> = summary(&readings).into_iter().map(|(g, _)| g).collect();
        assert_eq!(groups, vec![Group::Cpu, Group::Battery]);
    }

    #[test]
    fn the_primary_groups_are_the_ones_a_laptop_owner_watches() {
        let primary: Vec<&str> = SPECS
            .iter()
            .filter(|(_, spec)| spec.primary)
            .map(|(_, spec)| spec.id)
            .collect();
        assert_eq!(primary, vec!["cpu", "gpu", "heatsink", "battery"]);
        assert!(
            !spec(Group::Ssd).primary,
            "SSD proximity is not a disk temperature"
        );
    }

    #[test]
    fn smoothing_converges_and_remembers_the_peak() {
        let mut smoother = Smoothed::new(15.0);
        assert_eq!(smoother.push(60.0, 2.0), 60.0);
        let after_step = smoother.push(90.0, 2.0);
        assert!(after_step > 60.0 && after_step < 90.0, "{after_step}");
        let mut value = after_step;
        for _ in 0..60 {
            value = smoother.push(90.0, 2.0);
        }
        assert!((value - 90.0).abs() < 0.5, "{value}");
        assert_eq!(smoother.peak(), Some(90.0));
        // The window is in seconds, not in samples.
        let mut slow = Smoothed::new(15.0);
        slow.push(60.0, 2.0);
        let slow_step = slow.push(90.0, 2.0);
        let mut fast = Smoothed::new(15.0);
        fast.push(60.0, 2.0);
        let fast_step = fast.push(90.0, 8.0);
        assert!(fast_step > slow_step);
    }

    #[test]
    fn a_time_constant_of_zero_is_clamped_instead_of_dividing_by_zero() {
        let mut smoother = Smoothed::new(0.0);
        assert_eq!(smoother.push(10.0, 1.0), 10.0);
        assert!(smoother.push(20.0, 1.0).is_finite());
    }

    #[test]
    fn descriptions_say_what_the_key_is_without_inventing_detail() {
        assert_eq!(describe("Tp0A"), "CPU core / cluster (0A)");
        assert_eq!(describe("Te06"), "CPU efficiency core (06)");
        assert_eq!(describe("TCMz"), "CPU die maximum");
        assert_eq!(describe("TCMb"), "CPU die average");
        assert_eq!(describe("TCHP"), "Charger");
        assert!(describe("Ts0Y").contains("proximity"));
        assert_eq!(describe("TH0a"), "SSD / NAND proximity (0a)");
    }
}
