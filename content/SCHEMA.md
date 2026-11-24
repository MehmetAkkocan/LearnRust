# İçerik Şeması

Tüm dosyalar UTF-8 TOML'dur. Çok satırlı kod/markdown için literal string (`'''...'''`) kullanın; kaçış gerekmez.
TOML tuzakları: literal string içinde `'''` yazılamaz; tek satırlık `"..."` alanlarda (prompt, q, choices) çift tırnak `\"` ile kaçırılır.

## Dizin yapısı

```
content/
  level0/            # fixture (gerçek içerik değil, oynanabilir listede görünmez; DOKUNMAYIN)
  level1/
    level.toml
    01-kurulum-cargo.toml
    02-degiskenler.toml
    ...
  level7/
```

Dizin numarası `level.toml` içindeki `id` ile aynı olmalıdır. Dersler dosya adına göre sıralanır (`NN-slug.toml`, NN iki haneli).

## Sabit seviye tablosu (slug DEĞİŞTİRİLMEZ; adres çubuğunda kullanılır)

| id | slug | title (tr) | title (en) |
|---|---|---|---|
| 1 | temeller | Temeller | Basics |
| 2 | sahiplik | Ownership ve Borrow | Ownership and Borrowing |
| 3 | veri-modelleme | Kendi Tiplerini Tanımla | Defining Your Own Types |
| 4 | koleksiyon-hata | Koleksiyonlar ve Hata Yönetimi | Collections and Error Handling |
| 5 | ileri-temel | İleri Konular | Beyond the Basics |
| 6 | eszamanlilik | Eşzamanlılık (Concurrency) | Concurrency |
| 7 | senior | Senior | Senior |

## Dil ve üslup (Türkçe içerik)

Metin çeviri gibi DURMAMALI. Konuyu bilen biri, yanındaki arkadaşına anlatır gibi yazılır.

- Kısa, net cümleler; bir cümlede tek fikir. "Sen" diye hitap et (`yaz`, `dene`, `dikkat et`; "yazın/yazınız" değil).
- Soyut tanım yerine somut durum: "şunu yaparsan şu olur". Gerekirse gündelik hayattan kısa bir benzetme.
- İngilizce cümle kalıbını Türkçeye taşıma ("X, Y'nin yoludur", "…sağlar", "…biçimindedir", "…gerçekleştirilir" gibi ağır/edilgen kalıplardan kaçın).
- Teknik doğruluk her şeyden önce gelir; sadeleştirirken anlam DEĞİŞMEZ, bilgi eksilmez.
- Derleyici hata mesajları derleyicinin yazdığı gibi (İngilizce) verilir, yanına ne demek istediği sade Türkçeyle yazılır.

### Terimler

Rust'a özgü terimler ÇEVRİLMEZ; belgelerde ve hata mesajlarında göreceğin asıl ad kullanılır. Derste ilk geçtiği yerde ne olduğu bir cümleyle, düz Türkçeyle anlatılır. Türkçe ek kesme işaretiyle gelir (`trait'i`, `closure'ın`, `thread'ler`).

- **İngilizce kalır:** ownership, borrow, move, drop, clone, slice, struct, enum, trait, trait bound, trait object, generic, lifetime, closure, iterator, thread, channel, mutex, lock, heap, stack, crate, macro, unsafe, raw pointer, panic, shadowing, future, executor, deadlock, data race, orphan rule, associated function, variant (enum seçeneği), guard, marker trait, UB, FFI, ABI.
- `match` kolları için "kol (arm)" denir.
- **Türkçe kalır (yerleşik kelimeler):** değişken, fonksiyon, metot, parametre, tip, değer, döngü, koşul, referans, modül, derleyici, bellek, hata, test, sahip (owner), kapsam (scope), desen (pattern), değiştirilebilir (`mut`), derleme zamanı, çalışma zamanı, taşma (overflow).
- Fiiller doğal Türkçeyle yazılır, ilk geçişte terim parantezde verilir: "değer `b`'ye taşınır (move)", "değeri ödünç alırsın (borrow)", "değer silinir (drop)".
- **Kullanma:** dilim (slice), yineleyici (iterator), özellik (trait), ömür / yaşam süresi (lifetime; açıklarken "referansın geçerli kaldığı süre" de), kapanış (closure), iş parçacığı (thread), kanal (channel), öbek (heap), yığın (stack), sandık (crate), yetim kuralı, ilişkili fonksiyon, "ödünç alma" ve "sahiplik" kelimelerini tek başına konu adı olarak.

## İngilizce içerik (`content/en/`)

Türkçe dosyalar asıl kaynaktır (id, puan, `answer`, kod). İngilizce metin, aynı adlı bir "üst katman" dosyasında durur:

```
content/en/level1/level.toml            # content/level1/level.toml için
content/en/level1/01-kurulum-cargo.toml # content/level1/01-kurulum-cargo.toml için (dosya adı AYNI)
```

Üst katman yalnız aşağıdaki alanları içerebilir (başka alan hata verir). Verilmeyen opsiyonel alan Türkçe dosyadan alınır; üst katman dosyası hiç yoksa o içerik İngilizce modda Türkçe görünür ve `--check-content` FAIL verir.

Ders üst katmanı:

| Alan | Zorunlu | Açıklama |
|---|---|---|
| title | evet | İngilizce başlık |
| body_md | evet | Bölümler sırasıyla `## Why`, `## How`, `## Watch out` |
| example | hayır | Türkçe `example` içinde Türkçe metin (yorum, string, tanımlayıcı) varsa İngilizcesi |
| `[exercise]` prompt | evet | İngilizce görev metni |
| `[exercise]` starter / tests / solution | hayır | Aşağıdaki kurala göre |

