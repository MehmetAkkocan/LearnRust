//! Web katmanı: Axum + Askama + HTMX.

use std::sync::Arc;
use std::time::{Duration, Instant};

use askama::Template;
use axum::{
    extract::{DefaultBodyLimit, Form, Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use tower_http::services::ServeDir;

use crate::content::{render_md, Catalog, Level};
use crate::game::{self, AwardError, LevelStatus};
use crate::i18n::{Lang, Ui};
use crate::runner::{self, RunOutcome};
use crate::store::{Progress, Store};

#[derive(Clone)]
pub struct AppState {
    pub catalog: Arc<Catalog>,
    pub store: Arc<Store>,
}

// ---------------------------------------------------------------- görünümler

#[derive(Clone, Debug)]
pub struct StatsView {
    pub points: u32,
    pub max: u32,
    pub oob: bool,
}

#[derive(Clone, Debug)]
pub struct BadgeView {
    pub code: String,
    pub label: String,
}

pub struct CardView {
    pub id: u32,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub percent: u32,
    /// "120 / 340 puan · %35"
    pub meta: String,
    /// Kilitli kartta: açmak için gereken puan bilgisi.
    pub unlock_info: String,
    pub locked: bool,
    pub status_class: &'static str,
    pub status_label: &'static str,
}

pub struct LevelHead {
    pub id: u32,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub percent: u32,
    pub meta: String,
}

pub struct LessonRow {
    pub id: String,
    pub title: String,
    pub read: bool,
    pub done: bool,
}

pub struct ChoiceView {
    pub idx: usize,
    pub html: String,
}

pub struct McqView {
    pub id: String,
    pub q_html: String,
    pub points: u32,
    pub choices: Vec<ChoiceView>,
    /// "" | "answered-ok" | "answered-fail"
    pub status: &'static str,
}

pub struct CodingView {
    pub id: String,
    pub prompt_html: String,
    pub starter: String,
    pub points: u32,
    pub done: bool,
}

#[derive(Clone, Debug)]
pub struct UnlockView {
    pub slug: String,
    pub title: String,
}

pub struct McqItem {
    pub q_html: String,
    pub ok: bool,
    pub gained: u32,
    pub correct_html: String,
}

// ---------------------------------------------------------------- şablonlar
//
// Her şablon `t` (arayüz metinleri) alır; dahil edilen parçalar (stats,
// badges, unlock) da onu kullanır.

#[derive(Template)]
#[template(path = "panel.html")]
struct PanelTpl {
    t: &'static Ui,
    title: String,
    stats: StatsView,
    badges: Vec<BadgeView>,
    cards: Vec<CardView>,
}

#[derive(Template)]
#[template(path = "seviye.html")]
struct SeviyeTpl {
    t: &'static Ui,
    title: String,
    stats: StatsView,
    level: LevelHead,
    lessons: Vec<LessonRow>,
    has_quiz: bool,
    quiz_label: String,
}

#[derive(Template)]
#[template(path = "ders.html")]
struct DersTpl {
    t: &'static Ui,
    title: String,
    stats: StatsView,
    level_slug: String,
    level_title: String,
    lesson_id: String,
    lesson_title: String,
    body_html: String,
    example: String,
    prompt_html: String,
    starter: String,
    read: bool,
    exercise_done: bool,
    read_points: u32,
    exercise_points: u32,
    has_next: bool,
    next_id: String,
    next_title: String,
    has_quiz: bool,
    /// Tüm derslerin okuma + egzersizi tamam mı (test linki için).
    level_complete: bool,
}

#[derive(Template)]
#[template(path = "test.html")]
struct TestTpl {
    t: &'static Ui,
    title: String,
    stats: StatsView,
    slug: String,
    level_title: String,
    mcqs: Vec<McqView>,
    codings: Vec<CodingView>,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTpl {
    t: &'static Ui,
    title: String,
    stats: StatsView,
    code: u16,
    heading: String,
    message: String,
}

#[derive(Template)]
#[template(path = "read_result.html")]
struct ReadResultTpl {
    t: &'static Ui,
    stats: StatsView,
    ok: bool,
    message: String,
    badges: Vec<BadgeView>,
    unlocked: Vec<UnlockView>,
}

#[derive(Template)]
#[template(path = "run_result.html")]
struct RunResultTpl {
    t: &'static Ui,
    stats: StatsView,
    class_name: &'static str,
    title: String,
    output: String,
    /// "+30 puan kazandın!"; puan yoksa boş.
    gained_msg: String,
    note: String,
    badges: Vec<BadgeView>,
    unlocked: Vec<UnlockView>,
    next_href: String,
    next_label: String,
}

#[derive(Template)]
#[template(path = "mcq_result.html")]
struct McqResultTpl {
    t: &'static Ui,
    stats: StatsView,
    items: Vec<McqItem>,
    gained_total: u32,
    error: String,
    badges: Vec<BadgeView>,
    unlocked: Vec<UnlockView>,
}

// ---------------------------------------------------------------- yardımcılar

/// Şablonu HTML yanıtına çevirir; hata olursa 500 sayfası.
fn page<T: Template>(t: T) -> Response {
    match t.render() {
        Ok(s) => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            s,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("şablon hatası: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Şablon hatası").into_response()
        }
    }
}

fn stats_of(c: &Catalog, p: &Progress, oob: bool) -> StatsView {
    StatsView {
        points: game::total_points(c, p),
        max: game::total_max(c),
        oob,
    }
}

fn badge_views(lang: Lang, ids: &[String]) -> Vec<BadgeView> {
    ids.iter()
        .map(|id| BadgeView {
            code: id.clone(),
            label: lang.badge_label(id),
        })
        .collect()
}

/// Markdown -> satır içi HTML (dış <p> sarmalayıcısı atılır).
fn inline_md(s: &str) -> String {
    let h = render_md(s);
    let t = h.trim();
    match t.strip_prefix("<p>").and_then(|x| x.strip_suffix("</p>")) {
        Some(inner) if !inner.contains("<p>") => inner.to_string(),
        _ => t.to_string(),
    }
}

/// Seçilen dildeki oynanabilir seviye (id > 0).
fn playable_level<'a>(c: &'a Catalog, lang: Lang, slug: &str) -> Option<&'a Level> {
    c.levels_in(lang).iter().find(|l| l.id > 0 && l.slug == slug)
}

