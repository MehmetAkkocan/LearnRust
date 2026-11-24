//! JSON ilerleme kaydı: Progress + Store (atomik yazım).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    pub xp: u32,
    /// Okunan ders id'leri.
    pub reads: BTreeSet<String>,
    /// Egzersizi geçilen ders id'leri.
    pub exercises: BTreeSet<String>,
    /// mcq id -> son cevap doğru mu.
    pub mcq: BTreeMap<String, bool>,
    /// Çözülen coding quiz id'leri.
    pub coding: BTreeSet<String>,
    pub badges: BTreeSet<String>,
    pub updated_at: String,
}

/// Şimdiki zaman, RFC3339 (UTC, saniye hassasiyeti).
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant civil_from_days
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub struct Store {
    path: PathBuf,
    inner: Mutex<Progress>,
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

impl Store {
    /// Dosya yoksa Default; bozuksa `.bak`'a kopyalayıp Default ile başlar.
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        let path = path.as_ref().to_path_buf();
        let progress = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Progress>(&bytes) {
                Ok(p) => p,
                Err(e) => {
                    let bak = sibling(&path, ".bak");
                    tracing::warn!(
                        "ilerleme dosyası bozuk ({}): {}; {} dosyasına yedeklenip sıfırdan başlanıyor",
                        path.display(),
                        e,
                        bak.display()
                    );
                    if let Err(ce) = fs::copy(&path, &bak) {
                        tracing::warn!("yedek alınamadı: {}", ce);
                    }
                    Progress::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Progress::default(),
            Err(e) => {
                return Err(e).with_context(|| format!("okunamadı: {}", path.display()));
            }
        };
        Ok(Store {
            path,
            inner: Mutex::new(progress),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomik yazım (.tmp + rename); rename başarısızsa doğrudan yazıma düşer.
    pub fn save(&self, p: &Progress) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("dizin oluşturulamadı: {}", parent.display()))?;
            }
        }
        let json = serde_json::to_string_pretty(p)?;
        let tmp = sibling(&self.path, ".tmp");
        {
            use std::io::Write;
            let mut f = fs::File::create(&tmp)
                .with_context(|| format!("yazılamadı: {}", tmp.display()))?;
            f.write_all(json.as_bytes())
                .with_context(|| format!("yazılamadı: {}", tmp.display()))?;
            f.sync_all().ok();
        }
        if let Err(e) = fs::rename(&tmp, &self.path) {
            tracing::warn!("rename başarısız ({}); doğrudan yazıma düşülüyor", e);
            fs::write(&self.path, &json)
                .with_context(|| format!("yazılamadı: {}", self.path.display()))?;
            let _ = fs::remove_file(&tmp);
        }
        Ok(())
    }

    /// Salt okuma.
    pub fn with<R>(&self, f: impl FnOnce(&Progress) -> R) -> R {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        f(&g)
    }

    /// Kopya üzerinde değiştirir; değişiklik varsa `updated_at`'i günceller ve
    /// kaydeder. Kayıt BAŞARILI olursa bellekteki durum güncellenir.
    /// Değişiklik yoksa diske yazılmaz.
    pub fn update<R>(&self, f: impl FnOnce(&mut Progress) -> R) -> Result<R> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = g.clone();
        let r = f(&mut next);
        if next == *g {
            return Ok(r);
        }
        next.updated_at = now_rfc3339();
        self.save(&next)?;
        *g = next;
        Ok(r)
    }

    pub fn snapshot(&self) -> Progress {
        self.with(|p| p.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tmp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        std::env::temp_dir().join(format!(
            "rla_store_{}_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst),
            nanos
        ))
    }

    #[test]
    fn roundtrip() {
        let d = tmp_dir();
        let f = d.join("alt").join("progress.json");
        let s = Arc::new(Store::open(&f).unwrap());
        assert_eq!(s.snapshot(), Progress::default());
        s.update(|p| {
            p.xp = 40;
            p.reads.insert("1-01".into());
            p.mcq.insert("1-m1".into(), false);
            p.badges.insert("ilk-adim".into());
        })
        .unwrap();
        assert!(!sibling(&f, ".tmp").exists());
        let s2 = Store::open(&f).unwrap();
        let p = s2.snapshot();
        assert_eq!(p.xp, 40);
        assert!(p.reads.contains("1-01"));
        assert_eq!(p.mcq.get("1-m1"), Some(&false));
        assert!(p.updated_at.ends_with('Z') && p.updated_at.len() == 20);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn corrupt_is_backed_up() {
        let d = tmp_dir();
        fs::create_dir_all(&d).unwrap();
        let f = d.join("progress.json");
        fs::write(&f, "{bozuk").unwrap();
        let s = Store::open(&f).unwrap();
        assert_eq!(s.snapshot(), Progress::default());
        assert_eq!(fs::read_to_string(sibling(&f, ".bak")).unwrap(), "{bozuk");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn invalid_utf8_is_backed_up() {
        let d = tmp_dir();
        fs::create_dir_all(&d).unwrap();
        let f = d.join("progress.json");
        fs::write(&f, [0xff, 0xfe, 0x00, 0x7b]).unwrap();
        let s = Store::open(&f).unwrap();
        assert_eq!(s.snapshot(), Progress::default());
        assert_eq!(fs::read(sibling(&f, ".bak")).unwrap(), vec![0xff, 0xfe, 0x00, 0x7b]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_updates() {
        let d = tmp_dir();
        let s = Arc::new(Store::open(d.join("p.json")).unwrap());
        let hs: Vec<_> = (0..2)
            .map(|t| {
                let s = s.clone();
                std::thread::spawn(move || {
                    for i in 0..25 {
                        s.update(|p| {
                            p.xp += 1;
                            p.reads.insert(format!("{t}-{i}"));
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let disk = Store::open(d.join("p.json")).unwrap().snapshot();
        assert_eq!(disk.xp, 50);
        assert_eq!(disk.reads.len(), 50);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn failed_save_keeps_memory_and_noop_skips_write() {
        let d = tmp_dir();
        fs::create_dir_all(&d).unwrap();
        // üst "dizin" aslında dosya -> save başarısız olur
        let s = Store::open(d.join("sub").join("p.json")).unwrap();
        fs::write(d.join("sub"), "x").unwrap();
        assert!(s.update(|p| p.xp = 5).is_err());
        assert_eq!(s.snapshot().xp, 0);
        // değişiklik yok -> yazma yok, hata yok
        s.update(|_| ()).unwrap();
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rfc3339_shape() {
        let t = now_rfc3339();
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
        assert!(t.starts_with("20"));
    }
}