Seviye üst katmanı: `title`, `description`, her MCQ için `[[quiz.mcq]]` (`id`, `q`, `choices`: Türkçedekiyle AYNI sırada tam 4 şık; `answer` yazılmaz), her kodlama sorusu için `[[quiz.coding]]` (`id`, `prompt`, opsiyonel `starter` / `tests` / `solution`).

Kod alanlarını ezme kuralı:

- Türkçe `starter` içinde Türkçe tanımlayıcı (fonksiyon/tip/alan adı) veya testin beklediği Türkçe metin varsa `starter`, `tests`, `solution` ÜÇÜ BİRDEN İngilizce tanımlayıcılarla yazılır.
- Yalnız test fonksiyonlarının adı veya test verisi Türkçeyse sadece `tests` ezilir (İngilizce test adları, aynı doğrulamalar).
- Kod zaten İngilizceyse hiçbiri yazılmaz.
- Ezilen kod, Türkçe sürümle AYNI görevi ve aynı zorluk düzeyini taşır; tüm kod kuralları (aşağıda) onun için de geçerlidir ve `--check-content` onu da derler.

İngilizce metin de çeviri kokmamalı: aynı bilgiyi aynı sırayla, sade ve doğrudan İngilizceyle anlatır ("you" diye hitap eder).

## Id kuralları

Tüm id'ler yalnız ASCII küçük harf, rakam ve `-` içerir.

- Ders id: `<N>-<NN>` (örn `1-01`); NN, dosya adındaki NN ile AYNI olmalı (`01-merhaba.toml` -> `1-01`).
- Coding quiz id: `<N>-q<K>` (örn `1-q1`).
- MCQ id: `<N>-m<K>` (örn `1-m1`), ZORUNLU. İlerleme kaydı bu id'lerle tutulur; yayından sonra id değiştirilmez.

## level.toml

| Alan | Tip | Açıklama |
|---|---|---|
| id | u32 | Seviye numarası (1..7) |
| slug | string | Yukarıdaki tablodan |
| title | string | Yukarıdaki tablodan (tr) |
| description | string | Kısa açıklama |
| unlock_threshold | u32 (opsiyonel) | **Writer bu alanı YAZMAZ.** Kod hesaplar: önceki seviyenin max_points'inin %70'i, 10'a yuvarlanır (level1 -> 0) |
| `[[quiz.mcq]]` | tablo listesi | `id`, `q`, `choices` (TAM 4 şık), `answer` (0 tabanlı index), `points` (varsayılan 10) |
| `[[quiz.coding]]` | tablo listesi | `id`, `prompt`, `starter`, `tests`, `solution`, `points` (varsayılan 50) |

Örnek:

```toml
id = 1
slug = "temeller"
title = "Temeller"
description = "Kurulum, değişkenler, tipler, fonksiyonlar."

[[quiz.mcq]]
id = "1-m1"
q = "Değişkenler varsayılan olarak nasıldır?"
choices = ["Değiştirilebilir", "Değiştirilemez", "Global", "Gizli"]
answer = 1

[[quiz.coding]]
id = "1-q1"
prompt = "`double` fonksiyonunu yazın."
starter = '''
fn double(x: i32) -> i32 {
    todo!()
}
'''
tests = '''
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iki_kati() { assert_eq!(double(4), 8); }
}
'''
solution = '''
fn double(x: i32) -> i32 { x * 2 }
'''
```

MCQ kuralları: tam 4 şık; doğru şıkkın index'i (`answer`) sorular arasında dağıtılsın (hep aynı olmasın).

## Ders dosyası (NN-slug.toml)

| Alan | Tip | Açıklama |
|---|---|---|
| id | string | `<N>-<NN>` (yukarıya bakın) |
| title | string | Ders başlığı |
| read_points | u32 | Varsayılan 10 |
| body_md | string | Markdown anlatım |
| example | string | Tek başına derlenip çalışan TAM program (`fn main` içerir) |
| `[exercise]` | tablo | `prompt`, `starter`, `tests`, `solution`, `points` (varsayılan 30) |

Örnek: `content/level0/01-merhaba.toml`.

`body_md` biçimi: 150-400 kelime, Türkçe ("Dil ve üslup" bölümüne uy); `#` (h1) KULLANMA (başlık zaten gösterilir); bölümler sırasıyla `## Neden`, `## Nasıl`, `## Dikkat`. Kod tanımlayıcıları ASCII; Türkçe karakter yalnız string ve yorumlarda.

