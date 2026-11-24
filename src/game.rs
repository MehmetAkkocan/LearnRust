//! Oyunlaştırma mantığı (saf): puan, kilit, ödül, rozet. Catalog + Progress alır.

use crate::content::{Catalog, Level};
use crate::store::Progress;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LevelStatus {
    Locked,
    Open,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AwardError {
    /// Katalogda böyle bir ders/quiz yok.
    Unknown(String),
    /// Seviye kilitli (seviye id).
    Locked(u32),
}

impl std::fmt::Display for AwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AwardError::Unknown(id) => write!(f, "bilinmeyen öğe: {id}"),
            AwardError::Locked(l) => write!(f, "seviye {l} kilitli"),
        }
    }
}
impl std::error::Error for AwardError {}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AwardResult {
    pub gained: u32,
    pub new_badges: Vec<String>,
    pub xp: u32,
    /// Yalnız MCQ ödüllerinde anlamlı: cevap doğru muydu.
    pub correct: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LevelSummary {
    pub id: u32,
    pub title: String,
    pub slug: String,
    pub points: u32,
    pub max: u32,
    pub percent: u32,
    pub status: LevelStatus,
    pub threshold: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub total_points: u32,
    pub total_max: u32,
    pub xp: u32,
    pub levels: Vec<LevelSummary>,
    pub badges: Vec<String>,
}

/// Seviyede kazanılmış puan (okuma + egzersiz + doğru mcq + coding).
pub fn level_points(catalog: &Catalog, p: &Progress, level_id: u32) -> u32 {
    let Some(lv) = catalog.level_by_id(level_id) else {
        return 0;
    };
    level_points_of(lv, p)
}

fn level_points_of(lv: &Level, p: &Progress) -> u32 {
    let mut s = 0;
    for l in &lv.lessons {
        if p.reads.contains(&l.id) {
            s += l.read_points;
        }
        if p.exercises.contains(&l.id) {
            s += l.exercise.points;
        }
    }
    for m in &lv.quiz.mcq {
        if p.mcq.get(&m.id).copied().unwrap_or(false) {
            s += m.points;
        }
    }
    for c in &lv.quiz.coding {
        if p.coding.contains(&c.id) {
            s += c.points;
        }
    }
    s
}

/// Oynanabilir (id > 0) tüm seviyelerin toplam kazanılan puanı.
pub fn total_points(catalog: &Catalog, p: &Progress) -> u32 {
    catalog.playable().map(|l| level_points_of(l, p)).sum()
}

pub fn total_max(catalog: &Catalog) -> u32 {
    catalog.playable().map(|l| l.max_points()).sum()
}

/// Level <= 1 her zaman açık; N için önceki seviyenin puanı >= eşik.
pub fn is_unlocked(catalog: &Catalog, p: &Progress, level_id: u32) -> bool {
    if level_id <= 1 {
        return true;
    }
    level_points(catalog, p, level_id - 1) >= catalog.effective_threshold(level_id)
}

/// 0..=100 (max 0 ise 0).
pub fn level_percent(catalog: &Catalog, p: &Progress, level_id: u32) -> u32 {
    let Some(lv) = catalog.level_by_id(level_id) else {
        return 0;
    };
    let max = lv.max_points();
    if max == 0 {
        return 0;
    }
    (level_points_of(lv, p) * 100 / max).min(100)
}

/// Tüm ders okuma + egzersiz, tüm mcq (cevaplanmış) ve tüm coding işaretli mi?
/// Boş seviye tamamlanmış sayılmaz.
fn is_complete(lv: &Level, p: &Progress) -> bool {
    let items = lv.lessons.len() + lv.quiz.mcq.len() + lv.quiz.coding.len();
    items > 0
        && lv
            .lessons
            .iter()
            .all(|l| p.reads.contains(&l.id) && p.exercises.contains(&l.id))
        && lv.quiz.mcq.iter().all(|m| p.mcq.contains_key(&m.id))
        && lv.quiz.coding.iter().all(|c| p.coding.contains(&c.id))
}

pub fn level_status(catalog: &Catalog, p: &Progress, level_id: u32) -> LevelStatus {
    if !is_unlocked(catalog, p, level_id) {
        return LevelStatus::Locked;
    }
    match catalog.level_by_id(level_id) {
        Some(lv) if is_complete(lv, p) => LevelStatus::Completed,
        _ => LevelStatus::Open,
    }
}

/// Rozet kurallarını değerlendirir, yeni kazanılanları ekler ve döner.
fn refresh_badges(catalog: &Catalog, p: &mut Progress) -> Vec<String> {
    let mut want: Vec<String> = Vec::new();
    if !p.reads.is_empty() {
        want.push("ilk-adim".into());
    }
    if !p.exercises.is_empty() {
        want.push("ilk-egzersiz".into());
    }
    for lv in catalog.playable() {
        let id = lv.id;
        if id >= 2 && is_unlocked(catalog, p, id) {
            want.push(format!("seviye-{id}-acildi"));
        }
        if is_complete(lv, p) {
            want.push(format!("seviye-{id}-tamam"));
            if id == 7 {
                want.push("senior".into());
            }
        }
        if !lv.quiz.mcq.is_empty()
            && lv
                .quiz
                .mcq
                .iter()
                .all(|m| p.mcq.get(&m.id).copied().unwrap_or(false))
        {
            want.push("quiz-ustasi".into());
        }
    }
    let max = total_max(catalog);
    if max > 0 && total_points(catalog, p) * 2 >= max {
        want.push("yari-yol".into());
    }
    let mut new = Vec::new();
    for b in want {
        if p.badges.insert(b.clone()) {
            new.push(b);
        }
    }
    new
}

fn finish(catalog: &Catalog, p: &mut Progress, gained: u32) -> AwardResult {
    p.xp = p.xp.saturating_add(gained);
    let new_badges = refresh_badges(catalog, p);
    AwardResult {
        gained,
        new_badges,
        xp: p.xp,
        correct: false,
    }
}

fn level_of_lesson<'a>(catalog: &'a Catalog, id: &str) -> Result<&'a Level, AwardError> {
    catalog
        .levels
        .iter()
        .find(|l| l.id > 0 && l.lesson_by_id(id).is_some())
        .ok_or_else(|| AwardError::Unknown(id.to_string()))
}

