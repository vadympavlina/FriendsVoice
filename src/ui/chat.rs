//! Text chat panel.
use super::*;

/// A piece of a chat message: plain text or a web link.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Piece<'a> {
    Text(&'a str),
    Link(&'a str),
}

/// Splits a message into text and `http(s)` links. Punctuation that ends a sentence
/// right after a link is not part of the link.
pub(super) fn split_links(message: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut index = 0;
    while index < message.len() {
        let rest = &message[index..];
        let at_word_start = index == 0 || message[..index].ends_with(|c: char| c.is_whitespace() || "([«\"'".contains(c));
        if at_word_start && (rest.starts_with("http://") || rest.starts_with("https://")) {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '»', '"', '\'']);
            // "https://" alone is not a link.
            if url.len() > "https://".len() + 1 {
                if text_start < index { pieces.push(Piece::Text(&message[text_start..index])); }
                pieces.push(Piece::Link(url));
                index += url.len();
                text_start = index;
                continue;
            }
        }
        index += rest.chars().next().map_or(1, char::len_utf8);
    }
    if text_start < message.len() { pieces.push(Piece::Text(&message[text_start..])); }
    pieces
}

/// A message body: wrapped text, with links that open in the browser.
fn message_body(ui: &mut Ui, body: &str) {
    let pieces = split_links(body);
    if pieces.iter().all(|p| matches!(p, Piece::Text(_))) {
        ui.add(egui::Label::new(text(body, 14.0, TEXT)).wrap());
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for piece in pieces {
            match piece {
                Piece::Text(part) => { ui.label(text(part, 14.0, TEXT)); }
                Piece::Link(url) => {
                    let response = ui.link(RichText::new(url).size(14.0).color(ACCENT)).on_hover_text(url);
                    if response.clicked() { ui.ctx().open_url(egui::OpenUrl::new_tab(url)); }
                }
            }
        }
    });
}

