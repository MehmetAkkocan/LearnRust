//! Arayüz dili: Türkçe / İngilizce metinler ve dil seçimi (`lang` çerezi).

use axum::extract::FromRequestParts;
use axum::http::{header::COOKIE, request::Parts, HeaderMap};

use crate::runner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Tr,
    En,
}

impl Lang {
    pub fn from_code(s: &str) -> Option<Lang> {
        match s {
            "tr" => Some(Lang::Tr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        self.ui().code
    }

    pub fn ui(self) -> &'static Ui {
        match self {
            Lang::Tr => &TR,
            Lang::En => &EN,
        }
    }

    /// `Cookie` başlıklarındaki `lang=..` değerini okur; yoksa/bilinmiyorsa Türkçe.
    pub fn from_headers(h: &HeaderMap) -> Lang {
        h.get_all(COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(';'))
            .filter_map(|kv| kv.trim().strip_prefix("lang="))
            .find_map(Lang::from_code)
            .unwrap_or_default()
    }

    /// "120 / 340 puan · %35" | "120 / 340 points · 35%"
    pub fn meta(self, points: u32, max: u32, percent: u32) -> String {
        match self {
            Lang::Tr => format!("{points} / {max} puan · %{percent}"),
            Lang::En => format!("{points} / {max} points · {percent}%"),
        }
    }

    pub fn unlock_info(self, threshold: u32, need: u32) -> String {
        match self {
            Lang::Tr => format!(
                "Açılması için önceki seviyede {threshold} puan gerekli · {need} puan daha topla"
            ),
            Lang::En => format!(
                "Needs {threshold} points in the previous level · {need} more to go"
            ),
        }
    }

    pub fn quiz_label(self, mcq: usize, coding: usize) -> String {
        match self {
            Lang::Tr => format!("Seviye testi ({mcq} çoktan seçmeli, {coding} kodlama)"),
            Lang::En => format!("Level quiz ({mcq} multiple choice, {coding} coding)"),
        }
    }

    pub fn quiz_title(self, level_title: &str) -> String {
        match self {
            Lang::Tr => format!("{level_title} testi"),
            Lang::En => format!("{level_title} quiz"),
        }
    }

    pub fn read_ok(self, gained: u32) -> String {
        match self {
            Lang::Tr => format!("Ders okundu: +{gained} puan!"),
            Lang::En => format!("Lesson read: +{gained} points!"),
        }
    }

    pub fn gained(self, n: u32) -> String {
        match self {
            Lang::Tr => format!("+{n} puan kazandın!"),
            Lang::En => format!("You earned +{n} points!"),
        }
    }

    pub fn locked_level(self, title: &str, need: u32) -> String {
        match self {
            Lang::Tr => format!(
                "\"{title}\" seviyesi henüz kilitli. Açmak için {need} puan daha topla."
            ),
            Lang::En => format!(
                "The \"{title}\" level is still locked. Earn {need} more points to open it."
            ),
        }
    }

    /// Rozet adı.
    pub fn badge_label(self, id: &str) -> String {
        if self == Lang::Tr {
            return crate::game::badge_info(id).0;
        }
        match id {
            "ilk-adim" => return "First Step".into(),
            "ilk-egzersiz" => return "First Exercise".into(),
            "quiz-ustasi" => return "Quiz Master".into(),
            "yari-yol" => return "Halfway There".into(),
            "senior" => return "Senior".into(),
            _ => {}
        }
        if let Some(n) = id.strip_prefix("seviye-") {
            if let Some(n) = n.strip_suffix("-tamam") {
                return format!("Level {n} Complete");
            }
            if let Some(n) = n.strip_suffix("-acildi") {
                return format!("Level {n} Unlocked");
            }
        }
        id.to_string()
    }

    /// Çalıştırıcının kendi eklediği (Türkçe) notları arayüz diline çevirir;
    /// derleyici ve test çıktısına dokunmaz.
    pub fn runner_output(self, s: String) -> String {
        if self == Lang::Tr {
            return s;
        }
        s.replace(runner::MSG_TRUNCATED, "... [output cut at 8 KB]")
            .replace(runner::MSG_COMPILE_TIMEOUT, "compile timed out (30 s)")
            .replace(runner::MSG_RUN_TIMEOUT, "run timed out (10 s)")
            .replace(runner::MSG_FLOOD, "output limit exceeded, program stopped")
    }
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Lang::from_headers(&parts.headers))
    }
}