fn check_unlocked(catalog: &Catalog, p: &Progress, lv: &Level) -> Result<(), AwardError> {
    if is_unlocked(catalog, p, lv.id) {
        Ok(())
    } else {
        Err(AwardError::Locked(lv.id))
    }
}

pub fn award_read(
    catalog: &Catalog,
    p: &mut Progress,
    lesson_id: &str,
) -> Result<AwardResult, AwardError> {
    let lv = level_of_lesson(catalog, lesson_id)?;
    check_unlocked(catalog, p, lv)?;
    let pts = lv.lesson_by_id(lesson_id).map(|l| l.read_points).unwrap_or(0);
    let gained = if p.reads.insert(lesson_id.to_string()) { pts } else { 0 };
    Ok(finish(catalog, p, gained))
}

pub fn award_exercise(
    catalog: &Catalog,
    p: &mut Progress,
    lesson_id: &str,
) -> Result<AwardResult, AwardError> {
    let lv = level_of_lesson(catalog, lesson_id)?;
    check_unlocked(catalog, p, lv)?;
    let pts = lv
        .lesson_by_id(lesson_id)
        .map(|l| l.exercise.points)
        .unwrap_or(0);
    let gained = if p.exercises.insert(lesson_id.to_string()) { pts } else { 0 };
    Ok(finish(catalog, p, gained))
}