fn level_of_lesson<'a>(c: &'a Catalog, lang: Lang, lesson_id: &str) -> Option<&'a Level> {
    c.levels_in(lang)
        .iter()
        .filter(|l| l.id > 0)
        .find(|l| l.lesson_by_id(lesson_id).is_some())
}

fn has_quiz(lv: &Level) -> bool {
    !lv.quiz.mcq.is_empty() || !lv.quiz.coding.is_empty()
}

/// Seviyeyi açmak için "X puan daha" değeri.
fn points_needed(c: &Catalog, p: &Progress, level_id: u32) -> u32 {
    if level_id <= 1 {
        return 0;
    }
    c.effective_threshold(level_id)
        .saturating_sub(game::level_points(c, p, level_id - 1))
}

// ---------------------------------------------------------------- hata türü

pub struct AppError {
    t: &'static Ui,
    status: StatusCode,
    heading: String,
    message: String,
    stats: StatsView,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let code = self.status.as_u16();
        let tpl = ErrorTpl {
            t: self.t,
            title: self.heading.clone(),
            stats: self.stats,
            code,
            heading: self.heading,
            message: self.message,
        };
        let mut r = page(tpl);
        *r.status_mut() = self.status;
        r
    }
}

impl AppState {
    fn stats(&self, oob: bool) -> StatsView {
        let p = self.store.snapshot();
        stats_of(&self.catalog, &p, oob)
    }
    fn not_found(&self, lang: Lang) -> AppError {
        let t = lang.ui();
        AppError {
            t,
            status: StatusCode::NOT_FOUND,
            heading: t.not_found.into(),
            message: t.not_found_msg.into(),
            stats: self.stats(false),
        }
    }
    fn locked(&self, lang: Lang, level: &Level, p: &Progress) -> AppError {
        let need = points_needed(&self.catalog, p, level.id);
        AppError {
            t: lang.ui(),
            status: StatusCode::FORBIDDEN,
            heading: lang.ui().locked.into(),
            message: lang.locked_level(&level.title, need),
            stats: stats_of(&self.catalog, p, false),
        }
    }
    fn internal(&self, lang: Lang, e: impl std::fmt::Display) -> AppError {
        tracing::error!("iç hata: {e}");
        let t = lang.ui();
        AppError {
            t,
            status: StatusCode::INTERNAL_SERVER_ERROR,
            heading: t.server_error.into(),
            message: t.server_error_msg.into(),
            stats: self.stats(false),
        }
    }

    /// Store güncellemesini bloklayan iş parçacığında yapar; sonuç + güncel
    /// ilerleme kopyası + bu güncellemeyle açılan seviyeler döner.
    async fn update<R, F>(
        &self,
        lang: Lang,
        f: F,
    ) -> Result<(R, Progress, Vec<UnlockView>), AppError>
    where
        R: Send + 'static,
        F: FnOnce(&Catalog, &mut Progress) -> R + Send + 'static,
    {
        let cat = self.catalog.clone();
        let store = self.store.clone();
        let joined = tokio::task::spawn_blocking(move || {
            store.update(|p| {
                let before: Vec<u32> = cat
                    .playable()
                    .filter(|l| game::is_unlocked(&cat, p, l.id))
                    .map(|l| l.id)
                    .collect();
                let r = f(&cat, p);
                let unlocked = cat
                    .levels_in(lang)
                    .iter()
                    .filter(|l| l.id > 0)
                    .filter(|l| game::is_unlocked(&cat, p, l.id) && !before.contains(&l.id))
                    .map(|l| UnlockView {
                        slug: l.slug.clone(),
                        title: l.title.clone(),
                    })
                    .collect::<Vec<_>>();
                (r, p.clone(), unlocked)
            })
        })
        .await;
        match joined {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(self.internal(lang, e)),
            Err(e) => Err(self.internal(lang, e)),
        }
    }
}