impl FriendsApp {
    pub(super) fn chat_panel(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::top("chat_top").exact_height(52.0)
            .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin::symmetric(16.0, 0.0)))
            .show_inside(ui, |ui| {
                let bar = ui.max_rect();
                ui.painter().hline(bar.x_range(), bar.bottom(), Stroke::new(1.0_f32, LINE));
                ui.horizontal_centered(|ui| {
                    ui.label(text(ph::HASH, 19.0, TEXT3));
                    ui.label(strong(tr("Chat"), 15.5, TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, ph::MAGNIFYING_GLASS, tr("Search chat · Ctrl+F"),
                            self.chat_search_open, 34.0) {
                            self.chat_search_open = !self.chat_search_open;
                            self.chat_search_focus = self.chat_search_open;
                            if !self.chat_search_open { self.chat_search.clear(); }
                        }
                    });
                });
            });

        egui::TopBottomPanel::bottom("composer")
            .frame(egui::Frame::none().fill(CHAT).inner_margin(Margin { left: 16.0, right: 16.0, top: 6.0, bottom: 16.0 }))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let online = self.mesh.is_some();
                    let field = ui.add_enabled(online, egui::TextEdit::singleline(&mut self.draft)
                        .hint_text(if online { tr("Message your friends") } else { tr("No connection") })
                        .margin(Margin::symmetric(12.0, 10.0))
                        .desired_width(ui.available_width() - 80.0));
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (field.has_focus() || field.lost_focus()) && enter {
                        self.send_chat();
                        field.request_focus();
                    }
                    let ready = online && !self.draft.trim().is_empty();
                    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(38.0),
                        if ready { Sense::click() } else { Sense::hover() });
                    let resp = resp.on_hover_text(tr("Send · Enter"));
                    ui.painter().rect_filled(rect, Rounding::same(8.0),
                        if ready { if resp.hovered() { ACCENT.lerp_to_gamma(Color32::WHITE, 0.12) } else { ACCENT } } else { RAISED });
                    ui.painter().text(rect.center(), Align2::CENTER_CENTER, ph::PAPER_PLANE_RIGHT,
                        FontId::proportional(19.0), if ready { ACCENT_INK } else { TEXT3 });
                    if ready && resp.clicked() { self.send_chat(); }
                });
                if self.draft.chars().count() > 900 {
                    ui.label(text(format!("{}/1000", self.draft.chars().count()), 11.5, TEXT3));
                }
            });

        if self.chat_search_open {
            egui::TopBottomPanel::top("chat_search").frame(egui::Frame::none().fill(CHAT)
                .inner_margin(Margin { left: 16.0, right: 16.0, top: 10.0, bottom: 4.0 }))
                .show_inside(ui, |ui| {
                    let field = ui.add(egui::TextEdit::singleline(&mut self.chat_search)
                        .hint_text(tr("Search by text or name"))
                        .margin(Margin::symmetric(12.0, 8.0))
                        .desired_width(f32::INFINITY));
                    if self.chat_search_focus {
                        field.request_focus();
                        self.chat_search_focus = false;
                    }
                });
        }

        let search = self.chat_search.trim().to_lowercase();
        let nickname = self.settings.nickname.clone();
        egui::CentralPanel::default().frame(egui::Frame::none().fill(CHAT)).show_inside(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("chat_feed").auto_shrink([false, false])
                .stick_to_bottom(search.is_empty())
                .show(ui, |ui| {
                    egui::Frame::none().inner_margin(Margin::symmetric(16.0, 12.0)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let mut previous: Option<&str> = None;
                        let mut shown = 0;
                        for message in &self.messages {
                            if !search.is_empty()
                                && !message.text.to_lowercase().contains(&search)
                                && !message.name.to_lowercase().contains(&search) {
                                continue;
                            }
                            shown += 1;
                            if message.system {
                                previous = None;
                                ui.add_space(4.0);
                                let body = i18n::tr_stored(&message.text);
                                let note = if message.repeat > 1 {
                                    format!("{body}  ×{}", message.repeat)
                                } else { body };
                                ui.add(egui::Label::new(text(note, 12.5, TEXT3)).wrap());
                                ui.add_space(4.0);
                                continue;
                            }
                            let first = previous != Some(message.name.as_str());
                            ui.add_space(if first { 12.0 } else { 1.0 });
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                if first {
                                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                    paint_avatar(ui.painter(), rect.center(), 34.0, &message.name);
                                } else {
                                    ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                                }
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    if first {
                                        let mine = message.name == nickname;
                                        ui.horizontal(|ui| {
                                            ui.label(strong(&message.name, 14.0, name_color(&message.name)));
                                            if mine { ui.label(text(tr("you"), 11.5, TEXT3)); }
                                            ui.label(text(&message.time, 11.5, TEXT3));
                                        });
                                    }
                                    message_body(ui, &message.text);
                                });
                            });
                            previous = Some(message.name.as_str());
                        }
                        if shown == 0 && !search.is_empty() {
                            ui.add_space(24.0);
                            ui.vertical_centered(|ui| {
                                ui.label(strong(tr("Nothing found"), 14.0, TEXT));
                                ui.label(text(tr("Try another word or name."), 12.5, TEXT3));
                            });
                        }
                    });
                });
        });
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use Piece::*;

    #[test]
    fn plain_text_has_no_links() {
        assert_eq!(split_links("hello world"), vec![Text("hello world")]);
        assert_eq!(split_links(""), Vec::<Piece>::new());
        assert_eq!(split_links("see http:// or https://"), vec![Text("see http:// or https://")]);
    }

    #[test]
    fn links_are_found_with_text_around_them() {
        assert_eq!(split_links("look https://example.com/a?b=1 now"),
            vec![Text("look "), Link("https://example.com/a?b=1"), Text(" now")]);
        assert_eq!(split_links("http://a.io"), vec![Link("http://a.io")]);
        assert_eq!(split_links("two http://a.io and https://b.io/x"),
            vec![Text("two "), Link("http://a.io"), Text(" and "), Link("https://b.io/x")]);
    }

    #[test]
    fn punctuation_after_a_link_is_not_part_of_it() {
        assert_eq!(split_links("go to https://example.com."), vec![Text("go to "), Link("https://example.com"), Text(".")]);
        assert_eq!(split_links("(https://example.com/path)"), vec![Text("("), Link("https://example.com/path"), Text(")")]);
        assert_eq!(split_links("ok? https://x.org/a,b!"), vec![Text("ok? "), Link("https://x.org/a,b"), Text("!")]);
    }

    #[test]
    fn only_web_links_and_only_at_word_starts_are_links() {
        assert_eq!(split_links("ftp://example.com file:///etc/passwd javascript:alert(1)").len(), 1);
        assert_eq!(split_links("xhttps://example.com"), vec![Text("xhttps://example.com")]);
    }

    #[test]
    fn non_ascii_text_is_handled_without_panicking() {
        assert_eq!(split_links("привіт https://приклад.укр/сторінка гарно"),
            vec![Text("привіт "), Link("https://приклад.укр/сторінка"), Text(" гарно")]);
    }
}