/// Şablonlardaki sabit arayüz metinleri.
pub struct Ui {
    pub code: &'static str,
    pub lang_name: &'static str,
    pub lang_menu: &'static str,
    pub brand: &'static str,
    pub nav_home: &'static str,
    pub points_label: &'static str,
    pub points_word: &'static str,
    // panel
    pub panel_title: &'static str,
    pub panel_heading: &'static str,
    pub total_points: &'static str,
    pub badges: &'static str,
    pub no_badges: &'static str,
    pub levels: &'static str,
    pub level: &'static str,
    pub status_locked: &'static str,
    pub status_open: &'static str,
    pub status_completed: &'static str,
    // seviye
    pub lessons: &'static str,
    pub mark_read: &'static str,
    pub mark_unread: &'static str,
    pub mark_exercise_done: &'static str,
    pub mark_exercise: &'static str,
    // ders
    pub example_code: &'static str,
    pub already_read: &'static str,
    pub read_btn: &'static str,
    pub exercise: &'static str,
    pub done: &'static str,
    pub run_btn: &'static str,
    pub running: &'static str,
    pub next_lesson: &'static str,
    pub back_to_level: &'static str,
    pub level_quiz: &'static str,
    // test
    pub quiz_heading: &'static str,
    pub mcq_heading: &'static str,
    pub submit_answers: &'static str,
    pub coding_heading: &'static str,
    pub question: &'static str,
    pub correct: &'static str,
    pub wrong_answer_is: &'static str,
    pub nothing_selected: &'static str,
    pub total: &'static str,
    pub unlocked: &'static str,
    // sonuç / hata
    pub read_again: &'static str,
    pub all_passed: &'static str,
    pub tests_failed: &'static str,
    pub compile_error: &'static str,
    pub timeout: &'static str,
    pub run_error: &'static str,
    pub busy: &'static str,
    pub no_extra_points: &'static str,
    pub level_locked: &'static str,
    pub unknown_content: &'static str,
    pub unknown_lesson: &'static str,
    pub unknown_level: &'static str,
    pub unknown_question: &'static str,
    pub not_found: &'static str,
    pub not_found_msg: &'static str,
    pub locked: &'static str,
    pub server_error: &'static str,
    pub server_error_msg: &'static str,
    pub back_home: &'static str,
}

pub static TR: Ui = Ui {
    code: "tr",
    lang_name: "Türkçe",
    lang_menu: "Dil seç",
    brand: "Rust Öğren",
    nav_home: "Panel",
    points_label: "Puan",
    points_word: "puan",
    panel_title: "Panel",
    panel_heading: "Rust Öğrenme Yolculuğu",
    total_points: "Toplam puan",
    badges: "Rozetler",
    no_badges: "Henüz rozetin yok. İlk dersi okuyarak başla!",
    levels: "Seviyeler",
    level: "Seviye",
    status_locked: "Kilitli",
    status_open: "Açık",
    status_completed: "Tamamlandı",
    lessons: "Dersler",
    mark_read: "okundu",
    mark_unread: "okunmadı",
    mark_exercise_done: "egzersiz tamam",
    mark_exercise: "egzersiz bekliyor",
    example_code: "Örnek kod",
    already_read: "Bu dersi okudun.",
    read_btn: "Okudum",
    exercise: "Egzersiz",
    done: "Tamam",
    run_btn: "Çalıştır",
    running: "Derleniyor ve çalıştırılıyor...",
    next_lesson: "Sonraki ders",
    back_to_level: "Seviyeye dön",
    level_quiz: "Seviye testi",
    quiz_heading: "Seviye Testi",
    mcq_heading: "Çoktan seçmeli sorular",
    submit_answers: "Cevapları gönder",
    coding_heading: "Kodlama soruları",
    question: "Soru",
    correct: "Doğru!",
    wrong_answer_is: "Yanlış. Doğru cevap:",
    nothing_selected: "Hiçbir soru işaretlenmedi.",
    total: "Toplam",
    unlocked: "Yeni seviye açıldı:",
    read_again: "Bu dersi zaten okumuştun.",
    all_passed: "Tüm testler geçti!",
    tests_failed: "Bazı testler geçmedi.",
    compile_error: "Kod derlenmedi",
    timeout: "Süre doldu: program çok uzun sürdü (sonsuz döngü olabilir).",
    run_error: "Kod çalıştırılamadı",
    busy: "Sunucu şu an meşgul, biraz sonra tekrar dene.",
    no_extra_points: "Bu görevi daha önce tamamlamıştın; yeniden puan verilmez.",
    level_locked: "Bu seviye henüz kilitli.",
    unknown_content: "Böyle bir içerik yok.",
    unknown_lesson: "Böyle bir ders yok.",
    unknown_level: "Böyle bir seviye yok.",
    unknown_question: "Böyle bir soru yok.",
    not_found: "Sayfa bulunamadı",
    not_found_msg: "Aradığın sayfa yok.",
    locked: "Kilitli",
    server_error: "Sunucu hatası",
    server_error_msg: "Bir şeyler ters gitti. Lütfen tekrar dene.",
    back_home: "Panele dön",
};