fn award_msg(t: &Ui, e: &AwardError) -> String {
    match e {
        AwardError::Locked(_) => t.level_locked.to_string(),
        AwardError::Unknown(_) => t.unknown_content.to_string(),
    }
}

// ---------------------------------------------------------------- rotalar

pub fn router(state: AppState) -> Router {
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    Router::new()
        .route("/", get(panel))
        .route("/dil/:code", get(set_lang))
        .route("/seviye/:slug", get(seviye))
        .route("/seviye/:slug/test", get(test_page))
        .route("/seviye/:slug/test/mcq", post(test_mcq))
        .route("/seviye/:slug/test/kod/:id", post(test_kod))
        .route("/ders/:id", get(ders))
        .route("/ders/:id/okudum", post(okudum))
        .route("/ders/:id/calistir", post(calistir))
        .nest_service("/static", ServeDir::new(static_dir))
        .fallback(fallback)
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(tower_http::timeout::TimeoutLayer::new(Duration::from_secs(90)))
        .layer(middleware::from_fn(csrf_guard))
        .layer(middleware::from_fn(log_requests))
        .with_state(state)
}

/// Çalıştırıcı için toplam üst sınır (semafor bekleyişi + derleme + çalıştırma);
/// istek düzeyindeki 90 sn zaman aşımından önce dolar.
const RUN_DEADLINE: Duration = Duration::from_secs(80);

/// CSRF koruması: durum değiştiren (POST) isteklerde `HX-Request: true`
/// başlığı zorunlu (çapraz kaynaklı basit form gönderimi bunu ekleyemez) ve
/// varsa `Origin` ana bilgisayarı `Host` ile aynı olmalı.
async fn csrf_guard(req: Request, next: Next) -> Response {
    if req.method() != axum::http::Method::GET && req.method() != axum::http::Method::HEAD {
        let h = req.headers();
        let hx_ok = h
            .get("hx-request")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let origin_ok = match h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            None => true,
            Some(o) => {
                let o_host = o.split("://").nth(1).unwrap_or(o);
                let host = h
                    .get(header::HOST)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                o_host == host
            }
        };
        if !hx_ok || !origin_ok {
            return (StatusCode::FORBIDDEN, "Yasak: geçersiz istek kaynağı.").into_response();
        }
    }
    next.run(req).await
}

async fn log_requests(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let t = Instant::now();
    let res = next.run(req).await;
    tracing::info!(
        "{} {} -> {} ({} ms)",
        method,
        path,
        res.status().as_u16(),
        t.elapsed().as_millis()
    );
    res
}

async fn fallback(State(st): State<AppState>, lang: Lang) -> AppError {
    st.not_found(lang)
}

/// `Referer` adresinin yalnız yol kısmı. Ana bilgisayar atılır ki dil
/// bağlantısı başka bir siteye yönlendirme için kullanılamasın.
fn local_path(referer: &str) -> String {
    let rest = referer.split_once("://").map(|x| x.1).unwrap_or(referer);
    match rest.find('/') {
        Some(i) => {
            let p = &rest[i..];
            if p.starts_with("//") || p.starts_with("/\\") || p.starts_with("/dil/") {
                "/".to_string()
            } else {
                p.to_string()
            }
        }
        None => "/".to_string(),
    }
}

/// Dil seçimi: `lang` çerezini yazar ve gelinen sayfaya geri döner.
/// Yalnız bir görüntüleme tercihi değiştiği için GET ile yapılır.
async fn set_lang(Path(code): Path<String>, headers: HeaderMap) -> Response {
    let lang = Lang::from_code(&code).unwrap_or_default();
    let back = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(local_path)
        .unwrap_or_else(|| "/".to_string());
    let cookie = format!(
        "lang={}; Path=/; Max-Age=31536000; SameSite=Lax",
        lang.code()
    );
    (
        StatusCode::SEE_OTHER,
        [(header::SET_COOKIE, cookie), (header::LOCATION, back)],
    )
        .into_response()
}

async fn panel(State(st): State<AppState>, lang: Lang) -> Response {
    let t = lang.ui();
    let p = st.store.snapshot();
    let s = game::summary(&st.catalog, &p);
    let cards = s
        .levels
        .iter()
        .map(|l| {
            let (title, description) = st
                .catalog
                .level_in(lang, l.id)
                .map(|x| (x.title.clone(), x.description.clone()))
                .unwrap_or_default();
            let (class, label) = match l.status {
                LevelStatus::Locked => ("locked", t.status_locked),
                LevelStatus::Open => ("open", t.status_open),
                LevelStatus::Completed => ("completed", t.status_completed),
            };
            CardView {
                id: l.id,
                slug: l.slug.clone(),
                title,
                description,
                percent: l.percent,
                meta: lang.meta(l.points, l.max, l.percent),
                unlock_info: lang
                    .unlock_info(l.threshold, points_needed(&st.catalog, &p, l.id)),
                locked: l.status == LevelStatus::Locked,
                status_class: class,
                status_label: label,
            }
        })
        .collect();
    page(PanelTpl {
        t,
        title: t.panel_title.into(),
        stats: StatsView {
            points: s.total_points,
            max: s.total_max,
            oob: false,
        },
        badges: badge_views(lang, &s.badges),
        cards,
    })
}

