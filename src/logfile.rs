#![allow(dead_code)]
#![allow(non_snake_case)]

use std::ffi::OsString;
use std::io;
use std::path::Path;

use chrono::{Datelike, Timelike};

mod keysight_log;

// Removes the index from the testname.
// For example: "17%c617" -> "c617"
fn strip_index(s: &str) -> &str {
    let mut chars = s.chars();

    let mut c = chars.next();
    while c.is_some() {
        if c.unwrap() == '%' {
            break;
        }
        c = chars.next();
    }

    chars.as_str()
}

// YYMMDDhhmmss => YY.MM.DD. hh:mm:ss
pub fn u64_to_string(mut x: u64) -> String {
    let YY = x / u64::pow(10, 10);
    x %= u64::pow(10, 10);

    let MM = x / u64::pow(10, 8);
    x %= u64::pow(10, 8);

    let DD = x / u64::pow(10, 6);
    x %= u64::pow(10, 6);

    let hh = x / u64::pow(10, 4);
    x %= u64::pow(10, 4);

    let mm = x / u64::pow(10, 2);
    x %= u64::pow(10, 2);

    format!(
        "{:02.0}.{:02.0}.{:02.0} {:02.0}:{:02.0}:{:02.0}",
        YY, MM, DD, hh, mm, x
    )
}

fn local_time_to_u64(t: chrono::DateTime<chrono::Local>) -> u64 {
    (t.year() as u64 - 2000) * u64::pow(10, 10)
        + t.month() as u64 * u64::pow(10, 8)
        + t.day() as u64 * u64::pow(10, 6)
        + t.hour() as u64 * u64::pow(10, 4)
        + t.minute() as u64 * u64::pow(10, 2)
        + t.second() as u64
}

pub type TResult = (BResult, f32);
type TList = (String, TType);

#[derive(Clone, Copy, PartialEq)]
pub enum TLimit {
    None,
    Lim2(f32, f32),      // UL - LL
    Lim3(f32, f32, f32), // Nom - UL - LL
}

#[derive(Clone, Copy, PartialEq)]
pub enum TType {
    Pin,
    Shorts,
    Jumper,
    Fuse,
    Resistor,
    Capacitor,
    Inductor,
    Diode,
    Zener,
    NFet,
    PFet,
    Npn,
    Pnp,
    Pot,
    Switch,
    Testjet,
    Digital,
    Measurement,
    Current,
    BoundaryS,
    Unknown,
}

impl From<keysight_log::AnalogTest> for TType {
    fn from(value: keysight_log::AnalogTest) -> Self {
        match value {
            keysight_log::AnalogTest::Cap => TType::Capacitor,
            keysight_log::AnalogTest::Diode => TType::Diode,
            keysight_log::AnalogTest::Fuse => TType::Fuse,
            keysight_log::AnalogTest::Inductor => TType::Inductor,
            keysight_log::AnalogTest::Jumper => TType::Jumper,
            keysight_log::AnalogTest::Measurement => TType::Measurement,
            keysight_log::AnalogTest::NFet => TType::NFet,
            keysight_log::AnalogTest::PFet => TType::PFet,
            keysight_log::AnalogTest::Npn => TType::Npn,
            keysight_log::AnalogTest::Pnp => TType::Pnp,
            keysight_log::AnalogTest::Pot => TType::Pot,
            keysight_log::AnalogTest::Res => TType::Resistor,
            keysight_log::AnalogTest::Switch => TType::Switch,
            keysight_log::AnalogTest::Zener => TType::Zener,
            keysight_log::AnalogTest::Error => TType::Unknown,
        }
    }
}

