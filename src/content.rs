//! İçerik şeması ve yükleyici (TOML -> Catalog).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::i18n::Lang;

fn default_read_points() -> u32 {
    10
}
fn default_mcq_points() -> u32 {
    10
}
fn default_coding_points() -> u32 {
    50
}
fn default_exercise_points() -> u32 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mcq {
    /// Zorunlu, "<N>-m<K>"; ilerleme kaydı bu id ile anahtarlanır.
    pub id: String,
    pub q: String,
    pub choices: Vec<String>,
    pub answer: usize,
    #[serde(default = "default_mcq_points")]
    pub points: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodingQuiz {
    pub id: String,
    pub prompt: String,
    pub starter: String,
    pub tests: String,
    pub solution: String,
    #[serde(default = "default_coding_points")]
    pub points: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Quiz {
    #[serde(default)]
    pub mcq: Vec<Mcq>,
    #[serde(default)]
    pub coding: Vec<CodingQuiz>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Exercise {
    pub prompt: String,
    pub starter: String,
    pub tests: String,
    pub solution: String,
    #[serde(default = "default_exercise_points")]
    pub points: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    #[serde(default = "default_read_points")]
    pub read_points: u32,
    pub body_md: String,
    pub example: String,
    pub exercise: Exercise,
}

impl Lesson {
    /// Dersten kazanılabilecek toplam puan (okuma + egzersiz).
    pub fn max_points(&self) -> u32 {
        self.read_points + self.exercise.points
    }
    pub fn body_html(&self) -> String {
        render_md(&self.body_md)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    pub id: u32,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// Opsiyonel; verilmezse `Catalog::effective_threshold` hesaplar.
    #[serde(default)]
    pub unlock_threshold: Option<u32>,
    #[serde(default)]
    pub quiz: Quiz,
    #[serde(skip)]
    pub lessons: Vec<Lesson>,
}

impl Level {
    /// Okuma + egzersiz + quiz (mcq + coding) toplamı.
    pub fn max_points(&self) -> u32 {
        let lessons: u32 = self.lessons.iter().map(|l| l.max_points()).sum();
        let mcq: u32 = self.quiz.mcq.iter().map(|m| m.points).sum();
        let coding: u32 = self.quiz.coding.iter().map(|c| c.points).sum();
        lessons + mcq + coding
    }
    pub fn lesson_by_id(&self, id: &str) -> Option<&Lesson> {
        self.lessons.iter().find(|l| l.id == id)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// id'ye göre sıralı (id == 0 fixture dahil). Asıl kaynak (Türkçe);
    /// puan, kilit ve cevap denetimi hep bunun üzerinden yapılır.
    pub levels: Vec<Level>,
    /// `levels` ile aynı sırada İngilizce görünüm: id/puan/answer aynı,
    /// metinler (ve ezilmişse kod) `content/en/` üst katmanından gelir.
    /// Üst katmanı olmayan içerik Türkçe kalır. Boşsa `levels` kullanılır.
    pub en: Vec<Level>,
    /// İngilizce üst katmanı eksik olan içerikler (`--check-content` raporlar).
    pub missing_en: Vec<String>,
}

impl Catalog {
    /// Verilen dilde gösterilecek seviyeler.
    pub fn levels_in(&self, lang: Lang) -> &[Level] {
        match lang {
            Lang::En if !self.en.is_empty() => &self.en,
            _ => &self.levels,
        }
    }
    pub fn level_in(&self, lang: Lang, id: u32) -> Option<&Level> {
        self.levels_in(lang).iter().find(|l| l.id == id)
    }
    /// Oynanabilir seviyeler (id > 0).
    pub fn playable(&self) -> impl Iterator<Item = &Level> {
        self.levels.iter().filter(|l| l.id > 0)
    }
    pub fn level_by_id(&self, id: u32) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }
    pub fn level_by_slug(&self, slug: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.slug == slug)
    }
    pub fn lesson_by_id(&self, id: &str) -> Option<&Lesson> {
        self.levels.iter().find_map(|l| l.lesson_by_id(id))
    }
    /// Seviyeyi açmak için gereken puan: açıkça verilmişse o, yoksa önceki
    /// seviyenin max_points'inin %70'i (10'a yuvarlanmış); önceki yoksa 0.
    /// Level 0 oynanamayan fixture olduğundan Level 1'in önceli sayılmaz:
    /// level_id <= 1 (açık eşik yoksa) her zaman 0 döner.
    pub fn effective_threshold(&self, level_id: u32) -> u32 {
        let Some(level) = self.level_by_id(level_id) else {
            return 0;
        };
        if let Some(t) = level.unlock_threshold {
            return t;
        }
        if level_id <= 1 {
            return 0;
        }
        match self.level_by_id(level_id - 1) {
            Some(prev) => ((prev.max_points() * 7 / 10 + 5) / 10) * 10,
            None => 0,
        }
    }
}

/// Yalnız ASCII küçük harf, rakam ve '-' içeren, boş olmayan metin mi?
fn is_slug_like(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `id`, `<prefix><K>` biçiminde mi? (K: en az bir ASCII rakam, başka bir şey yok)
fn has_numeric_suffix(id: &str, prefix: &str) -> bool {
    id.strip_prefix(prefix)
        .map(|k| !k.is_empty() && k.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
}

/// Markdown -> HTML (matematik/metadata uzantıları kapalı).
pub fn render_md(md: &str) -> String {
    use pulldown_cmark::Options;
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = pulldown_cmark::Parser::new_ext(md, opts);
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, parser);
    out
}

// ---------------------------------------------------------------- İngilizce üst katman
//
// `content/en/levelN/<aynı dosya adı>`: yalnız metinler (ve gerekiyorsa kod).
// id, puan ve `answer` Türkçe dosyadan gelir. Bilinmeyen alan hata verir.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExerciseOverlay {
    prompt: String,
    starter: Option<String>,
    tests: Option<String>,
    solution: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LessonOverlay {
    title: String,
    body_md: String,
    example: Option<String>,
    exercise: ExerciseOverlay,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct McqOverlay {
    id: String,
    q: String,
    choices: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodingOverlay {
    id: String,
    prompt: String,
    starter: Option<String>,
    tests: Option<String>,
    solution: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuizOverlay {
    #[serde(default)]
    mcq: Vec<McqOverlay>,
    #[serde(default)]
    coding: Vec<CodingOverlay>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelOverlay {
    title: String,
    description: String,
    #[serde(default)]
    quiz: QuizOverlay,
}

/// Dosya varsa TOML olarak okur; yoksa `None`.
fn read_overlay<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("okunamadı: {}", path.display()))?;
    let v = toml::from_str(&text).with_context(|| format!("TOML hatası: {}", path.display()))?;
    Ok(Some(v))
}

/// Verilmişse ve boş değilse `dst`'yi ezer; boş metin hatadır.
fn override_code(dst: &mut String, src: Option<String>, what: &str) -> Result<()> {
    if let Some(s) = src {
        if s.trim().is_empty() {
            bail!("{what} boş");
        }
        *dst = s;
    }
    Ok(())
}

fn apply_lesson_overlay(l: &mut Lesson, o: LessonOverlay) -> Result<()> {
    if o.title.trim().is_empty() || o.body_md.trim().is_empty() || o.exercise.prompt.trim().is_empty() {
        bail!("ders {} (en): title/body_md/prompt boş olamaz", l.id);
    }
    l.title = o.title;
    l.body_md = o.body_md;
    l.exercise.prompt = o.exercise.prompt;
    override_code(&mut l.example, o.example, &format!("ders {} (en): example", l.id))?;
    override_code(&mut l.exercise.starter, o.exercise.starter, &format!("ders {} (en): starter", l.id))?;
    override_code(&mut l.exercise.tests, o.exercise.tests, &format!("ders {} (en): tests", l.id))?;
    override_code(&mut l.exercise.solution, o.exercise.solution, &format!("ders {} (en): solution", l.id))?;
    Ok(())
}

/// Seviye üst katmanını uygular; karşılığı olmayan soruların id'lerini döner.
fn apply_level_overlay(lv: &mut Level, o: LevelOverlay) -> Result<Vec<String>> {
    if o.title.trim().is_empty() {
        bail!("seviye {} (en): title boş", lv.id);
    }
    lv.title = o.title;
    lv.description = o.description;
    for mo in &o.quiz.mcq {
        if !lv.quiz.mcq.iter().any(|m| m.id == mo.id) {
            bail!("seviye {} (en): bilinmeyen mcq id '{}'", lv.id, mo.id);
        }
    }
    for co in &o.quiz.coding {
        if !lv.quiz.coding.iter().any(|c| c.id == co.id) {
            bail!("seviye {} (en): bilinmeyen coding id '{}'", lv.id, co.id);
        }
    }
    let mut missing = Vec::new();
    let mut mcqs = o.quiz.mcq;
    for m in &mut lv.quiz.mcq {
        let Some(i) = mcqs.iter().position(|x| x.id == m.id) else {
            missing.push(m.id.clone());
            continue;
        };
        let mo = mcqs.swap_remove(i);
        if mo.choices.len() != m.choices.len() {
            bail!(
                "mcq {} (en): {} seçenek var, {} olmalı",
                m.id,
                mo.choices.len(),
                m.choices.len()
            );
        }
        m.q = mo.q;
        m.choices = mo.choices;
    }
    let mut codings = o.quiz.coding;
    for c in &mut lv.quiz.coding {
        let Some(i) = codings.iter().position(|x| x.id == c.id) else {
            missing.push(c.id.clone());
            continue;
        };
        let co = codings.swap_remove(i);
        c.prompt = co.prompt;
        override_code(&mut c.starter, co.starter, &format!("quiz {} (en): starter", c.id))?;
        override_code(&mut c.tests, co.tests, &format!("quiz {} (en): tests", c.id))?;
        override_code(&mut c.solution, co.solution, &format!("quiz {} (en): solution", c.id))?;
    }
    Ok(missing)
}

/// `dir` altındaki `levelN/` klasörlerini (ve varsa `en/levelN/` üst
/// katmanlarını) yükler ve doğrular.
pub fn load(dir: &Path) -> Result<Catalog> {
    let en_root = dir.join("en");
    let mut pairs: Vec<(Level, Level)> = Vec::new();
    let mut missing_en = Vec::new();
    let rd = fs::read_dir(dir)
        .with_context(|| format!("içerik dizini okunamadı: {}", dir.display()))?;
    for entry in rd {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let is_level_dir = name
            .strip_prefix("level")
            .map(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false);
        if !is_level_dir {
            continue;
        }
        let (level, en) = load_level(&path, &en_root.join(&name), &mut missing_en)?;
        let dir_num: u32 = name["level".len()..].parse().unwrap_or(u32::MAX);
        if dir_num != level.id {
            bail!(
                "{}: dizin numarası ({}) level.toml id ({}) ile eşleşmiyor",
                name,
                dir_num,
                level.id
            );
        }
        pairs.push((level, en));
    }
    pairs.sort_by_key(|p| p.0.id);
    let (levels, en): (Vec<Level>, Vec<Level>) = pairs.into_iter().unzip();
    missing_en.sort();
    let catalog = Catalog { levels, en, missing_en };
    validate(&catalog)?;
    Ok(catalog)
}

/// Seviyeyi ve İngilizce görünümünü yükler. Level 0 (fixture) için üst
/// katman aranmaz. Eksik üst katmanlar `missing` listesine yazılır.
fn load_level(dir: &Path, en_dir: &Path, missing: &mut Vec<String>) -> Result<(Level, Level)> {
    let lp = dir.join("level.toml");
    let text = fs::read_to_string(&lp)
        .with_context(|| format!("okunamadı: {}", lp.display()))?;
    let mut level: Level =
        toml::from_str(&text).with_context(|| format!("TOML hatası: {}", lp.display()))?;
    let mut en = level.clone();
    let translated = level.id > 0;
    if translated {
        match read_overlay::<LevelOverlay>(&en_dir.join("level.toml"))? {
            Some(o) => missing.extend(apply_level_overlay(&mut en, o)?),
            None => missing.push(format!("seviye {}", level.id)),
        }
    }

    let mut files = Vec::new();
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        let is_toml = p.extension().map(|x| x == "toml").unwrap_or(false);
        let is_level = p.file_name().map(|n| n == "level.toml").unwrap_or(false);
        if is_toml && !is_level {
            files.push(p);
        }
    }
    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    for f in files {
        let t = fs::read_to_string(&f)
            .with_context(|| format!("okunamadı: {}", f.display()))?;
        let lesson: Lesson =
            toml::from_str(&t).with_context(|| format!("TOML hatası: {}", f.display()))?;
        // Dosya adı "NN-slug.toml" ise ders id'si "<level>-<NN>" olmalı.
        let stem = f
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let nn: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
        let expected = format!("{}-{}", level.id, nn);
        if nn.len() != 2 || lesson.id != expected {
            bail!(
                "{}: ders id '{}' dosya adından beklenen '{}' ile eşleşmiyor",
                f.display(),
                lesson.id,
                expected
            );
        }
        let mut en_lesson = lesson.clone();
        if translated {
            let name = f.file_name().map(|n| n.to_os_string()).unwrap_or_default();
            match read_overlay::<LessonOverlay>(&en_dir.join(name))? {
                Some(o) => apply_lesson_overlay(&mut en_lesson, o)?,
                None => missing.push(lesson.id.clone()),
            }
        }
        level.lessons.push(lesson);
        en.lessons.push(en_lesson);
    }
    Ok((level, en))
}

fn validate(c: &Catalog) -> Result<()> {
    let mut level_ids = HashSet::new();
    let mut slugs = HashSet::new();
    let mut ids = HashSet::new();
    for lv in &c.levels {
        if !level_ids.insert(lv.id) {
            bail!("tekrar eden seviye id: {}", lv.id);
        }
        if !is_slug_like(&lv.slug) {
            bail!("seviye {}: geçersiz slug '{}'", lv.id, lv.slug);
        }
        if !slugs.insert(lv.slug.clone()) {
            bail!("tekrar eden seviye slug: {}", lv.slug);
        }
        let prefix = format!("{}-", lv.id);
        for l in &lv.lessons {
            if !is_slug_like(&l.id) || !l.id.starts_with(&prefix) {
                bail!("geçersiz ders id: '{}'", l.id);
            }
            if !ids.insert(l.id.clone()) {
                bail!("tekrar eden ders id: {}", l.id);
            }
            let ex = &l.exercise;
            if ex.tests.trim().is_empty() {
                bail!("ders {}: egzersiz tests boş", l.id);
            }
            if ex.starter.trim().is_empty() || ex.solution.trim().is_empty() {
                bail!("ders {}: egzersiz starter/solution boş", l.id);
            }
        }
        for q in &lv.quiz.coding {
            if !has_numeric_suffix(&q.id, &format!("{}-q", lv.id)) {
                bail!("geçersiz coding id: '{}' (beklenen '{}-qK')", q.id, lv.id);
            }
            if !ids.insert(q.id.clone()) {
                bail!("tekrar eden quiz id: {}", q.id);
            }
            if q.tests.trim().is_empty() {
                bail!("quiz {}: tests boş", q.id);
            }
            if q.starter.trim().is_empty() || q.solution.trim().is_empty() {
                bail!("quiz {}: starter/solution boş", q.id);
            }
        }
        for m in &lv.quiz.mcq {
            if !has_numeric_suffix(&m.id, &format!("{}-m", lv.id)) {
                bail!("geçersiz mcq id: '{}' (beklenen '{}-mK')", m.id, lv.id);
            }
            if !ids.insert(m.id.clone()) {
                bail!("tekrar eden mcq id: {}", m.id);
            }
            if m.choices.len() != 4 {
                bail!("mcq {}: tam 4 seçenek olmalı ({} var)", m.id, m.choices.len());
            }
            if m.answer >= m.choices.len() {
                bail!(
                    "mcq {}: answer {} aralık dışı ({} seçenek)",
                    m.id,
                    m.answer,
                    m.choices.len()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn content_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    fn tmp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "rla_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("level1")).unwrap();
        d
    }

    const LEVEL1_HEAD: &str = "id=1\nslug=\"a\"\ntitle=\"A\"\ndescription=\"d\"\n";

    fn mcq(answer: usize, choices: &str) -> String {
        format!(
            "[[quiz.mcq]]\nid=\"1-m1\"\nq=\"?\"\nchoices={}\nanswer={}\n",
            choices, answer
        )
    }

    #[test]
    fn level0_parse() {
        let c = load(&content_dir()).unwrap();
        let l0 = c.level_by_id(0).expect("level0 var");
        assert_eq!(l0.slug, "ornek");
        assert_eq!(c.effective_threshold(0), 0);
        assert_eq!(l0.quiz.mcq.len(), 2);
        assert_eq!(l0.quiz.coding.len(), 1);
        assert_eq!(l0.lessons.len(), 1);
        assert_eq!(c.lesson_by_id("0-01").unwrap().title, "Merhaba Dünya");
        assert_eq!(c.playable().filter(|l| l.id == 0).count(), 0);
        assert!(c.level_by_slug("ornek").is_some());
    }

    #[test]
    fn max_points_hesabi() {
        let c = load(&content_dir()).unwrap();
        // ders: 10+30, mcq: 2*10, coding: 50
        assert_eq!(c.level_by_id(0).unwrap().max_points(), 40 + 20 + 50);
    }

    #[test]
    fn esik_hesabi() {
        let mk = |id: u32, pts: u32, thr: Option<u32>| Level {
            id,
            slug: format!("s{id}"),
            title: String::new(),
            description: String::new(),
            unlock_threshold: thr,
            quiz: Quiz {
                mcq: vec![],
                coding: vec![CodingQuiz {
                    id: format!("{id}-q1"),
                    prompt: String::new(),
                    starter: String::new(),
                    tests: String::new(),
                    solution: String::new(),
                    points: pts,
                }],
            },
            lessons: vec![],
        };
        let c = Catalog {
            levels: vec![mk(1, 340, None), mk(2, 100, None), mk(3, 100, Some(5))],
            ..Default::default()
        };
        assert_eq!(c.effective_threshold(1), 0);
        assert_eq!(c.effective_threshold(2), 240); // 340*0.7=238 -> 240
        assert_eq!(c.effective_threshold(3), 5); // açık değer korunur
    }

    #[test]
    fn hatali_answer_reddedilir() {
        let d = tmp_dir();
        let s = format!("{}{}", LEVEL1_HEAD, mcq(5, "[\"a\",\"b\",\"c\",\"d\"]"));
        fs::write(d.join("level1/level.toml"), s).unwrap();
        let err = load(&d).unwrap_err().to_string();
        assert!(err.contains("aralık dışı"), "{err}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn dort_secenek_zorunlu() {
        let d = tmp_dir();
        let s = format!("{}{}", LEVEL1_HEAD, mcq(0, "[\"a\",\"b\"]"));
        fs::write(d.join("level1/level.toml"), s).unwrap();
        let err = load(&d).unwrap_err().to_string();
        assert!(err.contains("4 seçenek"), "{err}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn dizin_id_uyumsuzlugu_reddedilir() {
        let d = tmp_dir();
        fs::write(
            d.join("level1/level.toml"),
            "id=2\nslug=\"a\"\ntitle=\"A\"\ndescription=\"d\"\n",
        )
        .unwrap();
        let err = load(&d).unwrap_err().to_string();
        assert!(err.contains("eşleşmiyor"), "{err}");
        let _ = fs::remove_dir_all(&d);
    }

    fn lesson_toml(id: &str, tests: &str) -> String {
        format!(
            "id=\"{}\"\ntitle=\"t\"\nbody_md=\"x\"\nexample=\"e\"\n\
             [exercise]\nprompt=\"p\"\nstarter=\"s\"\ntests=\"{}\"\nsolution=\"s\"\n",
            id, tests
        )
    }

    #[test]
    fn bos_tests_reddedilir() {
        let d = tmp_dir();
        fs::write(d.join("level1/level.toml"), LEVEL1_HEAD).unwrap();
        fs::write(d.join("level1/01-a.toml"), lesson_toml("1-01", "  ")).unwrap();
        assert!(load(&d).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn dosya_adi_id_uyumsuzlugu_reddedilir() {
        let d = tmp_dir();
        fs::write(d.join("level1/level.toml"), LEVEL1_HEAD).unwrap();
        fs::write(d.join("level1/02-a.toml"), lesson_toml("1-01", "t")).unwrap();
        let err = load(&d).unwrap_err().to_string();
        assert!(err.contains("eşleşmiyor"), "{err}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn gecerli_ders_yuklenir() {
        let d = tmp_dir();
        fs::write(d.join("level1/level.toml"), LEVEL1_HEAD).unwrap();
        fs::write(d.join("level1/01-a.toml"), lesson_toml("1-01", "t")).unwrap();
        let c = load(&d).unwrap();
        assert_eq!(c.playable().count(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn ingilizce_ust_katman() {
        let d = tmp_dir();
        let level = format!("{}{}", LEVEL1_HEAD, mcq(0, "[\"a\",\"b\",\"c\",\"d\"]"));
        fs::write(d.join("level1/level.toml"), level).unwrap();
        fs::write(d.join("level1/01-a.toml"), lesson_toml("1-01", "t")).unwrap();
        fs::write(d.join("level1/02-b.toml"), lesson_toml("1-02", "t")).unwrap();
        fs::create_dir_all(d.join("en/level1")).unwrap();
        fs::write(
            d.join("en/level1/level.toml"),
            "title=\"A-en\"\ndescription=\"d-en\"\n[[quiz.mcq]]\nid=\"1-m1\"\nq=\"q-en\"\n\
             choices=[\"w\",\"x\",\"y\",\"z\"]\n",
        )
        .unwrap();
        fs::write(
            d.join("en/level1/01-a.toml"),
            "title=\"T-en\"\nbody_md=\"b-en\"\n[exercise]\nprompt=\"p-en\"\ntests=\"t-en\"\n",
        )
        .unwrap();
        let c = load(&d).unwrap();
        let tr = c.level_in(Lang::Tr, 1).unwrap();
        let en = c.level_in(Lang::En, 1).unwrap();
        assert_eq!((tr.title.as_str(), en.title.as_str()), ("A", "A-en"));
        assert_eq!(en.quiz.mcq[0].choices[3], "z");
        assert_eq!(en.quiz.mcq[0].answer, tr.quiz.mcq[0].answer);
        assert_eq!(en.lessons[0].title, "T-en");
        assert_eq!(en.lessons[0].exercise.tests, "t-en");
        // ezilmeyen kod iki dilde ortaktır
        assert_eq!(en.lessons[0].exercise.starter, tr.lessons[0].exercise.starter);
        assert_eq!(en.lessons[0].example, tr.lessons[0].example);
        // 1-02'nin üst katmanı yok: Türkçe kalır ve eksik listesine girer
        assert_eq!(en.lessons[1].title, tr.lessons[1].title);
        assert_eq!(c.missing_en, vec!["1-02".to_string()]);
        // üst katmanda izin verilmeyen alan reddedilir
        fs::write(
            d.join("en/level1/02-b.toml"),
            "title=\"x\"\nbody_md=\"y\"\nread_points=5\n[exercise]\nprompt=\"p\"\n",
        )
        .unwrap();
        assert!(load(&d).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn gercek_icerigin_ingilizcesi_tam() {
        let c = load(&content_dir()).unwrap();
        assert_eq!(c.missing_en, Vec::<String>::new());
        assert_eq!(c.en.len(), c.levels.len());
        for (tr, en) in c.levels.iter().zip(&c.en) {
            assert_eq!(tr.id, en.id);
            assert_eq!(tr.max_points(), en.max_points());
            assert_eq!(tr.lessons.len(), en.lessons.len());
        }
    }

    #[test]
    fn level1_esigi_level0_fixture_saymaz() {
        let mut c = load(&content_dir()).unwrap();
        assert_eq!(c.effective_threshold(1), 0);
        // Gerçek katalogda level0 fixture varken eşiksiz bir level1 eklense
        // bile level0 "önceki seviye" sayılmamalı.
        c.levels.push(Level {
            id: 1,
            slug: "l1".into(),
            title: String::new(),
            description: String::new(),
            unlock_threshold: None,
            quiz: Quiz::default(),
            lessons: vec![],
        });
        assert!(c.level_by_id(0).is_some());
        assert_eq!(c.effective_threshold(1), 0);
    }

    /// level1'e tek bir quiz kaydı yazar, yükleme hatasını (varsa) döner.
    fn quiz_id_hatasi(kind: &str, id: &str) -> Option<String> {
        let d = tmp_dir();
        let body = if kind == "mcq" {
            format!(
                "[[quiz.mcq]]\nid=\"{id}\"\nq=\"?\"\nchoices=[\"a\",\"b\",\"c\",\"d\"]\nanswer=0\n"
            )
        } else {
            format!(
                "[[quiz.coding]]\nid=\"{id}\"\nprompt=\"p\"\nstarter=\"s\"\ntests=\"t\"\nsolution=\"s\"\n"
            )
        };
        fs::write(d.join("level1/level.toml"), format!("{LEVEL1_HEAD}{body}")).unwrap();
        let r = load(&d).err().map(|e| e.to_string());
        let _ = fs::remove_dir_all(&d);
        r
    }

    #[test]
    fn quiz_id_sonek_yalniz_rakam() {
        for bad in ["1-qx", "1-q", "1-q1a", "1-q-1"] {
            let e = quiz_id_hatasi("coding", bad);
            assert!(e.as_deref().unwrap_or("").contains("geçersiz coding id"), "{bad}: {e:?}");
        }
        for bad in ["1-mx", "1-m", "1-m2b"] {
            let e = quiz_id_hatasi("mcq", bad);
            assert!(e.as_deref().unwrap_or("").contains("geçersiz mcq id"), "{bad}: {e:?}");
        }
        assert_eq!(quiz_id_hatasi("coding", "1-q12"), None);
        assert_eq!(quiz_id_hatasi("mcq", "1-m3"), None);
    }

    #[test]
    fn render_md_calisir() {
        let h = render_md("# Başlık\n\n- a\n- b");
        assert!(h.contains("<h1>Başlık</h1>"));
        assert!(h.contains("<li>a</li>"));
        // matematik uzantısı kapalı: $ işareti olduğu gibi kalır
        assert!(render_md("$x + $y").contains("$x + $y"));
    }
}