async fn seviye(
    State(st): State<AppState>,
    lang: Lang,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let c = &st.catalog;
    let lv = playable_level(c, lang, &slug).ok_or_else(|| st.not_found(lang))?;
    let p = st.store.snapshot();
    if !game::is_unlocked(c, &p, lv.id) {
        return Err(st.locked(lang, lv, &p));
    }
    let lessons = lv
        .lessons
        .iter()
        .map(|l| LessonRow {
            id: l.id.clone(),
            title: l.title.clone(),
            read: p.reads.contains(&l.id),
            done: p.exercises.contains(&l.id),
        })
        .collect();
    let percent = game::level_percent(c, &p, lv.id);
    Ok(page(SeviyeTpl {
        t: lang.ui(),
        title: lv.title.clone(),
        stats: stats_of(c, &p, false),
        level: LevelHead {
            id: lv.id,
            slug: lv.slug.clone(),
            title: lv.title.clone(),
            description: lv.description.clone(),
            percent,
            meta: lang.meta(game::level_points(c, &p, lv.id), lv.max_points(), percent),
        },
        lessons,
        has_quiz: has_quiz(lv),
        quiz_label: lang.quiz_label(lv.quiz.mcq.len(), lv.quiz.coding.len()),
    }))
}

async fn ders(
    State(st): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let c = &st.catalog;
    let lv = level_of_lesson(c, lang, &id).ok_or_else(|| st.not_found(lang))?;
    let p = st.store.snapshot();
    if !game::is_unlocked(c, &p, lv.id) {
        return Err(st.locked(lang, lv, &p));
    }
    let idx = lv.lessons.iter().position(|l| l.id == id).unwrap_or(0);
    let lesson = &lv.lessons[idx];
    let next = lv.lessons.get(idx + 1);
    Ok(page(DersTpl {
        t: lang.ui(),
        title: lesson.title.clone(),
        stats: stats_of(c, &p, false),
        level_slug: lv.slug.clone(),
        level_title: lv.title.clone(),
        lesson_id: lesson.id.clone(),
        lesson_title: lesson.title.clone(),
        body_html: lesson.body_html(),
        example: lesson.example.clone(),
        prompt_html: render_md(&lesson.exercise.prompt),
        starter: lesson.exercise.starter.clone(),
        read: p.reads.contains(&lesson.id),
        exercise_done: p.exercises.contains(&lesson.id),
        read_points: lesson.read_points,
        exercise_points: lesson.exercise.points,
        has_next: next.is_some(),
        next_id: next.map(|n| n.id.clone()).unwrap_or_default(),
        next_title: next.map(|n| n.title.clone()).unwrap_or_default(),
        has_quiz: has_quiz(lv),
        level_complete: lv
            .lessons
            .iter()
            .all(|l| p.reads.contains(&l.id) && p.exercises.contains(&l.id)),
    }))
}

async fn okudum(
    State(st): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let t = lang.ui();
    let lid = id.clone();
    let (res, p, unlocked) = st
        .update(lang, move |c, p| game::award_read(c, p, &lid))
        .await?;
    let stats = stats_of(&st.catalog, &p, true);
    Ok(page(match res {
        Ok(a) => ReadResultTpl {
            t,
            stats,
            ok: true,
            message: if a.gained > 0 {
                lang.read_ok(a.gained)
            } else {
                t.read_again.to_string()
            },
            badges: badge_views(lang, &a.new_badges),
            unlocked,
        },
        Err(e) => ReadResultTpl {
            t,
            stats,
            ok: false,
            message: award_msg(t, &e),
            badges: vec![],
            unlocked: vec![],
        },
    }))
}

#[derive(Deserialize)]
struct CodeForm {
    #[serde(default)]
    code: String,
}