## Puan tablosu

| Kalem | Puan |
|---|---|
| Ders okuma | 10 |
| Egzersiz | 30 |
| MCQ (soru başı) | 10 |
| Coding quiz (soru başı) | 50 |

Seviye `max_points` = Σ(read_points + exercise.points) + Σ mcq.points + Σ coding.points.
Örnek: 6 ders + 5 mcq + 1 coding = 240 + 50 + 50 = 340; sonraki seviyenin eşiği 240 (kod hesaplar).

Her seviyede: 5-8 ders, 5 mcq, 1-2 coding quiz.

## Doğrulama kuralları (yükleyici; ihlalde hata)

- Dizin numarası != level id; tekrar eden seviye id / slug
- Slug veya id biçimi geçersiz (ASCII küçük harf, rakam, `-`); ders id'si dosya adındaki NN ile uyumsuz; id ön eki (`<N>-`, `<N>-q`, `<N>-m`) yanlış
- Tekrar eden ders / coding / mcq id'si
- MCQ'da şık sayısı != 4 veya `answer` aralık dışı
- Boş `tests`, `starter` veya `solution`

## Egzersiz / quiz kod kuralları

- `tests` alanı şu biçimde olmalı: `#[cfg(test)] mod tests { use super::*; #[test] fn ...() { ... } }`
- Öğrenci kodu ile `tests` birleştirilip `rustc --edition 2021 --test` ile derlenir.
- std dışı crate YOK, stdin YOK, 10 saniye içinde bitmeli.
- `starter` ve `solution` içinde `mod tests`, `#[cfg(test)]` veya `fn main` OLMAZ.
- `starter` derlenmeli ama testler başarısız olmalı (`todo!()`); `solution` tüm testleri geçmeli.
- `tests` yalnızca `starter`'da tanımlı isimleri (fonksiyon/struct/enum/trait) kullanır.
- stdout test edilmez; yalnız dönüş değeri / durum doğrulanır.
- Thread içeren testler deterministik olmalı (join/channel ile; `sleep` ile zamanlamaya güvenme).
- `example` alanı tek başına derlenip çalışan tam programdır (`fn main` içerir), aynı kısıtlar (yalnız std, stdin yok, 10 sn).
- Ders egzersizi YALNIZ o derse kadar (o ders dahil) anlatılanlarla çözülebilmelidir; aynı seviyenin SONRAKİ derslerinin konusunu gerektiremez (ör. 2-01'de `&` referansı, 6-02'de `thread::spawn` yazdırılmaz). Sonraki konu kaçınılmazsa o kısım `starter` veya `tests` içinde hazır verilir.
- Seviye quizleri YALNIZ o seviyenin ve önceki seviyelerin konularını kullanır.
- Hiçbir derste anlatılmamış bir std metodu gerekiyorsa (ör. `abs`, `split_once`, `saturating_add`) `prompt` içinde ipucu olarak adı verilir.
- `solution` da aynı kurala uyar: öğrencinin o ana kadar gördüğü araçlarla yazılır.

### Test edilemeyen konular

Kurulum/cargo, cargo test/doc, benchmark, derleyici hatalarını okuma gibi konularda da egzersiz saf fonksiyon + assert testidir. Kavram "bozuk kodu düzelt" kalıbıyla uygulanır: starter derlenebilir ama yanlış çalışan (veya `todo!()` içeren) kod verir; öğrenci düzeltir, testler doğru davranışı doğrular. (Derlenmeyen starter kullanılmaz; hata okuma dersinde derleme hatası mesajı `body_md` içinde gösterilir, egzersiz mantık hatasını düzelttirir.)

L7 `extern "C"`: yalnız Rust içinde tanımlı `extern "C" fn` veya libc'de hazır sembol (ör. `abs`, `strlen`) kullanılır; C kaynak dosyası ve build.rs YOK.

## 7 seviyenin SABİT konu listesi

Her konu en az bir ders olacak şekilde işlenir (yakın konular birleştirilebilir; 5-8 ders sınırı korunur).

1. **L1 Temeller**: kurulum/cargo, değişkenler, tipler, fonksiyonlar, kontrol akışı, String temel
2. **L2 Sahiplik**: ownership, move, borrow, &mut, slice, String vs &str
3. **L3 Veri modelleme**: struct, impl, enum, Option, match, if let, modül temel
4. **L4 Koleksiyon & hata**: Vec, HashMap, Result, ?, custom error, trait, generic, trait object
5. **L5 İleri temel**: lifetime, closure, iterator adaptörleri, cargo/test/doc, Box, modül organizasyonu
6. **L6 Eşzamanlılık**: Rc/RefCell, Arc/Mutex, thread, channel, Send/Sync, basit Future/executor (tokio YOK)
7. **L7 Senior**: unsafe, raw pointer, extern "C" (gcc yok), declarative macro, performans/benchmark düşüncesi, API tasarımı/mimari, derleyici hatalarını okuma
