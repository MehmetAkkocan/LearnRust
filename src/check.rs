//! `--check-content`: tüm örnek/çözüm/starter kodlarını gerçekten derleyip doğrular.

use crate::content::Catalog;
use crate::runner::{self, RunOutcome, RunResult};
use tokio::task::JoinSet;

#[derive(Clone, Copy)]
enum Kind {
    Example,
    Solution,
    Starter,
}

struct Job {
    label: String,
    kind: Kind,
    code: String,
    tests: String,
}

fn head3(s: &str) -> String {
    s.lines().take(3).collect::<Vec<_>>().join("\n      ")
}

fn judge(kind: Kind, r: &RunResult) -> Result<(), String> {
    let detail = || {
        let err = if r.stderr.trim().is_empty() { &r.stdout } else { &r.stderr };
        format!("{:?}\n      {}", r.outcome, head3(err))
    };
    match kind {
        Kind::Example | Kind::Solution => {
            if r.outcome == RunOutcome::Passed {
                Ok(())
            } else {
                Err(detail())
            }
        }
        Kind::Starter => match r.outcome {
            RunOutcome::TestsFailed => Ok(()),
            RunOutcome::CompileError => Err(format!("starter derlenmiyor
      {}", head3(&r.stderr))),
            RunOutcome::Passed => Err("starter passes!".to_string()),
            _ => Err(detail()),
        },
    }
}

fn job(label: String, kind: Kind, code: &str, tests: &str) -> Job {
    Job { label, kind, code: code.to_string(), tests: tests.to_string() }
}

pub fn run(catalog: &Catalog) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime kurulamadı: {e}");
            return 1;
        }
    };
    let only_raw = std::env::var("CHECK_LEVEL").ok();
    let only: Option<u32> = only_raw.as_ref().and_then(|v| v.trim().parse().ok());
    if let Some(n) = only_raw {
        if !n.trim().parse::<u32>().is_ok_and(|id| catalog.playable().any(|l| l.id == id)) {
            eprintln!("CHECK_LEVEL geçersiz veya seviye yok: {n}");
            return 1;
        }
    }
    let (mut total, mut failed) = (0usize, 0usize);

    for lv in catalog.playable() {
        if only.is_some_and(|n| n != lv.id) {
            continue;
        }
        println!("== Seviye {} ({}) ==", lv.id, lv.slug);
        let (nl, nm, nc) = (lv.lessons.len(), lv.quiz.mcq.len(), lv.quiz.coding.len());
        let structural = [
            ((5..=8).contains(&nl), format!("{}: ders sayısı {nl} (5-8 olmalı)", lv.id)),
            (nm == 5, format!("{}: mcq sayısı {nm} (5 olmalı)", lv.id)),
            ((1..=2).contains(&nc), format!("{}: coding sayısı {nc} (1-2 olmalı)", lv.id)),
        ];
        for (ok, msg) in structural {
            total += 1;
            if ok {
                println!("OK   {msg}");
            } else {
                failed += 1;
                println!("FAIL {msg}");
            }
        }

        let mut jobs = Vec::new();
        for l in &lv.lessons {
            let e = &l.exercise;
            jobs.push(job(format!("{} example", l.id), Kind::Example, &l.example, ""));
            jobs.push(job(format!("{} solution", l.id), Kind::Solution, &e.solution, &e.tests));
            jobs.push(job(format!("{} starter", l.id), Kind::Starter, &e.starter, &e.tests));
        }
        for q in &lv.quiz.coding {
            jobs.push(job(format!("{} solution", q.id), Kind::Solution, &q.solution, &q.tests));
            jobs.push(job(format!("{} starter", q.id), Kind::Starter, &q.starter, &q.tests));
        }

        // İngilizce görünüm: yalnız Türkçeden farklı (ezilmiş) kod yeniden denetlenir.
        if let Some(en) = catalog.en.iter().find(|l| l.id == lv.id) {
            for (l, e) in lv.lessons.iter().zip(&en.lessons) {
                let (a, b) = (&l.exercise, &e.exercise);
                if e.example != l.example {
                    jobs.push(job(format!("{} example (en)", e.id), Kind::Example, &e.example, ""));
                }
                if (&a.starter, &a.tests, &a.solution) != (&b.starter, &b.tests, &b.solution) {
                    jobs.push(job(format!("{} solution (en)", e.id), Kind::Solution, &b.solution, &b.tests));
                    jobs.push(job(format!("{} starter (en)", e.id), Kind::Starter, &b.starter, &b.tests));
                }
            }
            for (q, e) in lv.quiz.coding.iter().zip(&en.quiz.coding) {
                if (&q.starter, &q.tests, &q.solution) != (&e.starter, &e.tests, &e.solution) {
                    jobs.push(job(format!("{} solution (en)", e.id), Kind::Solution, &e.solution, &e.tests));
                    jobs.push(job(format!("{} starter (en)", e.id), Kind::Starter, &e.starter, &e.tests));
                }
            }
        }

        // Eşzamanlılığı runner içindeki semafor sınırlar.
        let results = rt.block_on(async {
            let mut set = JoinSet::new();
            for (i, j) in jobs.into_iter().enumerate() {
                set.spawn(async move {
                    let r = match j.kind {
                        Kind::Example => runner::run_program(&j.code).await,
                        _ => runner::run_tests(&j.code, &j.tests).await,
                    };
                    (i, j.label, judge(j.kind, &r))
                });
            }
            let mut out = Vec::new();
            while let Some(r) = set.join_next().await {
                match r {
                    Ok(v) => out.push(v),
                    Err(e) => out.push((usize::MAX, "görev".to_string(), Err(format!("panic: {e}")))),
                }
            }
            out.sort_by_key(|v| v.0);
            out
        });

        for (_, label, r) in results {
            total += 1;
            match r {
                Ok(()) => println!("OK   {label}"),
                Err(m) => {
                    failed += 1;
                    println!("FAIL {label} {m}");
                }
            }
        }
    }

    // İngilizce üst katmanı eksik içerik (İngilizce modda Türkçe görünür).
    if only.is_none() {
        println!("== İngilizce çeviri ==");
        total += 1;
        if catalog.missing_en.is_empty() {
            println!("OK   tüm içeriğin İngilizcesi var");
        } else {
            failed += 1;
            println!("FAIL İngilizcesi eksik: {}", catalog.missing_en.join(", "));
        }
    }

    println!("\nToplam: {total}  Geçen: {}  Başarısız: {failed}", total - failed);
    if failed > 0 {
        1
    } else {
        0
    }
}