/// Yanlış cevap kaydedilir (puan yok); yanlıştan doğruya geçiş puan verir;
/// zaten doğru olan soru tekrar puan vermez ve doğru kaydı yanlışa çevrilmez.
pub(crate) fn award_mcq(
    catalog: &Catalog,
    p: &mut Progress,
    mcq_id: &str,
    correct: bool,
) -> Result<AwardResult, AwardError> {
    let (lv, pts) = catalog
        .levels
        .iter()
        .filter(|l| l.id > 0)
        .find_map(|l| l.quiz.mcq.iter().find(|m| m.id == mcq_id).map(|m| (l, m.points)))
        .ok_or_else(|| AwardError::Unknown(mcq_id.to_string()))?;
    check_unlocked(catalog, p, lv)?;
    let already_correct = p.mcq.get(mcq_id).copied().unwrap_or(false);
    let mut gained = 0;
    if !already_correct {
        p.mcq.insert(mcq_id.to_string(), correct);
        if correct {
            gained = pts;
        }
    }
    let mut r = finish(catalog, p, gained);
    r.correct = correct;
    Ok(r)
}

/// Seçilen şıkkı catalog'daki `answer` ile karşılaştırır, sonucu `correct`
/// alanında döner ve `award_mcq` kurallarını uygular.
pub fn award_mcq_choice(
    catalog: &Catalog,
    p: &mut Progress,
    mcq_id: &str,
    choice: usize,
) -> Result<AwardResult, AwardError> {
    let answer = catalog
        .levels
        .iter()
        .filter(|l| l.id > 0)
        .find_map(|l| l.quiz.mcq.iter().find(|m| m.id == mcq_id).map(|m| m.answer))
        .ok_or_else(|| AwardError::Unknown(mcq_id.to_string()))?;
    award_mcq(catalog, p, mcq_id, choice == answer)
}

/// Rozet için (başlık, açıklama); bilinmeyen id için (id, "").
pub fn badge_info(id: &str) -> (String, String) {
    let t = |a: &str, b: &str| (a.to_string(), b.to_string());
    match id {
        "ilk-adim" => return t("İlk Adım", "İlk dersini okudun."),
        "ilk-egzersiz" => return t("İlk Egzersiz", "İlk egzersizini çözdün."),
        "quiz-ustasi" => {
            return t("Quiz Ustası", "Bir seviyenin tüm sorularını doğru yanıtladın.")
        }
        "yari-yol" => return t("Yarı Yol", "Toplam puanın yarısına ulaştın."),
        "senior" => return t("Senior", "Senior seviyesini tamamladın."),
        _ => {}
    }
    if let Some(n) = id.strip_prefix("seviye-") {
        if let Some(n) = n.strip_suffix("-tamam") {
            return (
                format!("Seviye {n} Tamam"),
                format!("Seviye {n} tamamlandı."),
            );
        }
        if let Some(n) = n.strip_suffix("-acildi") {
            return (
                format!("Seviye {n} Açıldı"),
                format!("Seviye {n} kilidi açıldı."),
            );
        }
    }
    (id.to_string(), String::new())
}

pub fn award_coding(
    catalog: &Catalog,
    p: &mut Progress,
    coding_id: &str,
) -> Result<AwardResult, AwardError> {
    let (lv, pts) = catalog
        .levels
        .iter()
        .filter(|l| l.id > 0)
        .find_map(|l| l.quiz.coding.iter().find(|c| c.id == coding_id).map(|c| (l, c.points)))
        .ok_or_else(|| AwardError::Unknown(coding_id.to_string()))?;
    check_unlocked(catalog, p, lv)?;
    let gained = if p.coding.insert(coding_id.to_string()) { pts } else { 0 };
    Ok(finish(catalog, p, gained))
}

