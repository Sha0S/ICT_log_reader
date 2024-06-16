#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{env, path::PathBuf};
use eframe::egui;
use egui_extras::{Column, TableBuilder};

mod logfile;
use logfile::*;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let log_path = args.get(1).expect("No arg found!");
    let log_path = PathBuf::from(log_path);

    if !log_path.exists() {
        panic!("File {} does not exist!", log_path.to_string_lossy());
    }

    let log = LogFile::load_v2(&log_path).expect("Failed to load logfile!");
    let x = if log.report.is_empty() { 660.0 } else { 1000.0 };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(egui::Vec2 { x, y: 500.0 }),
        ..Default::default()
    };

    _ = eframe::run_native(
        "ICT Log Reader",
        options,
        Box::new(|_| Box::new(IctLogReader::default(log))),
    );
}

struct IctLogReader {
    failed_only: bool,
    search: String,
    log: LogFile,
}

impl IctLogReader {
    fn default(log: LogFile) -> Self {
        IctLogReader {
            failed_only: log.status != 0,
            search: String::new(),
            log,
        }
    }
}

impl eframe::App for IctLogReader {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.log.report.is_empty() {
            egui::SidePanel::right("Report")
                .default_width(300.0)
                .resizable(true)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.log.report.as_str())
                                    .desired_width(f32::INFINITY),
                            );
                        });
                });
        }

        egui::TopBottomPanel::top("Board Data").show(ctx, |ui| {
            egui::Grid::new("board_stats").show(ui, |ui| {
                ui.monospace("Fájl:");
                ui.monospace(format!("{}", self.log.source.to_string_lossy()));
                ui.end_row();

                ui.monospace("Termék:");
                ui.monospace(&self.log.product_id);
                ui.end_row();

                ui.monospace("DMC:");
                ui.monospace(&self.log.DMC);
                ui.end_row();

                ui.monospace("Fő DMC:");
                ui.monospace(&self.log.DMC_mb);
                ui.end_row();

                ui.monospace("Teszt ideje:");
                ui.monospace(format!(
                    "{} - {}",
                    u64_to_string(self.log.time_start),
                    u64_to_string(self.log.time_end)
                ));
                ui.end_row();

                ui.monospace("Eredmény:");
                ui.monospace(format!("{} - {}", self.log.status, self.log.status_str));
                ui.end_row();
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.monospace("Keresés: ");
                ui.text_edit_singleline(&mut self.search);
                ui.checkbox(&mut self.failed_only, "Csak a kiesőket mutassa")
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            TableBuilder::new(ui)
                .striped(true)
                .column(Column::initial(200.0).resizable(true))
                .column(Column::initial(30.0))
                .columns(Column::initial(100.0), 4)
                .header(16.0, |mut header| {
                    header.col(|ui| {
                        ui.label("Teszt");
                    });
                    header.col(|ui| {
                        ui.label(" ");
                    });
                    header.col(|ui| {
                        ui.label("Mért");
                    });
                    header.col(|ui| {
                        ui.label("Alsó határ");
                    });
                    header.col(|ui| {
                        ui.label("Középérték");
                    });
                    header.col(|ui| {
                        ui.label("Felső határ");
                    });
                })
                .body(|body| {
                    let selected_tests: Vec<&Test> = self
                        .log
                        .tests
                        .iter()
                        .filter(|f| {
                            if self.failed_only {
                                f.name.contains(&self.search) && f.result.0 == BResult::Fail
                            } else {
                                f.name.contains(&self.search)
                            }
                        })
                        .collect();
                    let total_rows = selected_tests.len();

                    body.rows(14.0, total_rows, |mut row| {
                        let row_index = row.index();
                        if let Some(test) = selected_tests.get(row_index) {
                            row.col(|ui| {
                                ui.label(&test.name);
                            });
                            row.col(|ui| {
                                ui.label(test.result.0.print());
                            });
                            row.col(|ui| {
                                ui.label(format!("{:+1.4E}", test.result.1));
                            });

                            match test.limits {
                                TLimit::None => {}
                                TLimit::Lim2(u, l) => {
                                    row.col(|ui| {
                                        ui.label(format!("{:+1.4E}", l));
                                    });
                                    row.col(|_ui| {});
                                    row.col(|ui| {
                                        ui.label(format!("{:+1.4E}", u));
                                    });
                                }
                                // Nom - UL - LL
                                TLimit::Lim3(n, u, l) => {
                                    row.col(|ui| {
                                        ui.label(format!("{:+1.4E}", l));
                                    });
                                    row.col(|ui| {
                                        ui.label(format!("{:+1.4E}", n));
                                    });
                                    row.col(|ui| {
                                        ui.label(format!("{:+1.4E}", u));
                                    });
                                }
                            }
                        }
                    });
                });
        });
    }
}
