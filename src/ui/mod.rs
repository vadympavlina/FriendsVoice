//! Native egui interface, laid out like a voice chat client:
//! channel rail on the left, call stage in the middle, text chat on the right.
//!
//! Design rules: flat surfaces separated by tone (not borders or cards), one
//! violet accent (taken from the mascot) used only for primary actions, green only for "speaking /
//! online", red only for destructive or muted states, sentence-case text.
use super::*;
use eframe::egui::{
    self, text::LayoutJob, Align2, FontFamily, FontId, Margin, Pos2, Rect, Response, RichText,
    Rounding, Sense, Stroke, Ui, Vec2,
};
use crate::screen;
use egui_phosphor::regular as ph;
use std::sync::Arc;

mod chat;
mod rail;
mod settings;
mod share;
mod stage;
mod theme;
mod tiles;
mod widgets;

pub use theme::install_theme;
use theme::*;
use tiles::*;
use widgets::*;

impl FriendsApp {
    pub(super) fn render(&mut self, ctx: &egui::Context) {
        if let Some(key) = self.fullview {
            // The stream is gone (friend stopped sharing): leave the full view after a moment.
            if self.video_textures.contains_key(&key) {
                self.fullview_missing_since = None;
            } else {
                let since = *self.fullview_missing_since.get_or_insert_with(Instant::now);
                if since.elapsed() > Duration::from_millis(2500) {
                    self.close_fullview(ctx);
                    self.fullview_missing_since = None;
                }
            }
            if self.fullview.is_some() {
                egui::CentralPanel::default().frame(egui::Frame::none().fill(Color32::BLACK))
                    .show(ctx, |ui| self.full_view(ui, key));
                if self.share_picker_open { self.share_picker(ctx); }
                return;
            }
        }
        let narrow = ctx.screen_rect().width() < 1100.0;
        if self.sidebar_open {
            egui::SidePanel::left("rail").resizable(false).exact_width(if narrow { 232.0 } else { 252.0 })
                .frame(egui::Frame::none().fill(RAIL))
                .show(ctx, |ui| self.rail(ui));
        }
        if self.settings_open {
            egui::SidePanel::left("settings_nav").resizable(false).exact_width(220.0)
                .frame(egui::Frame::none().fill(Color32::from_rgb(20, 21, 24))
                    .inner_margin(Margin::symmetric(12.0, 20.0)))
                .show(ctx, |ui| self.settings_nav(ui));
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin::symmetric(40.0, 24.0)))
                .show(ctx, |ui| self.settings_page_ui(ui));
            return;
        }
        if !self.focus_mode {
            egui::SidePanel::right("chat").resizable(true)
                .default_width(if narrow { 300.0 } else { 360.0 })
                .width_range(270.0..=540.0)
                .frame(egui::Frame::none().fill(CHAT))
                .show(ctx, |ui| self.chat_panel(ui));
        }
        egui::CentralPanel::default().frame(egui::Frame::none().fill(STAGE))
            .show(ctx, |ui| self.stage(ui));
        if self.share_picker_open {
            self.share_picker(ctx);
        }
    }

}