/// Web katmanı için özet (yalnız oynanabilir seviyeler).
pub fn summary(catalog: &Catalog, p: &Progress) -> Summary {
    let levels = catalog
        .playable()
        .map(|lv| LevelSummary {
            id: lv.id,
            title: lv.title.clone(),
            slug: lv.slug.clone(),
            points: level_points_of(lv, p),
            max: lv.max_points(),
            percent: level_percent(catalog, p, lv.id),
            status: level_status(catalog, p, lv.id),
            threshold: catalog.effective_threshold(lv.id),
        })
        .collect();
    Summary {
        total_points: total_points(catalog, p),
        total_max: total_max(catalog),
        xp: p.xp,
        levels,
        badges: p.badges.iter().cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{CodingQuiz, Exercise, Lesson, Mcq, Quiz};

    fn lesson(id: &str) -> Lesson {
        Lesson {
            id: id.into(),
            title: "t".into(),
            read_points: 10,
            body_md: String::new(),
            example: String::new(),
            exercise: Exercise {
                prompt: String::new(),
                starter: String::new(),
                tests: String::new(),
                solution: String::new(),
                points: 30,
            },
        }
    }

    /// Seviye: 1 ders (40) + 2 mcq (20) + 1 coding (50) = 110.
    fn level(id: u32) -> Level {
        Level {
            id,
            slug: format!("s{id}"),
            title: format!("L{id}"),
            description: String::new(),
            unlock_threshold: None,
            quiz: Quiz {
                mcq: (1..=2)
                    .map(|k| Mcq {
                        id: format!("{id}-m{k}"),
                        q: String::new(),
                        choices: vec![],
                        answer: 0,
                        points: 10,
                    })
                    .collect(),
                coding: vec![CodingQuiz {
                    id: format!("{id}-q1"),
                    prompt: String::new(),
                    starter: String::new(),
                    tests: String::new(),
                    solution: String::new(),
                    points: 50,
                }],
            },
            lessons: vec![lesson(&format!("{id}-01"))],
        }
    }

    fn catalog() -> Catalog {
        // level 0 fixture + 1, 2 (eşik 80: 110*0.7=77 -> 80)
        Catalog {
            levels: vec![level(0), level(1), level(2)],
            ..Default::default()
        }
    }

    #[test]
    fn kilit_mantigi() {
        let c = catalog();
        let mut p = Progress::default();
        assert_eq!(c.effective_threshold(2), 80);
        assert!(is_unlocked(&c, &p, 1));
        assert!(!is_unlocked(&c, &p, 2));
        assert_eq!(level_status(&c, &p, 2), LevelStatus::Locked);
        award_read(&c, &mut p, "1-01").unwrap(); // 10
        award_exercise(&c, &mut p, "1-01").unwrap(); // 40
        award_mcq(&c, &mut p, "1-m1", true).unwrap(); // 50
        award_mcq(&c, &mut p, "1-m2", true).unwrap(); // 60
        assert!(!is_unlocked(&c, &p, 2)); // eşik altı
        let r = award_coding(&c, &mut p, "1-q1").unwrap(); // 110
        assert!(is_unlocked(&c, &p, 2)); // eşik üstü
        assert!(r.new_badges.contains(&"seviye-2-acildi".to_string()));
        assert!(r.new_badges.contains(&"seviye-1-tamam".to_string()));
        assert_eq!(level_percent(&c, &p, 1), 100);
        assert_eq!(level_status(&c, &p, 1), LevelStatus::Completed);
        assert_eq!(level_status(&c, &p, 2), LevelStatus::Open);
    }

    #[test]
    fn kilitli_seviyede_odul_reddedilir() {
        let c = catalog();
        let mut p = Progress::default();
        assert_eq!(award_read(&c, &mut p, "2-01"), Err(AwardError::Locked(2)));
        assert_eq!(award_coding(&c, &mut p, "2-q1"), Err(AwardError::Locked(2)));
        assert!(matches!(
            award_read(&c, &mut p, "9-99"),
            Err(AwardError::Unknown(_))
        ));
        assert_eq!(p, Progress::default());
    }

    #[test]
    fn idempotent_odul() {
        let c = catalog();
        let mut p = Progress::default();
        let a = award_read(&c, &mut p, "1-01").unwrap();
        assert_eq!(a.gained, 10);
        assert_eq!(a.xp, 10);
        let b = award_read(&c, &mut p, "1-01").unwrap();
        assert_eq!(b.gained, 0);
        assert_eq!(b.xp, 10);
        assert!(b.new_badges.is_empty());
        assert_eq!(award_exercise(&c, &mut p, "1-01").unwrap().gained, 30);
        assert_eq!(award_exercise(&c, &mut p, "1-01").unwrap().gained, 0);
        assert_eq!(award_coding(&c, &mut p, "1-q1").unwrap().gained, 50);
        assert_eq!(award_coding(&c, &mut p, "1-q1").unwrap().gained, 0);
        assert_eq!(p.xp, 90);
    }

    #[test]
    fn mcq_yanlistan_dogruya() {
        let c = catalog();
        let mut p = Progress::default();
        let r = award_mcq(&c, &mut p, "1-m1", false).unwrap();
        assert_eq!(r.gained, 0);
        assert_eq!(p.mcq.get("1-m1"), Some(&false));
        assert_eq!(level_points(&c, &p, 1), 0);
        let r = award_mcq(&c, &mut p, "1-m1", true).unwrap();
        assert_eq!(r.gained, 10);
        assert_eq!(award_mcq(&c, &mut p, "1-m1", true).unwrap().gained, 0);
        // dogru kaydi sonradan yanlisla bozulmaz
        assert_eq!(award_mcq(&c, &mut p, "1-m1", false).unwrap().gained, 0);
        assert_eq!(p.mcq.get("1-m1"), Some(&true));
        assert_eq!(p.xp, 10);
    }

    #[test]
    fn rozetler() {
        let c = catalog();
        let mut p = Progress::default();
        let r = award_read(&c, &mut p, "1-01").unwrap();
        assert_eq!(r.new_badges, vec!["ilk-adim".to_string()]);
        let r = award_exercise(&c, &mut p, "1-01").unwrap(); // 40 / 220
        assert_eq!(r.new_badges, vec!["ilk-egzersiz".to_string()]);
        award_mcq(&c, &mut p, "1-m1", true).unwrap();
        let r = award_mcq(&c, &mut p, "1-m2", true).unwrap(); // 60 / 220
        assert!(r.new_badges.contains(&"quiz-ustasi".to_string()));
        assert!(!p.badges.contains("yari-yol"));
        let r = award_coding(&c, &mut p, "1-q1").unwrap(); // 110 / 220 = %50
        assert!(r.new_badges.contains(&"yari-yol".to_string()));
        assert!(p.badges.contains("seviye-1-tamam"));
        assert!(!p.badges.contains("senior"));
    }

    #[test]
    fn kilitli_mcq_reddedilir_ve_choice() {
        let c = catalog();
        let mut p = Progress::default();
        assert_eq!(
            award_mcq_choice(&c, &mut p, "2-m1", 0),
            Err(AwardError::Locked(2))
        );
        assert_eq!(p, Progress::default());
        let r = award_mcq_choice(&c, &mut p, "1-m1", 1).unwrap(); // answer = 0
        assert!(!r.correct && r.gained == 0);
        let r = award_mcq_choice(&c, &mut p, "1-m1", 0).unwrap();
        assert!(r.correct && r.gained == 10);
        assert!(matches!(
            award_mcq_choice(&c, &mut p, "x", 0),
            Err(AwardError::Unknown(_))
        ));
    }

    #[test]
    fn rozet_bilgisi() {
        assert_eq!(badge_info("ilk-adim").0, "İlk Adım");
        assert_eq!(badge_info("seviye-3-acildi").0, "Seviye 3 Açıldı");
        assert_eq!(badge_info("seviye-3-tamam").0, "Seviye 3 Tamam");
        assert_eq!(badge_info("yok").0, "yok");
    }

    #[test]
    fn summary_alanlari() {
        let c = catalog();
        let mut p = Progress::default();
        award_read(&c, &mut p, "1-01").unwrap();
        let s = summary(&c, &p);
        assert_eq!(s.levels.len(), 2); // level 0 yok
        assert_eq!(s.total_points, 10);
        assert_eq!(s.total_max, 220);
        assert_eq!(s.levels[0].status, LevelStatus::Open);
        assert_eq!(s.levels[1].status, LevelStatus::Locked);
        assert_eq!(s.levels[1].threshold, 80);
        serde_json::to_string(&s).unwrap();
    }
}