/// Runner sonucunu şablona çevirir; geçtiyse `award` ile puan verilir.
async fn run_and_award<F>(
    st: &AppState,
    lang: Lang,
    code: &str,
    tests: &str,
    next: Option<(String, String)>,
    award: F,
) -> Result<Response, AppError>
where
    F: FnOnce(&Catalog, &mut Progress) -> Result<game::AwardResult, AwardError> + Send + 'static,
{
    let t = lang.ui();
    // Semafor bekleyişi dahil toplam süre sınırlı; aşılırsa future düşer
    // (runner kill_on_drop ile çocuğu öldürür).
    let r = match tokio::time::timeout(RUN_DEADLINE, runner::run_tests(code, tests)).await {
        Ok(r) => r,
        Err(_) => runner::RunResult {
            outcome: RunOutcome::Error,
            stdout: String::new(),
            stderr: t.busy.to_string(),
            duration_ms: RUN_DEADLINE.as_millis(),
        },
    };
    let combined = |r: &runner::RunResult| {
        let mut o = r.stdout.clone();
        if !r.stderr.is_empty() {
            if !o.is_empty() && !o.ends_with('\n') {
                o.push('\n');
            }
            o.push_str(&r.stderr);
        }
        o
    };
    let (class_name, title, output) = match r.outcome {
        RunOutcome::Passed => ("result-ok", t.all_passed, combined(&r)),
        RunOutcome::TestsFailed => ("result-fail", t.tests_failed, combined(&r)),
        RunOutcome::CompileError => ("result-fail", t.compile_error, r.stderr.clone()),
        RunOutcome::Timeout => ("result-fail", t.timeout, combined(&r)),
        RunOutcome::Error => ("result-fail", t.run_error, combined(&r)),
    };
    let mut gained_msg = String::new();
    let mut note = String::new();
    let mut badges = vec![];
    let mut unlocked = vec![];
    let stats;
    if r.outcome == RunOutcome::Passed {
        let (res, p, un) = st.update(lang, award).await?;
        unlocked = un;
        stats = stats_of(&st.catalog, &p, true);
        match res {
            Ok(a) => {
                badges = badge_views(lang, &a.new_badges);
                if a.gained > 0 {
                    gained_msg = lang.gained(a.gained);
                } else {
                    note = t.no_extra_points.into();
                }
            }
            Err(e) => note = award_msg(t, &e),
        }
    } else {
        stats = st.stats(true);
    }
    let (next_href, next_label) = match next {
        Some(n) if r.outcome == RunOutcome::Passed => n,
        _ => (String::new(), String::new()),
    };
    Ok(page(RunResultTpl {
        t,
        stats,
        class_name,
        title: title.to_string(),
        output: lang.runner_output(output),
        gained_msg,
        note,
        badges,
        unlocked,
        next_href,
        next_label,
    }))
}

/// Kilitli/bilinmeyen durumlar için HTMX parçası (200 ile, htmx 4xx'i takas etmez).
fn fragment_fail(st: &AppState, lang: Lang, msg: &str) -> Response {
    page(RunResultTpl {
        t: lang.ui(),
        stats: st.stats(true),
        class_name: "result-fail",
        title: msg.to_string(),
        output: String::new(),
        gained_msg: String::new(),
        note: String::new(),
        badges: vec![],
        unlocked: vec![],
        next_href: String::new(),
        next_label: String::new(),
    })
}

async fn calistir(
    State(st): State<AppState>,
    lang: Lang,
    Path(id): Path<String>,
    Form(f): Form<CodeForm>,
) -> Result<Response, AppError> {
    let t = lang.ui();
    // Testler de seçilen dilden alınır: İngilizce görev farklı adlar kullanabilir.
    let Some(lv) = level_of_lesson(&st.catalog, lang, &id) else {
        return Ok(fragment_fail(&st, lang, t.unknown_lesson));
    };
    let p = st.store.snapshot();
    if !game::is_unlocked(&st.catalog, &p, lv.id) {
        return Ok(fragment_fail(&st, lang, t.level_locked));
    }
    let tests = lv.lesson_by_id(&id).map(|l| l.exercise.tests.clone()).unwrap_or_default();
    let idx = lv.lessons.iter().position(|l| l.id == id).unwrap_or(0);
    let next = match lv.lessons.get(idx + 1) {
        Some(n) => (format!("/ders/{}", n.id), t.next_lesson.to_string()),
        None if has_quiz(lv) => (format!("/seviye/{}/test", lv.slug), t.level_quiz.to_string()),
        None => (format!("/seviye/{}", lv.slug), t.back_to_level.to_string()),
    };
    run_and_award(&st, lang, &f.code, &tests, Some(next), move |c, p| {
        game::award_exercise(c, p, &id)
    })
    .await
}

async fn test_page(
    State(st): State<AppState>,
    lang: Lang,
    Path(slug): Path<String>,
) -> Result<Response, AppError> {
    let c = &st.catalog;
    let lv = playable_level(c, lang, &slug).ok_or_else(|| st.not_found(lang))?;
    let p = st.store.snapshot();
    if !game::is_unlocked(c, &p, lv.id) {
        return Err(st.locked(lang, lv, &p));
    }
    let mcqs = lv
        .quiz
        .mcq
        .iter()
        .map(|m| McqView {
            id: m.id.clone(),
            q_html: inline_md(&m.q),
            points: m.points,
            choices: m
                .choices
                .iter()
                .enumerate()
                .map(|(i, t)| ChoiceView { idx: i, html: inline_md(t) })
                .collect(),
            status: match p.mcq.get(&m.id) {
                Some(true) => "answered-ok",
                Some(false) => "answered-fail",
                None => "",
            },
        })
        .collect();
    let codings = lv
        .quiz
        .coding
        .iter()
        .map(|q| CodingView {
            id: q.id.clone(),
            prompt_html: render_md(&q.prompt),
            starter: q.starter.clone(),
            points: q.points,
            done: p.coding.contains(&q.id),
        })
        .collect();
    Ok(page(TestTpl {
        t: lang.ui(),
        title: lang.quiz_title(&lv.title),
        stats: stats_of(c, &p, false),
        slug: lv.slug.clone(),
        level_title: lv.title.clone(),
        mcqs,
        codings,
    }))
}