impl TType {
    fn print(&self) -> String {
        match self {
            TType::Pin => "Pin".to_string(),
            TType::Shorts => "Shorts".to_string(),
            TType::Jumper => "Jumper".to_string(),
            TType::Fuse => "Fuse".to_string(),
            TType::Resistor => "Resistor".to_string(),
            TType::Capacitor => "Capacitor".to_string(),
            TType::Inductor => "Inductor".to_string(),
            TType::Diode => "Diode".to_string(),
            TType::Zener => "Zener".to_string(),
            TType::Testjet => "Testjet".to_string(),
            TType::Digital => "Digital".to_string(),
            TType::Measurement => "Measurement".to_string(),
            TType::Current => "Current".to_string(),
            TType::BoundaryS => "Boundary Scan".to_string(),
            TType::Unknown => "Unknown".to_string(),
            TType::NFet => "N-FET".to_string(),
            TType::PFet => "P-FET".to_string(),
            TType::Npn => "NPN".to_string(),
            TType::Pnp => "PNP".to_string(),
            TType::Pot => "Pot".to_string(),
            TType::Switch => "Switch".to_string(),
        }
    }

    pub fn unit(&self) -> String {
        match self {
            TType::Pin | TType::Shorts => "Result".to_string(),
            TType::Jumper | TType::Fuse | TType::Resistor => "Ω".to_string(),
            TType::Capacitor => "F".to_string(),
            TType::Inductor => "H".to_string(),
            TType::Diode | TType::Zener => "V".to_string(),
            TType::NFet | TType::PFet | TType::Npn | TType::Pnp => "V".to_string(),
            TType::Pot | TType::Switch => "Ω".to_string(),
            TType::Testjet => "Result".to_string(),
            TType::Digital => "Result".to_string(),
            TType::Measurement => "V".to_string(),
            TType::Current => "A".to_string(),
            TType::BoundaryS => "Result".to_string(),
            TType::Unknown => "Result".to_string(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum BResult {
    Pass,
    Fail,
    Unknown,
}

impl From<BResult> for bool {
    fn from(val: BResult) -> Self {
        matches!(val, BResult::Pass)
    }
}

impl From<bool> for BResult {
    fn from(value: bool) -> Self {
        if value {
            return BResult::Pass;
        }

        BResult::Fail
    }
}

impl From<i32> for BResult {
    fn from(value: i32) -> Self {
        if value == 0 {
            BResult::Pass
        } else {
            BResult::Fail
        }
    }
}

impl From<&str> for BResult {
    fn from(value: &str) -> Self {
        if matches!(value, "0" | "00") {
            return BResult::Pass;
        }

        BResult::Fail
    }
}

impl BResult {
    pub fn print(&self) -> String {
        match self {
            BResult::Pass => String::from("Pass"),
            BResult::Fail => String::from("Fail"),
            BResult::Unknown => String::from("NA"),
        }
    }

    pub fn into_color(self) -> egui::Color32 {
        match self {
            BResult::Pass => egui::Color32::GREEN,
            BResult::Fail => egui::Color32::RED,
            BResult::Unknown => egui::Color32::YELLOW,
        }
    }

    pub fn into_dark_color(self) -> egui::Color32 {
        match self {
            BResult::Pass => egui::Color32::DARK_GREEN,
            BResult::Fail => egui::Color32::RED,
            BResult::Unknown => egui::Color32::BLACK,
        }
    }
}

#[derive(Clone)]
pub struct Test {
    pub name: String,
    pub ttype: TType,

    pub result: TResult,
    pub limits: TLimit,
}

impl Test {
    fn clear(&mut self) {
        self.name = String::new();
        self.ttype = TType::Unknown;
        self.result = (BResult::Unknown, 0.0);
        self.limits = TLimit::None;
    }
}

pub struct LogFile {
    pub source: OsString,
    pub DMC: String,
    pub DMC_mb: String,
    pub product_id: String,
    pub index: usize,

    pub result: bool,
    pub status: i32,
    pub status_str: String,

    pub time_start: u64,
    pub time_end: u64,

    pub tests: Vec<Test>,
    pub report: String,
}

impl LogFile {
    pub fn load_v2(p: &Path) -> io::Result<Self> {
        println!("INFO: Loading (v2) file {}", p.display());
        let source = p.as_os_str().to_owned();

        let mut product_id = String::from("NoID");
        //let mut revision_id = String::new(); // ! New, needs to be implemented in the program

        let mut DMC = String::from("NoDMC");
        let mut DMC_mb = String::from("NoMB");
        let mut index = 1;
        let mut time_start: u64 = 0;
        let mut time_end: u64 = 0;
        let mut status = 0;

        let mut tests: Vec<Test> = Vec::new();
        let mut report: Vec<String> = Vec::new();
        let mut failed_nodes: Vec<String> = Vec::new();
        let mut failed_pins: Vec<String> = Vec::new();

        // pre-populate pins test
        tests.push(Test {
            name: "pins".to_owned(),
            ttype: TType::Pin,
            result: (BResult::Unknown, 0.0),
            limits: TLimit::None,
        });
        //

        // Variables for user defined blocks:
        let mut PS_counter = 0;
        //

        let tree = keysight_log::parse_file(p)?;
        let mut batch_node: Option<&keysight_log::TreeNode> = None;
        let mut btest_node: Option<&keysight_log::TreeNode> = None;

        if let Some(batch) = tree.last() {
            // {@BATCH|UUT type|UUT type rev|fixture id|testhead number|testhead type|process step|batch id|
            //      operator id|controller|testplan id|testplan rev|parent panel type|parent panel type rev (| version label)}
            if let keysight_log::KeysightPrefix::Batch(
                p_id,
                _, //r_id,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
                _,
            ) = &batch.data
            {
                product_id = p_id.clone();
                //revision_id = r_id.clone();
                batch_node = Some(batch);
            } else {
                eprintln!("W: No BATCH field found!");
            }
        }

        if let Some(btest) = {
            if let Some(x) = batch_node {
                x.branches.last()
            } else {
                tree.last()
            }
        } {
            // {@BTEST|board id|test status|start datetime|duration|multiple test|log level|log set|learning|
            // known good|end datetime|status qualifier|board number|parent panel id}
            if let keysight_log::KeysightPrefix::BTest(
                b_id,
                b_status,
                t_start,
                _,
                _,
                _,
                _,
                _,
                _,
                t_end,
                _,
                b_index,
                mb_id,
            ) = &btest.data
            {
                DMC = b_id.clone();

                if let Some(mb) = mb_id {
                    DMC_mb = mb.clone();
                } else {
                    DMC_mb = DMC.clone();
                }

                status = *b_status;
                time_start = *t_start;
                time_end = *t_end;
                index = *b_index as usize;
                btest_node = Some(btest);
            } else {
                eprintln!("W: No BTEST field found!");
            }
        }

        let test_nodes = if let Some(x) = btest_node {
            &x.branches
        } else {
            &tree
        };

        for test in test_nodes {
            match &test.data {
                // I haven't encountered any analog fields outside of a BLOCK, so this might be not needed.
                keysight_log::KeysightPrefix::Analog(analog, status, result, sub_name) => {
                    if let Some(name) = sub_name {
                        let limits = match test.branches.first() {
                            Some(lim) => match lim.data {
                                keysight_log::KeysightPrefix::Lim2(max, min) => {
                                    TLimit::Lim2(max, min)
                                }
                                keysight_log::KeysightPrefix::Lim3(nom, max, min) => {
                                    TLimit::Lim3(nom, max, min)
                                }
                                _ => {
                                    eprintln!(
                                        "ERR: Analog test limit parsing error!\n\t{:?}",
                                        lim.data
                                    );
                                    TLimit::None
                                }
                            },
                            None => TLimit::None,
                        };

                        for subfield in test.branches.iter().skip(1) {
                            match &subfield.data {
                                keysight_log::KeysightPrefix::Report(rpt) => {
                                    report.push(rpt.clone());
                                }
                                _ => {
                                    eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                                }
                            }
                        }

                        tests.push(Test {
                            name: strip_index(name).to_string(),
                            ttype: TType::from(*analog),
                            result: (BResult::from(*status), *result),
                            limits,
                        })
                    } else {
                        eprintln!(
                            "ERR: Analog test outside of a BLOCK and without name!\n\t{:?}",
                            test.data
                        );
                    }
                }
                keysight_log::KeysightPrefix::AlarmId(_, _) => todo!(),
                keysight_log::KeysightPrefix::Alarm(_, _, _, _, _, _, _, _, _) => todo!(),
                keysight_log::KeysightPrefix::Array(_, _, _, _) => todo!(),
                keysight_log::KeysightPrefix::Block(b_name, _) => {
                    let block_name = strip_index(b_name).to_string();
                    let mut digital_tp: Option<usize> = None;
                    let mut boundary_tp: Option<usize> = None;

                    for sub_test in &test.branches {
                        match &sub_test.data {
                            keysight_log::KeysightPrefix::Analog(
                                analog,
                                status,
                                result,
                                sub_name,
                            ) => {
                                let limits = match sub_test.branches.first() {
                                    Some(lim) => match lim.data {
                                        keysight_log::KeysightPrefix::Lim2(max, min) => {
                                            TLimit::Lim2(max, min)
                                        }
                                        keysight_log::KeysightPrefix::Lim3(nom, max, min) => {
                                            TLimit::Lim3(nom, max, min)
                                        }
                                        _ => {
                                            eprintln!(
                                                "ERR: Analog test limit parsing error!\n\t{:?}",
                                                lim.data
                                            );
                                            TLimit::None
                                        }
                                    },
                                    None => TLimit::None,
                                };

                                for subfield in sub_test.branches.iter().skip(1) {
                                    match &subfield.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        _ => {
                                            eprintln!(
                                                "ERR: Unhandled subfield!\n\t{:?}",
                                                subfield.data
                                            )
                                        }
                                    }
                                }

                                let mut name = block_name.clone();
                                if let Some(sn) = &sub_name {
                                    name = format!("{}%{}", name, sn);
                                }

                                tests.push(Test {
                                    name,
                                    ttype: TType::from(*analog),
                                    result: (BResult::from(*status), *result),
                                    limits,
                                })
                            }
                            keysight_log::KeysightPrefix::Digital(status, _, _, _, sub_name) => {
                                // subrecords: DPIN - ToDo!

                                for subfield in sub_test.branches.iter() {
                                    match &subfield.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        _ => {
                                            eprintln!(
                                                "ERR: Unhandled subfield!\n\t{:?}",
                                                subfield.data
                                            )
                                        }
                                    }
                                }

                                if let Some(dt) = digital_tp {
                                    if *status != 0 {
                                        tests[dt].result = (BResult::from(*status), *status as f32);
                                    }
                                } else {
                                    digital_tp = Some(tests.len());
                                    tests.push(Test {
                                        name: strip_index(sub_name).to_string(),
                                        ttype: TType::Digital,
                                        result: (BResult::from(*status), *status as f32),
                                        limits: TLimit::None,
                                    });
                                }
                            }
                            keysight_log::KeysightPrefix::TJet(status, _, sub_name) => {
                                for subfield in sub_test.branches.iter() {
                                    match &subfield.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        keysight_log::KeysightPrefix::DPin(_, pins) => {
                                            let mut tmp: Vec<String> =
                                                pins.iter().map(|f| f.0.clone()).collect();
                                            failed_nodes.append(&mut tmp);
                                        }
                                        _ => {
                                            eprintln!(
                                                "ERR: Unhandled subfield!\n\t{:?}",
                                                subfield.data
                                            )
                                        }
                                    }
                                }

                                let name = format!("{}%{}", block_name, strip_index(sub_name));
                                tests.push(Test {
                                    name,
                                    ttype: TType::Testjet,
                                    result: (BResult::from(*status), *status as f32),
                                    limits: TLimit::None,
                                })
                            }
                            keysight_log::KeysightPrefix::Boundary(sub_name, status, _, _) => {
                                // Subrecords: BS-O, BS-S - ToDo

                                for subfield in sub_test.branches.iter() {
                                    match &subfield.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        _ => {
                                            eprintln!(
                                                "ERR: Unhandled subfield!\n\t{:?}",
                                                subfield.data
                                            )
                                        }
                                    }
                                }

                                if let Some(dt) = boundary_tp {
                                    if *status != 0 {
                                        tests[dt].result = (BResult::from(*status), *status as f32);
                                    }
                                } else {
                                    boundary_tp = Some(tests.len());
                                    tests.push(Test {
                                        name: strip_index(sub_name).to_string(),
                                        ttype: TType::BoundaryS,
                                        result: (BResult::from(*status), *status as f32),
                                        limits: TLimit::None,
                                    })
                                }
                            }
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            keysight_log::KeysightPrefix::UserDefined(s) => {
                                eprintln!("ERR: Not implemented USER DEFINED block!\n\t{:?}", s);
                            }
                            keysight_log::KeysightPrefix::Error(s) => {
                                eprintln!("ERR: KeysightPrefix::Error found!\n\t{:?}", s);
                            }
                            _ => {
                                eprintln!(
                                    "ERR: Found a invalid field nested in BLOCK!\n\t{:?}",
                                    sub_test.data
                                );
                            }
                        }
                    }
                }

                // Boundary exists in BLOCK and as a solo filed if it fails.
                keysight_log::KeysightPrefix::Boundary(test_name, status, _, _) => {
                    // Subrecords: BS-O, BS-S - ToDo

                    for subfield in test.branches.iter() {
                        match &subfield.data {
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            _ => {
                                eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                            }
                        }
                    }

                    tests.push(Test {
                        name: strip_index(test_name).to_string(),
                        ttype: TType::BoundaryS,
                        result: (BResult::from(*status), *status as f32),
                        limits: TLimit::None,
                    })
                }

                // Digital tests can be present as a BLOCK member, or solo.
                keysight_log::KeysightPrefix::Digital(status, _, _, _, test_name) => {
                    for subfield in test.branches.iter() {
                        match &subfield.data {
                            keysight_log::KeysightPrefix::DPin(_, pins) => {
                                let mut tmp: Vec<String> =
                                    pins.iter().map(|f| f.0.clone()).collect();
                                failed_nodes.append(&mut tmp);
                            }
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            _ => {
                                eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                            }
                        }
                    }

                    tests.push(Test {
                        name: strip_index(test_name).to_string(),
                        ttype: TType::Digital,
                        result: (BResult::from(*status), *status as f32),
                        limits: TLimit::None,
                    })
                }
                keysight_log::KeysightPrefix::Pins(_, status, _) => {
                    // Subrecord: Pin - ToDo
                    for subfield in test.branches.iter() {
                        match &subfield.data {
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            keysight_log::KeysightPrefix::Pin(pin) => {
                                failed_pins.append(&mut pin.clone());
                            }
                            _ => {
                                eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                            }
                        }
                    }

                    tests[0].result = (BResult::from(*status), *status as f32);
                }
                keysight_log::KeysightPrefix::Report(rpt) => {
                    report.push(rpt.clone());
                }

                // I haven't encountered any testjet fields outside of a BLOCK, so this might be not needed.
                keysight_log::KeysightPrefix::TJet(status, _, test_name) => {
                    // subrecords: DPIN - ToDo!
                    for subfield in test.branches.iter() {
                        match &subfield.data {
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            _ => {
                                eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                            }
                        }
                    }

                    tests.push(Test {
                        name: strip_index(test_name).to_string(),
                        ttype: TType::Testjet,
                        result: (BResult::from(*status), *status as f32),
                        limits: TLimit::None,
                    })
                }
                keysight_log::KeysightPrefix::Shorts(mut status, s1, s2, s3, _) => {
                    // Sometimes, failed shorts tests are marked as passed at the 'test status' field.
                    // So we check the next 3 fields too, they all have to be '000'
                    if *s1 > 0 || *s2 > 0 || *s3 > 0 {
                        status = 1;
                    }

                    for subfield in test.branches.iter() {
                        match &subfield.data {
                            keysight_log::KeysightPrefix::Report(rpt) => {
                                report.push(rpt.clone());
                            }
                            keysight_log::KeysightPrefix::ShortsSrc(_, _, node) => {
                                failed_nodes.push(node.clone());
                                for sub2 in &subfield.branches {
                                    match &sub2.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        keysight_log::KeysightPrefix::ShortsDest(dst) => {
                                            let mut tmp: Vec<String> =
                                                dst.iter().map(|d| d.0.clone()).collect();
                                            failed_nodes.append(&mut tmp);
                                        }
                                        _ => {
                                            eprintln!("ERR: Unhandled subfield!\n\t{:?}", sub2.data)
                                        }
                                    }
                                }
                            }
                            keysight_log::KeysightPrefix::ShortsOpen(src, dst, _) => {
                                failed_nodes.push(src.clone());
                                failed_nodes.push(dst.clone());

                                for sub2 in &subfield.branches {
                                    match &sub2.data {
                                        keysight_log::KeysightPrefix::Report(rpt) => {
                                            report.push(rpt.clone());
                                        }
                                        _ => {
                                            eprintln!("ERR: Unhandled subfield!\n\t{:?}", sub2.data)
                                        }
                                    }
                                }
                            }
                            _ => {
                                eprintln!("ERR: Unhandled subfield!\n\t{:?}", subfield.data)
                            }
                        }
                    }

                    tests.push(Test {
                        name: String::from("shorts"),
                        ttype: TType::Shorts,
                        result: (BResult::from(status), status as f32),
                        limits: TLimit::None,
                    })
                }
                keysight_log::KeysightPrefix::UserDefined(s) => match s[0].as_str() {
                    "@Programming_time" => {
                        if s.len() < 2 {
                            eprintln!("ERR: Parsing error at @Programming_time!\n\t{:?}", s);
                            continue;
                        }

                        if let Some(t) = s[1].strip_suffix("msec") {
                            if let Ok(ts) = t.parse::<i32>() {
                                tests.push(Test {
                                    name: String::from("Programming_time"),
                                    ttype: TType::Unknown,
                                    result: (BResult::Pass, ts as f32 / 1000.0),
                                    limits: TLimit::None,
                                })
                            } else {
                                eprintln!("ERR: Parsing error at @Programming_time!\n\t{:?}", s);
                            }
                        } else {
                            eprintln!("ERR: Parsing error at @Programming_time!\n\t{:?}", s);
                        }
                    }
                    "@PS_info" => {
                        if s.len() < 3 {
                            eprintln!("ERR: Parsing error at @PS_info!\n\t{:?}", s);
                            continue;
                        }

                        let voltage;
                        let current;

                        if let Some(t) = s[1].strip_suffix('V') {
                            if let Ok(ts) = t.parse::<f32>() {
                                voltage = ts;
                            } else {
                                eprintln!("ERR: Parsing error at @PS_info!\n\t{:?}", s);
                                continue;
                            }
                        } else {
                            eprintln!("ERR: Parsing error at @PS_info!\n\t{:?}", s);
                            continue;
                        }

                        if let Some(t) = s[2].strip_suffix('A') {
                            if let Ok(ts) = t.parse::<f32>() {
                                current = ts;
                            } else {
                                eprintln!("ERR: Parsing error at @PS_info!\n\t{:?}", s);
                                continue;
                            }
                        } else {
                            eprintln!("ERR: Parsing error at @PS_info!\n\t{:?}", s);
                            continue;
                        }

                        println!("{} - {}", voltage, current);
                        PS_counter += 1;
                        tests.push(Test {
                            name: format!("PS_Info_{PS_counter}%Voltage"),
                            ttype: TType::Measurement,
                            result: (BResult::Pass, voltage),
                            limits: TLimit::None,
                        });
                        tests.push(Test {
                            name: format!("PS_Info_{PS_counter}%Current"),
                            ttype: TType::Current,
                            result: (BResult::Pass, current),
                            limits: TLimit::None,
                        });
                    }
                    _ => {
                        eprintln!("ERR: Not implemented USER DEFINED block!\n\t{:?}", s);
                    }
                },
                keysight_log::KeysightPrefix::Error(s) => {
                    eprintln!("ERR: KeysightPrefix::Error found!\n\t{:?}", s);
                }
                _ => {
                    eprintln!(
                        "ERR: Found a invalid field nested in BTEST!\n\t{:?}",
                        test.data
                    );
                }
            }
        }


        if time_start == 0 {
            if let Ok(x) = p.metadata() {
                time_start = local_time_to_u64(x.modified().unwrap().into());
            }
        }

        if time_end == 0 {
            time_end = time_start;
        }

        Ok(LogFile {
            source,
            DMC,
            DMC_mb,
            product_id,
            index,
            result: status == 0,
            status,
            status_str: keysight_log::status_to_str(status),
            time_start,
            time_end,
            tests,
            report: report.join("\n"),
        })
    }
}