pub static EN: Ui = Ui {
    code: "en",
    lang_name: "English",
    lang_menu: "Choose language",
    brand: "Learn Rust",
    nav_home: "Dashboard",
    points_label: "Points",
    points_word: "points",
    panel_title: "Dashboard",
    panel_heading: "Your Rust Learning Path",
    total_points: "Total points",
    badges: "Badges",
    no_badges: "No badges yet. Read your first lesson to get started!",
    levels: "Levels",
    level: "Level",
    status_locked: "Locked",
    status_open: "Open",
    status_completed: "Completed",
    lessons: "Lessons",
    mark_read: "read",
    mark_unread: "not read",
    mark_exercise_done: "exercise done",
    mark_exercise: "exercise to do",
    example_code: "Example code",
    already_read: "You have read this lesson.",
    read_btn: "Mark as read",
    exercise: "Exercise",
    done: "Done",
    run_btn: "Run",
    running: "Compiling and running...",
    next_lesson: "Next lesson",
    back_to_level: "Back to level",
    level_quiz: "Level quiz",
    quiz_heading: "Level Quiz",
    mcq_heading: "Multiple-choice questions",
    submit_answers: "Submit answers",
    coding_heading: "Coding questions",
    question: "Question",
    correct: "Correct!",
    wrong_answer_is: "Wrong. The correct answer:",
    nothing_selected: "No question was answered.",
    total: "Total",
    unlocked: "New level unlocked:",
    read_again: "You had already read this lesson.",
    all_passed: "All tests passed!",
    tests_failed: "Some tests failed.",
    compile_error: "The code did not compile",
    timeout: "Time's up: the program ran too long (maybe an infinite loop).",
    run_error: "The code could not be run",
    busy: "The server is busy right now, try again in a moment.",
    no_extra_points: "You had already completed this task; no points are given twice.",
    level_locked: "This level is still locked.",
    unknown_content: "There is no such content.",
    unknown_lesson: "There is no such lesson.",
    unknown_level: "There is no such level.",
    unknown_question: "There is no such question.",
    not_found: "Page not found",
    not_found_msg: "The page you are looking for does not exist.",
    locked: "Locked",
    server_error: "Server error",
    server_error_msg: "Something went wrong. Please try again.",
    back_home: "Back to dashboard",
};

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn lang_of(cookie: &str) -> Lang {
        let mut h = HeaderMap::new();
        h.insert(COOKIE, HeaderValue::from_str(cookie).unwrap());
        Lang::from_headers(&h)
    }

    #[test]
    fn cerezden_dil() {
        assert_eq!(Lang::from_headers(&HeaderMap::new()), Lang::Tr);
        assert_eq!(lang_of("lang=en"), Lang::En);
        assert_eq!(lang_of("a=b; lang=en; c=d"), Lang::En);
        assert_eq!(lang_of("lang=tr"), Lang::Tr);
        assert_eq!(lang_of("lang=xx"), Lang::Tr);
        assert_eq!(lang_of("xlang=en"), Lang::Tr);
    }

    #[test]
    fn rozet_adlari() {
        assert_eq!(Lang::Tr.badge_label("ilk-adim"), "İlk Adım");
        assert_eq!(Lang::En.badge_label("ilk-adim"), "First Step");
        assert_eq!(Lang::En.badge_label("seviye-3-acildi"), "Level 3 Unlocked");
        assert_eq!(Lang::En.badge_label("seviye-7-tamam"), "Level 7 Complete");
    }

    #[test]
    fn calistirici_notlari_cevrilir() {
        let s = format!("x\n{}", runner::MSG_RUN_TIMEOUT);
        assert_eq!(Lang::Tr.runner_output(s.clone()), s);
        assert!(Lang::En.runner_output(s).contains("timed out"));
    }
}