async fn test_mcq(
    State(st): State<AppState>,
    lang: Lang,
    Path(slug): Path<String>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Result<Response, AppError> {
    let t = lang.ui();
    let fail = |msg: &str| {
        page(McqResultTpl {
            t,
            stats: st.stats(true),
            items: vec![],
            gained_total: 0,
            error: msg.to_string(),
            badges: vec![],
            unlocked: vec![],
        })
    };
    let Some(lv) = playable_level(&st.catalog, lang, &slug) else {
        return Ok(fail(t.unknown_level));
    };
    if !game::is_unlocked(&st.catalog, &st.store.snapshot(), lv.id) {
        return Ok(fail(t.level_locked));
    }
    // (soru id, seçilen indeks) — yalnız bu seviyenin soruları ve geçerli indeksler.
    let answers: Vec<(String, usize)> = lv
        .quiz
        .mcq
        .iter()
        .filter_map(|m| {
            let key = format!("q_{}", m.id);
            let v = fields.iter().find(|(k, _)| *k == key)?.1.parse::<usize>().ok()?;
            (v < m.choices.len()).then(|| (m.id.clone(), v))
        })
        .collect();
    let answers2 = answers.clone();
    let (results, p, unlocked) = st
        .update(lang, move |c, p| {
            answers2
                .iter()
                .map(|(id, ch)| game::award_mcq_choice(c, p, id, *ch))
                .collect::<Vec<_>>()
        })
        .await?;
    let mut items = vec![];
    let mut gained_total = 0;
    let mut new_badges: Vec<String> = vec![];
    let mut error = String::new();
    for ((id, _), res) in answers.iter().zip(results) {
        let Some(m) = lv.quiz.mcq.iter().find(|m| &m.id == id) else { continue };
        match res {
            Ok(a) => {
                gained_total += a.gained;
                for b in a.new_badges {
                    if !new_badges.contains(&b) {
                        new_badges.push(b);
                    }
                }
                items.push(McqItem {
                    q_html: inline_md(&m.q),
                    ok: a.correct,
                    gained: a.gained,
                    correct_html: inline_md(&m.choices[m.answer]),
                });
            }
            Err(e) => error = award_msg(t, &e),
        }
    }
    Ok(page(McqResultTpl {
        t,
        stats: stats_of(&st.catalog, &p, true),
        items,
        gained_total,
        error,
        badges: badge_views(lang, &new_badges),
        unlocked,
    }))
}

async fn test_kod(
    State(st): State<AppState>,
    lang: Lang,
    Path((slug, id)): Path<(String, String)>,
    Form(f): Form<CodeForm>,
) -> Result<Response, AppError> {
    let t = lang.ui();
    let Some(lv) = playable_level(&st.catalog, lang, &slug) else {
        return Ok(fragment_fail(&st, lang, t.unknown_level));
    };
    let Some(q) = lv.quiz.coding.iter().find(|q| q.id == id) else {
        return Ok(fragment_fail(&st, lang, t.unknown_question));
    };
    let p = st.store.snapshot();
    if !game::is_unlocked(&st.catalog, &p, lv.id) {
        return Ok(fragment_fail(&st, lang, t.level_locked));
    }
    let tests = q.tests.clone();
    run_and_award(&st, lang, &f.code, &tests, None, move |c, p| {
        game::award_coding(c, p, &id)
    })
    .await
}

// ---------------------------------------------------------------- testler

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    fn parts() -> (Router, AppState) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("content");
        let catalog = crate::content::load(&dir).expect("içerik yüklenmeli");
        let tmp = std::env::temp_dir().join(format!(
            "rla_web_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let store = Store::open(tmp.join("progress.json")).unwrap();
        let st = AppState {
            catalog: Arc::new(catalog),
            store: Arc::new(store),
        };
        (router(st.clone()), st)
    }

    fn app() -> Router {
        parts().0
    }

    async fn send(app: &Router, req: Request<Body>) -> (StatusCode, String) {
        let r = app.clone().oneshot(req).await.unwrap();
        let s = r.status();
        let b = to_bytes(r.into_body(), 4 * 1024 * 1024).await.unwrap();
        (s, String::from_utf8_lossy(&b).to_string())
    }

    async fn get_status(app: &Router, uri: &str) -> (StatusCode, String) {
        send(app, Request::builder().uri(uri).body(Body::empty()).unwrap()).await
    }

    /// HTMX gibi davranan POST (HX-Request başlığıyla).
    async fn post_hx(app: &Router, uri: &str, body: &str) -> (StatusCode, String) {
        send(
            app,
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("hx-request", "true")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
    }

    #[tokio::test]
    async fn panel_200() {
        let (s, body) = get_status(&app(), "/").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("level-card"));
        assert!(body.contains("id=\"stats\""));
    }

    #[tokio::test]
    async fn ders_ve_seviye_200() {
        let app = app();
        let (s, body) = get_status(&app, "/ders/1-01").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("Okudum"));
        assert_eq!(get_status(&app, "/seviye/temeller").await.0, StatusCode::OK);
        assert_eq!(get_status(&app, "/seviye/temeller/test").await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn bilinmeyenler_404_ve_kilitli_403() {
        let app = app();
        assert_eq!(get_status(&app, "/ders/yok").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get_status(&app, "/seviye/ornek").await.0, StatusCode::NOT_FOUND);
        assert_eq!(get_status(&app, "/nope").await.0, StatusCode::NOT_FOUND);
        let (s, body) = get_status(&app, "/ders/2-01").await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        assert!(body.contains("Kilitli"));
    }

    #[tokio::test]
    async fn csrf_korumasi() {
        let app = app();
        let mk = |hx: bool, origin: Option<&str>| {
            let mut b = Request::builder()
                .method("POST")
                .uri("/ders/1-01/okudum")
                .header("host", "localhost:8080");
            if hx {
                b = b.header("hx-request", "true");
            }
            if let Some(o) = origin {
                b = b.header("origin", o);
            }
            b.body(Body::empty()).unwrap()
        };
        assert_eq!(send(&app, mk(false, None)).await.0, StatusCode::FORBIDDEN);
        assert_eq!(
            send(&app, mk(true, Some("http://evil.example"))).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(send(&app, mk(true, None)).await.0, StatusCode::OK);
        assert_eq!(
            send(&app, mk(true, Some("http://localhost:8080"))).await.0,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn okudum_puan_verir() {
        let (s, t) = post_hx(&app(), "/ders/1-01/okudum", "").await;
        assert_eq!(s, StatusCode::OK);
        assert!(t.contains("hx-swap-oob"));
        assert!(t.contains("result-ok"));
    }

    #[tokio::test]
    async fn okudum_kilitli_reddedilir() {
        let (s, t) = post_hx(&app(), "/ders/2-01/okudum", "").await;
        assert_eq!(s, StatusCode::OK);
        assert!(t.contains("result-fail"));
        assert!(t.contains("kilitli"));
        assert!(!t.contains("result-ok"));
    }

    #[tokio::test]
    async fn mcq_sunucuda_dogrulanir() {
        let (app, st) = parts();
        let m = st.catalog.level_by_id(1).unwrap().quiz.mcq[0].clone();
        let wrong = (m.answer + 1) % m.choices.len();
        let key = format!("q_{}", m.id);

        // yanlış indeks: puan yok
        let (_, t) = post_hx(&app, "/seviye/temeller/test/mcq", &format!("{key}={wrong}")).await;
        assert!(t.contains("Yanlış"), "{t}");
        assert!(!t.contains("Toplam +"));
        // aralık dışı indeks yok sayılır
        let (_, t) = post_hx(&app, "/seviye/temeller/test/mcq", &format!("{key}=99")).await;
        assert!(t.contains("Hiçbir soru"), "{t}");
        // doğru indeks: puan
        let (_, t) =
            post_hx(&app, "/seviye/temeller/test/mcq", &format!("{key}={}", m.answer)).await;
        assert!(t.contains("Doğru!"), "{t}");
        assert!(t.contains(&format!("+{}", m.points)));
        // kilitli seviye
        let (s, t) = post_hx(&app, "/seviye/temeller/test/mcq", "").await;
        assert_eq!(s, StatusCode::OK);
        assert!(t.contains("Hiçbir soru"));
        if let Some(l2) = st.catalog.level_by_id(2) {
            let (_, t) = post_hx(&app, &format!("/seviye/{}/test/mcq", l2.slug), "").await;
            assert!(t.contains("kilitli"), "{t}");
        }
    }

    #[tokio::test]
    async fn seviye_acilis_bildirimi() {
        let (app, st) = parts();
        let c = st.catalog.clone();
        let Some(l2) = c.level_by_id(2) else { return };
        let thr = c.effective_threshold(2);
        // Seviye 1'i eşiğin 1 okuma puanı altına kadar doldur (1-01 okuması hariç).
        let mut p = Progress::default();
        let l1 = c.level_by_id(1).unwrap();
        let rp = l1.lessons[0].read_points;
        let mut items: Vec<Box<dyn Fn(&mut Progress)>> = vec![];
        for l in l1.lessons.iter().skip(1) {
            let id = l.id.clone();
            let c2 = c.clone();
            items.push(Box::new(move |p| {
                game::award_exercise(&c2, p, &id).unwrap();
            }));
            let id = l.id.clone();
            let c2 = c.clone();
            items.push(Box::new(move |p| {
                game::award_read(&c2, p, &id).unwrap();
            }));
        }
        let first_ex = l1.lessons[0].id.clone();
        let c2 = c.clone();
        items.push(Box::new(move |p| {
            game::award_exercise(&c2, p, &first_ex).unwrap();
        }));
        for it in items {
            if game::level_points(&c, &p, 1) + rp >= thr {
                break;
            }
            it(&mut p);
        }
        assert!(game::level_points(&c, &p, 1) + rp >= thr, "ön koşul sağlanamadı");
        assert!(game::level_points(&c, &p, 1) < thr);
        st.store.update(|q| *q = p.clone()).unwrap();
        assert!(!game::is_unlocked(&c, &st.store.snapshot(), 2));

        let (_, t) = post_hx(&app, "/ders/1-01/okudum", "").await;
        assert!(t.contains("unlock-notice"), "{t}");
        assert!(t.contains(&format!("/seviye/{}", l2.slug)));
        // ikinci kez: yeni açılış yok
        let (_, t) = post_hx(&app, "/ders/1-01/okudum", "").await;
        assert!(!t.contains("unlock-notice"));
    }

    #[test]
    fn kullanici_kodu_escape_edilir() {
        let t = RunResultTpl {
            t: Lang::Tr.ui(),
            stats: StatsView { points: 0, max: 0, oob: true },
            class_name: "result-fail",
            title: "x".into(),
            output: "<script>alert(1)</script>".into(),
            gained_msg: String::new(),
            note: String::new(),
            badges: vec![],
            unlocked: vec![],
            next_href: String::new(),
            next_label: String::new(),
        };
        let h = t.render().unwrap();
        assert!(h.contains("&lt;script&gt;"));
        assert!(!h.contains("<script>"));
    }

    async fn get_en(app: &Router, uri: &str) -> (StatusCode, String) {
        let req = Request::builder()
            .uri(uri)
            .header("cookie", "lang=en")
            .body(Body::empty())
            .unwrap();
        send(app, req).await
    }

    #[tokio::test]
    async fn dil_secimi_cerez_yazar_ve_geri_doner() {
        let app = app();
        let req = Request::builder()
            .uri("/dil/en")
            .header("referer", "http://localhost:8080/ders/1-01")
            .body(Body::empty())
            .unwrap();
        let r = app.clone().oneshot(req).await.unwrap();
        assert_eq!(r.status(), StatusCode::SEE_OTHER);
        let h = r.headers();
        assert_eq!(h.get("location").unwrap(), "/ders/1-01");
        assert!(h.get("set-cookie").unwrap().to_str().unwrap().starts_with("lang=en;"));
        // bilinmeyen dil kodu Türkçeye düşer, Referer yoksa panele döner
        let req = Request::builder().uri("/dil/xx").body(Body::empty()).unwrap();
        let r = app.clone().oneshot(req).await.unwrap();
        assert_eq!(r.headers().get("location").unwrap(), "/");
        assert!(r.headers().get("set-cookie").unwrap().to_str().unwrap().starts_with("lang=tr;"));
    }

    #[test]
    fn local_path_yalniz_yerel_yol() {
        assert_eq!(local_path("http://localhost:8080/seviye/temeller/test"), "/seviye/temeller/test");
        assert_eq!(local_path("http://localhost:8080"), "/");
        assert_eq!(local_path("http://evil.example//evil.example/x"), "/");
        assert_eq!(local_path("http://localhost:8080/dil/en"), "/");
        assert_eq!(local_path("/ders/1-02"), "/ders/1-02");
    }

    #[tokio::test]
    async fn ingilizce_arayuz_ve_icerik() {
        let (app, st) = parts();
        let (s, tr) = get_status(&app, "/").await;
        assert_eq!(s, StatusCode::OK);
        assert!(tr.contains("<html lang=\"tr\">"));
        assert!(tr.contains("Puan:"));
        assert!(tr.contains("lang-menu"));
        assert!(!tr.contains("XP"));
        let (s, en) = get_en(&app, "/").await;
        assert_eq!(s, StatusCode::OK);
        assert!(en.contains("<html lang=\"en\">"));
        assert!(en.contains("Points:"));
        assert!(!en.contains("Puan:"));
        // içerik de dile göre değişir, adresler aynı kalır
        let l_tr = st.catalog.level_in(Lang::Tr, 1).unwrap();
        let l_en = st.catalog.level_in(Lang::En, 1).unwrap();
        assert_ne!(l_tr.lessons[0].title, l_en.lessons[0].title);
        let (_, d_tr) = get_status(&app, "/ders/1-01").await;
        let (_, d_en) = get_en(&app, "/ders/1-01").await;
        assert!(d_en.contains("Mark as read"));
        assert!(d_en.contains("<h2>Why</h2>"), "İngilizce ders gövdesi bekleniyordu");
        assert!(d_tr.contains("<h2>Neden</h2>"));
        // kilit ve hata iletileri
        let (s, body) = get_en(&app, "/ders/2-01").await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        assert!(body.contains("Locked"));
        let (s, body) = get_en(&app, "/nope").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert!(body.contains("Page not found"));
    }

    #[test]
    fn inline_md_sarmalayici_atar() {
        assert_eq!(inline_md("a `b`"), "a <code>b</code>");
    }
}
